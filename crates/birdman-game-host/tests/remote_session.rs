//! Actual HTTP/WebSocket software consumers; this test does not establish GPU rendering.
use birdman_game_app::application::{ApplicationSession, SessionCommand};
use birdman_game_host::{
    controller_io::NativeIoSignal,
    network_io::NativeNetworkIo,
    service::{HostListenConfig, HostManagementCommand, HostService, HostServiceHandle},
    web_distribution::{WebDistribution, write_web_bundle_manifest},
};
use birdman_game_presentation::game_ui::{
    GameUiControlInput, GameUiOverlay, GameUiScene, GameUiView, GameUiViewport,
};
use birdman_game_runtime::*;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

struct Client {
    runtime: ApplicationRuntime,
    network: NativeNetworkIo,
    epoch: Instant,
    platform: ApplicationPlatformObservation,
    view: Option<Arc<GameUiView>>,
    inputs: VecDeque<ApplicationInput>,
    timers: BTreeMap<u32, f64>,
    candidates: BTreeMap<u32, ApplicationGraphicsTarget>,
    socket_id: Option<u32>,
    sent: Vec<String>,
    dynamic: BTreeMap<ApplicationBufferKey, [u8; 32]>,
    preparations: usize,
    next_frame_ms: f64,
    gamepads: Vec<ApplicationGamepadObservation>,
    aircraft_position: Option<[f64; 3]>,
    consumed_frame_revision: Option<u32>,
    lifecycle: ApplicationLifecycle,
    diagnostic: Option<String>,
    host: HostServiceHandle,
    wire_trace: VecDeque<String>,
}

impl Client {
    fn new(epoch: Instant, host: &str, renderer_id: u32, handle: HostServiceHandle) -> Self {
        let platform = ApplicationPlatformObservation {
            location: None,
            gpu_timing_requested: false,
            surface: None,
            renderer: Some(ApplicationRendererObservation {
                resource_id: renderer_id,
                extent_px: [1280, 720],
                screen_extent_px: [1280, 720],
                xr_extent_px: None,
                gpu_timing: ApplicationGpuTimingCapability::Unsupported {
                    cause: "Software byte consumer has no GPU timer".to_owned(),
                },
            }),
            viewport: GameUiViewport {
                width: 1280.0,
                height: 720.0,
                root_font_size: 16.0,
            },
            capabilities: ApplicationCapabilities {
                analysis_available: true,
                replay_available: true,
                flight_log_available: true,
                ..Default::default()
            },
        };
        // This constructor runs on the ordinary test thread, outside any ambient Tokio runtime.
        let network = NativeNetworkIo::new(Arc::new(NativeIoSignal::default())).unwrap();
        let runtime = ApplicationRuntime::connect(platform.clone(), host).unwrap();
        let mut client = Self {
            runtime,
            network,
            epoch,
            platform,
            view: None,
            inputs: VecDeque::new(),
            timers: BTreeMap::new(),
            candidates: BTreeMap::new(),
            socket_id: None,
            sent: Vec::new(),
            dynamic: BTreeMap::new(),
            preparations: 0,
            next_frame_ms: 0.0,
            gamepads: Vec::new(),
            aircraft_position: None,
            consumed_frame_revision: None,
            lifecycle: ApplicationLifecycle::Running,
            diagnostic: None,
            host: handle,
            wire_trace: VecDeque::new(),
        };
        client.observe(ApplicationInput::Platform {
            observation: client.platform.clone(),
        });
        client
    }

    fn observe(&mut self, input: ApplicationInput) {
        self.inputs.push_back(input);
        self.flush();
    }
    fn flush(&mut self) {
        while let Some(input) = self.inputs.pop_front() {
            let consumed = matches!(input, ApplicationInput::BuffersConsumed { .. });
            let output = self
                .runtime
                .update(
                    ApplicationObservation {
                        observed_ms: self.epoch.elapsed().as_secs_f64() * 1000.0,
                        input,
                    },
                    &mut |_| panic!("Screen software consumer has no spatial font request"),
                )
                .unwrap();
            if consumed {
                assert!(
                    output.drawing.is_none(),
                    "consumer completion must not create feedback drawings"
                );
            }
            self.accept(output);
        }
    }
    fn accept(&mut self, output: ApplicationOutput) {
        self.lifecycle = output.lifecycle;
        if output.diagnostic.is_some() {
            self.diagnostic = output.diagnostic;
        }
        if let Some(drawing) = output.drawing {
            let position = drawing.scene.world_from_aircraft.position;
            self.aircraft_position = Some([position.x, position.y, position.z]);
            match drawing.ui {
                ApplicationUiProjection::View { view } => {
                    self.view = Some(view);
                    self.consumed_frame_revision = None;
                }
                ApplicationUiProjection::Frame { frame } => {
                    self.consumed_frame_revision = Some(frame.view_revision);
                }
            }
            if let Some(lease) = drawing.asset_lease {
                for key in &lease.buffers {
                    let bytes = self
                        .runtime
                        .asset_bytes(lease.lease_id, key.buffer_id, key.revision)
                        .unwrap();
                    let digest: [u8; 32] = Sha256::digest(&bytes).into();
                    self.dynamic.insert(*key, digest);
                    while self.dynamic.len() > 64 {
                        self.dynamic.pop_first();
                    }
                }
                self.inputs.push_back(ApplicationInput::BuffersConsumed {
                    lease_id: lease.lease_id,
                    renderer_resource_id: lease.renderer_resource_id,
                    resource_epoch: lease.resource_epoch,
                });
            }
        }
        for request in output.requests {
            match request {
                ApplicationRequest::ReadHost { .. }
                | ApplicationRequest::CancelHostRead { .. }
                | ApplicationRequest::NetworkOpen { .. }
                | ApplicationRequest::NetworkClose { .. }
                | ApplicationRequest::NetworkSend { .. }
                | ApplicationRequest::ObserveNetwork { .. } => {
                    if let ApplicationRequest::NetworkOpen { socket_id, .. } = &request {
                        self.socket_id = Some(*socket_id);
                    }
                    if let ApplicationRequest::NetworkSend {
                        payload: ApplicationNetworkData::Text { text },
                        ..
                    } = &request
                    {
                        trace_packet(&mut self.wire_trace, "send", text);
                        self.sent.push(text.clone());
                    }
                    self.network.submit(request).unwrap_or_else(|request| {
                        let (kind, length) = match &*request {
                            ApplicationRequest::ReadHost { .. } => ("read_host", 0),
                            ApplicationRequest::CancelHostRead { .. } => ("cancel_host_read", 0),
                            ApplicationRequest::NetworkOpen { .. } => ("network_open", 0),
                            ApplicationRequest::NetworkClose { .. } => ("network_close", 0),
                            ApplicationRequest::ObserveNetwork { .. } => ("observe_network", 0),
                            ApplicationRequest::NetworkSend { payload, .. } => match payload {
                                ApplicationNetworkData::Text { text } => ("network_send_text", text.len()),
                                ApplicationNetworkData::Binary { bytes } => ("network_send_binary", bytes.len()),
                            },
                            _ => unreachable!("network-only request match"),
                        };
                        panic!("network I/O rejected {kind} ({length} bytes); diagnostic={:?}; wire={:?}", self.diagnostic, self.wire_trace)
                    });
                }
                ApplicationRequest::ScheduleWake {
                    request_id, at_ms, ..
                } => {
                    self.timers.insert(request_id, at_ms);
                }
                ApplicationRequest::CancelWake { request_id } => {
                    self.timers.remove(&request_id);
                }
                ApplicationRequest::PrepareResources {
                    request_id,
                    graphics,
                    asset_lease,
                    assets,
                    ..
                } => {
                    self.preparations += 1;
                    // Read every real scoped transfer before acknowledging software preparation.
                    let mut actual = BTreeMap::new();
                    for key in &asset_lease.buffers {
                        let bytes = self
                            .runtime
                            .asset_bytes(asset_lease.lease_id, key.buffer_id, key.revision)
                            .unwrap();
                        actual.insert(key.buffer_id, bytes);
                    }
                    for mesh in &assets.scene.meshes {
                        Self::attribute(&actual, mesh.position, ApplicationBufferType::F32, 3);
                        Self::attribute(&actual, mesh.normal, ApplicationBufferType::F32, 3);
                        Self::attribute(&actual, mesh.index, ApplicationBufferType::U32, 1);
                        let indices = actual.get(&mesh.index.buffer_id).unwrap();
                        assert!(
                            indices
                                .chunks_exact(4)
                                .all(|bytes| u32::from_le_bytes(bytes.try_into().unwrap())
                                    < mesh.position.count)
                        );
                        for descriptor in [mesh.uv, mesh.color].into_iter().flatten() {
                            Self::attribute(
                                &actual,
                                descriptor,
                                descriptor.data_type,
                                descriptor.components,
                            );
                        }
                    }
                    for image in [
                        &assets.sky,
                        &assets.water.detail[0].image,
                        &assets.water.detail[1].image,
                    ] {
                        assert_eq!(
                            actual.get(&image.buffer_id).unwrap().len(),
                            image.size[0] as usize * image.size[1] as usize * 4
                        );
                    }
                    for texture in &assets.scene.wing_textures {
                        Self::attribute(&actual, texture.buffer, ApplicationBufferType::U8, 4);
                    }
                    for chunk in assets
                        .water
                        .geometry
                        .near
                        .iter()
                        .chain(std::iter::once(&assets.water.geometry.far))
                    {
                        for descriptor in [
                            chunk.position,
                            chunk.normal,
                            chunk.uv,
                            chunk.grid_spacing,
                            chunk.index,
                        ] {
                            Self::attribute(
                                &actual,
                                descriptor,
                                descriptor.data_type,
                                descriptor.components,
                            );
                        }
                    }
                    self.candidates.insert(request_id, graphics);
                    self.inputs.push_back(ApplicationInput::Resources {
                        request_id,
                        observation: ApplicationResourceObservation::Prepared {
                            resource_id: request_id,
                        },
                    });
                }
                ApplicationRequest::CommitResources {
                    request_id,
                    resource_id,
                } => {
                    let target = *self
                        .candidates
                        .get(&request_id)
                        .expect("CPU candidate was actually read");
                    self.inputs.push_back(ApplicationInput::Resources {
                        request_id,
                        observation: ApplicationResourceObservation::Committed {
                            resource_id,
                            actual: ApplicationResourceCommit {
                                applied: target.quality,
                                extent_px: target.extent_px,
                            },
                        },
                    });
                }
                ApplicationRequest::CancelResources { request_id } => {
                    self.candidates.remove(&request_id);
                    self.inputs.push_back(ApplicationInput::Resources {
                        request_id,
                        observation: ApplicationResourceObservation::Cancelled,
                    });
                }
                ApplicationRequest::ReleaseResources { resource_id } => {
                    self.candidates.remove(&resource_id);
                    self.inputs.push_back(ApplicationInput::ResourcesReleased {
                        resource_id,
                        failure: None,
                    });
                }
                ApplicationRequest::ReleaseRenderer {
                    renderer_resource_id,
                } => {
                    self.inputs.push_back(ApplicationInput::RendererReleased {
                        renderer_resource_id,
                        failure: None,
                    });
                }
                request => panic!("Unexpected Screen software I/O request: {request:?}"),
            }
        }
    }
    fn attribute(
        actual: &BTreeMap<u32, Vec<u8>>,
        descriptor: ApplicationBufferDescriptor,
        expected: ApplicationBufferType,
        components: u32,
    ) {
        assert_eq!(descriptor.data_type, expected);
        assert_eq!(descriptor.components, components);
        let bytes = actual
            .get(&descriptor.buffer_id)
            .expect("descriptor has real bytes");
        assert_eq!(
            bytes.len(),
            descriptor.count as usize * descriptor.stride_bytes as usize
        );
        if descriptor.data_type == ApplicationBufferType::F32 {
            assert!(
                bytes
                    .chunks_exact(4)
                    .all(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()).is_finite())
            );
        }
    }
    fn pump(&mut self) {
        let mut outputs = Vec::new();
        let now = self.epoch.elapsed().as_secs_f64() * 1000.0;
        let runtime = &mut self.runtime;
        let epoch = self.epoch;
        let wire_trace = &mut self.wire_trace;
        self.network.poll(|input| {
            match &input {
                ApplicationInput::NetworkMessage {
                    payload: ApplicationNetworkData::Text { text },
                    ..
                } => trace_packet(wire_trace, "receive", text),
                ApplicationInput::NetworkMessage {
                    payload: ApplicationNetworkData::Binary { bytes },
                    ..
                } => trace(wire_trace, format!("receive binary {} bytes", bytes.len())),
                ApplicationInput::NetworkOpened { failure, .. } => {
                    trace(wire_trace, format!("opened {failure:?}"))
                }
                ApplicationInput::NetworkClosed { code, reason, .. } => {
                    trace(wire_trace, format!("closed {code}: {reason}"))
                }
                _ => {}
            }
            outputs.push(
                runtime
                    .update(
                        ApplicationObservation {
                            observed_ms: epoch.elapsed().as_secs_f64() * 1000.0,
                            input,
                        },
                        &mut |_| panic!("Screen has no spatial font request"),
                    )
                    .unwrap(),
            )
        });
        for output in outputs {
            self.accept(output);
        }
        self.flush();
        let due: Vec<_> = self
            .timers
            .iter()
            .filter_map(|(id, at)| (*at <= now).then_some(*id))
            .collect();
        for request_id in due {
            self.timers.remove(&request_id);
            self.observe(ApplicationInput::Wake { request_id });
        }
        if now >= self.next_frame_ms {
            self.next_frame_ms = now + 1000.0 / 60.0;
            self.observe(ApplicationInput::Frame {
                display_resource_id: None,
                timestamp_ms: now,
                gamepads: self.gamepads.clone(),
                xr: None,
                spatial: ApplicationSpatialObservation::default(),
            });
        }
    }
    fn can_activate(&self, command: &SessionCommand) -> bool {
        let action = birdman_game_presentation::game_ui::GameUiAction::Session {
            command: command.clone(),
        };
        self.view.as_ref().is_some_and(|view| {
            view.panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .any(|control| {
                    control.enabled && control.content.activation_action().as_ref() == Some(&action)
                })
        })
    }
    fn activate(&mut self, command: SessionCommand) -> String {
        self.control(command.clone(), true).unwrap_or_else(|| {
            let view = self.view.as_ref().map(|view| (view.scene, view.view_revision, &view.interaction_scope));
            let sent: Vec<_> = self.sent.iter().rev().take(4)
                .map(|text| text.chars().take(256).collect::<String>()).collect();
            panic!("no actual control packet for {command:?}; view={view:?}; diagnostic={:?}; wire={:?}; sent_tail={sent:?}",
                self.diagnostic, self.wire_trace)
        })
    }
    fn paused(&self) -> bool {
        self.view.as_ref().is_some_and(|view| {
            view.scene == GameUiScene::Flight && view.active_overlay == Some(GameUiOverlay::Pause)
        })
    }
    fn flying(&self) -> bool {
        self.view
            .as_ref()
            .is_some_and(|view| view.scene == GameUiScene::Flight && view.active_overlay.is_none())
    }
    fn control(&mut self, command: SessionCommand, require_enabled: bool) -> Option<String> {
        let view = self.view.as_ref().expect("published view");
        let action = birdman_game_presentation::game_ui::GameUiAction::Session { command };
        let control_id = view
            .panels
            .iter()
            .flat_map(|panel| &panel.controls)
            .find(|control| {
                (!require_enabled || control.enabled)
                    && control.content.activation_action().as_ref() == Some(&action)
            })
            .unwrap()
            .id
            .clone();
        let before = self.sent.len();
        self.observe(ApplicationInput::Control {
            view_revision: view.view_revision,
            input: GameUiControlInput::Activate {
                control_id: control_id.clone(),
            },
        });
        self.sent[before..]
            .iter()
            .find(|text| text.contains(&control_id))
            .cloned()
    }
}

#[derive(serde::Deserialize)]
struct PacketKind {
    kind: String,
}
fn trace_packet(trace: &mut VecDeque<String>, direction: &str, text: &str) {
    let kind = serde_json::from_str::<PacketKind>(text)
        .map_or_else(|_| "invalid".to_owned(), |packet| packet.kind);
    self::trace(trace, format!("{direction} {kind} {} bytes", text.len()));
}
fn trace(trace: &mut VecDeque<String>, message: String) {
    if trace.back() == Some(&message) {
        return;
    }
    if trace.len() == 16 {
        trace.pop_front();
    }
    trace.push_back(message);
}

fn until(clients: &mut [Client], condition: impl Fn(&[Client]) -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        for client in clients.iter_mut() {
            client.pump();
        }
        if condition(clients) {
            return;
        }
        assert!(
            Instant::now() < end,
            "actual remote I/O did not reach the expected state: {:?}; host={:?} listening={:?} diagnostic={:?}",
            clients
                .iter()
                .map(|client| (
                    client.view.as_ref().map(|view| view.scene),
                    &client.diagnostic,
                    client.preparations,
                    client.lifecycle,
                    &client.wire_trace,
                ))
                .collect::<Vec<_>>(),
            clients[0].host.snapshot().application,
            clients[0].host.snapshot().listening,
            clients[0].host.snapshot().diagnostic
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn shared_host_scopes_two_actual_native_connections_and_software_buffers() {
    let source = APPLICATION_SOURCE_SHA.expect("Run this fixed-source integration test with BPG_APPLICATION_SOURCE_SHA or BPG_NATIVE_SOURCE_SHA");
    let temporary = std::env::temp_dir().join(format!(
        "birdman-remote-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&temporary).unwrap();
    let web_root = temporary.join("web");
    std::fs::create_dir(&web_root).unwrap();
    std::fs::write(
        web_root.join("index.html"),
        b"<!doctype html><meta charset=utf-8><title>same-source software fixture</title>",
    )
    .unwrap();
    write_web_bundle_manifest(&web_root, source, APPLICATION_CONTRACT_SHA256).unwrap();
    let web =
        Arc::new(WebDistribution::open(&web_root, source, APPLICATION_CONTRACT_SHA256).unwrap());
    let identity = ApplicationHostIdentity {
        source_sha: source.to_owned(),
        contract_sha256: APPLICATION_CONTRACT_SHA256.to_owned(),
        web_bundle_sha256: web.manifest_sha256().to_owned(),
    };
    let server = ApplicationServer::new(ApplicationSession::default(), identity).unwrap();
    let epoch = Instant::now();
    let host = HostService::new(server, epoch, temporary.join("records")).unwrap();
    let handle = host.handle();
    let mut config = HostListenConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        tls: None,
        web,
    };
    handle
        .administer(HostManagementCommand::Start(config.clone()))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let address = loop {
        if let Some(address) = handle.snapshot().listening {
            break address;
        }
        assert!(
            Instant::now() < deadline,
            "actual HTTP listener did not start"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    let url = format!("http://{address}/");
    let mut clients = [
        Client::new(epoch, &url, 100, handle.clone()),
        Client::new(epoch, &url, 200, handle.clone()),
    ];
    until(&mut clients, |clients| {
        clients.iter().all(|client| {
            client
                .view
                .as_ref()
                .is_some_and(|view| view.scene == GameUiScene::Title)
        })
    });
    let peers = handle.snapshot().application.peers;
    assert_eq!(peers.len(), 2);
    assert!(
        peers
            .iter()
            .all(|peer| peer.role == ApplicationPeerRole::Mirror)
    );
    handle
        .administer(HostManagementCommand::Application(
            ApplicationServerCommand::AssignOperator {
                peer: Some(peers[0].id),
            },
        ))
        .unwrap();
    until(&mut clients, |_| {
        handle.snapshot().application.operator == Some(peers[0].id)
    });
    // Wait for the role's View to reach its actual corresponding client.
    until(&mut clients, |clients| {
        clients.iter().any(|client| {
            client.view.as_ref().is_some_and(|view| {
                view.panels
                    .iter()
                    .flat_map(|panel| &panel.controls)
                    .any(|control| control.enabled && control.id == "game-title-start")
            })
        })
    });
    let operator = clients
        .iter()
        .position(|client| {
            client
                .view
                .as_ref()
                .unwrap()
                .panels
                .iter()
                .flat_map(|panel| &panel.controls)
                .any(|control| control.enabled && control.id == "game-title-start")
        })
        .unwrap();
    let captured = clients[operator].activate(SessionCommand::OpenSetup {});
    until(&mut clients, |clients| {
        clients.iter().all(|client| {
            client
                .view
                .as_ref()
                .is_some_and(|view| view.scene == GameUiScene::FlightSetup)
        })
    });
    let retired_view_revision = clients[operator].view.as_ref().unwrap().view_revision;
    handle
        .administer(HostManagementCommand::Application(
            ApplicationServerCommand::AssignOperator { peer: None },
        ))
        .unwrap();
    until(&mut clients, |_| {
        handle.snapshot().application.operator.is_none()
    });
    handle
        .administer(HostManagementCommand::Application(
            ApplicationServerCommand::AssignOperator {
                peer: Some(peers[0].id),
            },
        ))
        .unwrap();
    until(&mut clients, |_| {
        handle.snapshot().application.operator == Some(peers[0].id)
    });
    until(&mut clients, |clients| {
        clients[operator]
            .view
            .as_ref()
            .is_some_and(|view| view.view_revision > retired_view_revision)
            && clients[operator].can_activate(&SessionCommand::ReturnToTitle {})
    });
    let return_packet = clients[operator].activate(SessionCommand::ReturnToTitle {});
    until(&mut clients, |clients| {
        clients.iter().all(|client| {
            client
                .view
                .as_ref()
                .is_some_and(|view| view.scene == GameUiScene::Title)
        })
    });
    let mut stale: serde_json::Value = serde_json::from_str(&captured).unwrap();
    stale["sequence"] = 999u64.into();
    stale["view_revision"] = clients[operator]
        .view
        .as_ref()
        .unwrap()
        .view_revision
        .into();
    clients[operator]
        .network
        .submit(ApplicationRequest::NetworkSend {
            socket_id: clients[operator].socket_id.unwrap(),
            send_id: u64::MAX,
            payload: ApplicationNetworkData::Text {
                text: serde_json::to_string(&stale).unwrap(),
            },
        })
        .unwrap_or_else(|_| panic!("actual stale-packet send rejected by transport"));
    let guard = Instant::now() + Duration::from_millis(100);
    while Instant::now() < guard {
        for client in &mut clients {
            client.pump();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(handle.snapshot().application.scene, GameUiScene::Title);
    until(&mut clients, |clients| {
        clients[operator].can_activate(&SessionCommand::OpenSetup {})
    });
    clients[operator].activate(SessionCommand::OpenSetup {});
    until(&mut clients, |clients| {
        clients.iter().all(|client| {
            client
                .view
                .as_ref()
                .is_some_and(|view| view.scene == GameUiScene::FlightSetup)
        })
    });
    assert!(clients.iter().all(|client| client.preparations > 0));
    until(&mut clients, |clients| {
        clients[0]
            .dynamic
            .keys()
            .any(|key| clients[1].dynamic.contains_key(key))
    });
    let common: Vec<_> = clients[0]
        .dynamic
        .iter()
        .filter_map(|(key, hash)| clients[1].dynamic.get(key).map(|other| (hash, other)))
        .collect();
    assert!(!common.is_empty());
    assert!(
        common.iter().all(|(first, second)| first == second),
        "same complete buffer revision must carry identical bytes to both peers"
    );

    let mirror = 1 - operator;
    let peer_scope = clients[mirror]
        .sent
        .iter()
        .find_map(|text| {
            let value: serde_json::Value = serde_json::from_str(text).ok()?;
            value.get("peer").cloned()
        })
        .expect("actual mirror scope from its generated ACK");
    let mirror_id = peer_scope["client_id"].as_u64().unwrap();
    let generation = handle
        .snapshot()
        .application
        .peers
        .iter()
        .find(|peer| peer.id.get() == mirror_id)
        .unwrap()
        .role_generation;
    let mut unauthorized: serde_json::Value = serde_json::from_str(&return_packet).unwrap();
    unauthorized["actor"]["peer"] = peer_scope;
    unauthorized["actor"]["role_generation"] = generation.into();
    unauthorized["sequence"] = 500u64.into();
    unauthorized["view_revision"] = clients[mirror].view.as_ref().unwrap().view_revision.into();
    let mirror_socket = clients[mirror].socket_id.unwrap();
    clients[mirror]
        .network
        .submit(ApplicationRequest::NetworkSend {
            socket_id: mirror_socket,
            send_id: u64::MAX - 1,
            payload: ApplicationNetworkData::Text {
                text: serde_json::to_string(&unauthorized).unwrap(),
            },
        })
        .unwrap_or_else(|_| panic!("actual mirror transport failed"));
    let guard = Instant::now() + Duration::from_millis(100);
    while Instant::now() < guard {
        for client in &mut clients {
            client.pump();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        handle.snapshot().application.scene,
        GameUiScene::FlightSetup
    );
    // Canonical clock, input retirement and reconnection share this same actual I/O journey.
    until(&mut clients, |clients| {
        clients[operator].can_activate(&SessionCommand::Prepare {})
    });
    clients[operator].activate(SessionCommand::Prepare {});
    until(&mut clients, |clients| {
        clients[operator].can_activate(&SessionCommand::StartCountdown {})
    });
    clients[operator].activate(SessionCommand::StartCountdown {});
    until(&mut clients, |clients| {
        clients.iter().all(|client| {
            client
                .view
                .as_ref()
                .is_some_and(|view| view.scene == GameUiScene::Countdown)
        })
    });
    until(&mut clients, |clients| clients.iter().all(Client::flying));
    let launch_position = clients[operator].aircraft_position.unwrap();
    until(&mut clients, |clients| {
        clients[operator]
            .aircraft_position
            .is_some_and(|position| position != launch_position)
    });

    clients[operator].observe(ApplicationInput::Dispose);
    until(&mut clients, |clients| {
        clients[operator].lifecycle == ApplicationLifecycle::Drained
            && clients[mirror].paused()
            && handle.snapshot().application.operator.is_none()
    });
    until(&mut clients, |clients| {
        clients[mirror].paused()
            && clients[mirror].consumed_frame_revision
                == clients[mirror].view.as_ref().map(|view| view.view_revision)
    });
    let stopped = clients[mirror].aircraft_position.unwrap();
    let pause_guard = Instant::now() + Duration::from_millis(100);
    until(&mut clients, |_| Instant::now() >= pause_guard);
    assert_eq!(
        clients[mirror].aircraft_position,
        Some(stopped),
        "operator loss must stop physical advancement"
    );

    let mut reconnect = Client::new(epoch, &url, 300, handle.clone());
    reconnect.gamepads = vec![ApplicationGamepadObservation {
        identity: birdman_game_app::flight_input::FlightDeviceIdentity {
            slot_low: 0,
            slot_high: 0,
            generation: 1,
        },
        connected: true,
        mapping: "standard".into(),
        axes: vec![Some(0.7), Some(0.0), Some(0.0), Some(0.0)],
        buttons: Vec::new(),
    }];
    let mut retired = std::mem::replace(&mut clients[operator], reconnect);
    retired.network.shutdown().unwrap();
    until(&mut clients, |clients| clients[operator].paused());
    let reconnect_scope = clients[operator]
        .sent
        .iter()
        .find_map(|text| {
            let packet: serde_json::Value = serde_json::from_str(text).ok()?;
            packet.get("peer")?.get("client_id")?.as_u64()
        })
        .expect("new connection's actual accepted scope");
    let reconnect_id = handle
        .snapshot()
        .application
        .peers
        .iter()
        .find(|peer| peer.id.get() == reconnect_scope)
        .unwrap()
        .id;
    assert_ne!(reconnect_id, peers[0].id);
    handle
        .administer(HostManagementCommand::Application(
            ApplicationServerCommand::AssignOperator {
                peer: Some(reconnect_id),
            },
        ))
        .unwrap();
    until(&mut clients, |_| {
        handle.snapshot().application.operator == Some(reconnect_id)
    });
    until(&mut clients, |clients| {
        clients[operator].can_activate(&SessionCommand::Abort {})
    });
    let neutral_guard = Instant::now() + Duration::from_millis(100);
    until(&mut clients, |_| Instant::now() >= neutral_guard);
    assert!(!clients[operator].can_activate(&SessionCommand::Resume {}));
    clients[operator].control(SessionCommand::Resume {}, false);
    let blocked_guard = Instant::now() + Duration::from_millis(100);
    until(&mut clients, |_| Instant::now() >= blocked_guard);
    assert!(
        clients.iter().all(Client::paused),
        "non-neutral reconnection cannot resume flight"
    );

    clients[operator].gamepads[0].axes.fill(Some(0.0));
    until(&mut clients, |clients| {
        clients[operator].can_activate(&SessionCommand::Resume {})
    });
    assert!(
        clients.iter().all(Client::paused),
        "neutral input alone does not resume flight"
    );
    clients[operator].activate(SessionCommand::Resume {});
    until(&mut clients, |clients| clients.iter().all(Client::flying));
    until(&mut clients, |clients| {
        clients[operator]
            .aircraft_position
            .is_some_and(|position| position != stopped)
    });

    handle.administer(HostManagementCommand::Stop).unwrap();
    until(&mut clients, |_| handle.snapshot().listening.is_none());
    config.bind = address;
    handle
        .administer(HostManagementCommand::Start(config))
        .unwrap();
    until(&mut clients, |_| {
        handle.snapshot().listening == Some(address)
    });
    for client in &mut clients {
        client.observe(ApplicationInput::Dispose);
    }
    until(&mut clients, |clients| {
        clients
            .iter()
            .all(|client| client.lifecycle == ApplicationLifecycle::Drained)
    });
    for mut client in clients {
        client.network.shutdown().unwrap();
    }
    host.shutdown().unwrap();
    std::fs::remove_dir_all(&temporary).unwrap();
}
