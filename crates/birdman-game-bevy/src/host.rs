//! Bevy management controls and in-process raw transport for the shared host.

use super::NativeFont;
use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use birdman_game_host::{
    server_cli::ServerOptions,
    service::{HostListenConfig, HostManagementCommand, HostPeer, HostService, HostServiceHandle},
};
use birdman_game_runtime::{
    ApplicationInput, ApplicationNetworkData, ApplicationNetworkMessage, ApplicationNetworkPayload,
    ApplicationPeerId, ApplicationRequest, ApplicationServerCommand, ApplicationTransmission,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
    time::Instant,
};
use tokio::sync::oneshot;

#[derive(Resource)]
pub(crate) struct NativeHost(Mutex<State>);

struct Socket {
    opening: Option<oneshot::Receiver<Result<HostPeer, String>>>,
    peer: Option<HostPeer>,
    acknowledgement: Option<(u64, Result<(), String>)>,
    pending_transmission: Option<ApplicationTransmission>,
    closing: Option<(u16, String)>,
    accepted: VecDeque<(usize, oneshot::Receiver<()>)>,
    maximum_text: usize,
    maximum_binary: usize,
}

struct State {
    service: Option<HostService>,
    handle: HostServiceHandle,
    config: HostListenConfig,
    sockets: BTreeMap<u32, Socket>,
    diagnostic: Option<String>,
    expanded: bool,
    panel_signature: String,
    initial_operator: bool,
}

impl NativeHost {
    pub(crate) fn start(
        options: ServerOptions,
        epoch: Instant,
        source: &str,
    ) -> Result<Self, String> {
        let (service, config) = HostService::start(options, source, epoch)?;
        Ok(Self(Mutex::new(State {
            handle: service.handle(),
            service: Some(service),
            config,
            sockets: BTreeMap::new(),
            diagnostic: None,
            expanded: true,
            panel_signature: String::new(),
            initial_operator: true,
        })))
    }

    pub(crate) fn local_origin(&self) -> String {
        let state = self.0.lock().expect("Native host poisoned");
        format!(
            "{}://localhost:{}/",
            if state.config.tls.is_some() {
                "https"
            } else {
                "http"
            },
            state.config.bind.port()
        )
    }

    pub(crate) fn shutdown(&self) -> Result<(), String> {
        let service = self.0.lock().expect("Native host poisoned").service.take();
        service.map_or(Ok(()), HostService::shutdown)
    }
}

pub(crate) fn install(app: &mut App) {
    app.add_systems(Update, update_panel);
}

/// Execute only raw host transport work; canonical policy remains in Runtime.
pub(crate) fn execute(
    world: &mut World,
    request: &ApplicationRequest,
) -> Result<Option<Vec<ApplicationInput>>, String> {
    let Some(host) = world.get_resource::<NativeHost>() else {
        return Ok(None);
    };
    let mut state = host.0.lock().expect("Native host poisoned");
    let mut inputs = Vec::new();
    match request {
        ApplicationRequest::ReadHost {
            request_id,
            maximum_bytes,
            ..
        } => {
            inputs.push(ApplicationInput::HostRead {
                request_id: *request_id,
                status: 200,
                content_type: Some("application/json".into()),
                bytes: state
                    .handle
                    .bootstrap_bytes()
                    .iter()
                    .take(*maximum_bytes as usize + 1)
                    .copied()
                    .collect(),
                failure: None,
            });
        }
        ApplicationRequest::CancelHostRead { .. } => {}
        ApplicationRequest::NetworkOpen {
            socket_id,
            maximum_text_bytes,
            maximum_binary_bytes,
            ..
        } => {
            if state.sockets.contains_key(socket_id)
                || state.sockets.len() >= birdman_game_runtime::APPLICATION_NETWORK_MAX_SOCKETS
            {
                inputs.push(ApplicationInput::NetworkOpened {
                    socket_id: *socket_id,
                    failure: Some("In-process socket slot is unavailable".into()),
                });
            } else {
                let opening = state.handle.request_local_peer()?;
                state.sockets.insert(
                    *socket_id,
                    Socket {
                        opening: Some(opening),
                        peer: None,
                        acknowledgement: None,
                        pending_transmission: None,
                        closing: None,
                        accepted: VecDeque::new(),
                        maximum_text: *maximum_text_bytes as usize,
                        maximum_binary: *maximum_binary_bytes as usize,
                    },
                );
            }
        }
        ApplicationRequest::NetworkSend {
            socket_id,
            send_id,
            payload,
        } => {
            let handle = state.handle.clone();
            let socket = state.sockets.get_mut(socket_id);
            let ready_state = socket.as_deref().map(socket_state);
            let mut buffered_amount = 0.0;
            let result = match socket {
                Some(socket) if socket.closing.is_none() && socket.peer.is_some() => {
                    let raw = match payload {
                        ApplicationNetworkData::Text { text } => {
                            ApplicationNetworkMessage::Text(text.clone())
                        }
                        ApplicationNetworkData::Binary { bytes } => {
                            ApplicationNetworkMessage::Binary(bytes.clone())
                        }
                    };
                    let length = match payload {
                        ApplicationNetworkData::Text { text } => text.len(),
                        ApplicationNetworkData::Binary { bytes } => bytes.len(),
                    };
                    let result = handle
                        .try_receive_observed(socket.peer.as_ref().expect("open peer").id, raw)
                        .map(|receipt| socket.accepted.push_back((length, receipt)))
                        .map_err(|_| "Host receive queue is unavailable".to_owned());
                    buffered_amount = socket
                        .accepted
                        .iter()
                        .map(|(bytes, _)| *bytes)
                        .sum::<usize>() as f64;
                    result
                }
                _ => Err("In-process socket is not open".to_owned()),
            };
            inputs.push(ApplicationInput::NetworkSendObserved {
                socket_id: *socket_id,
                send_id: *send_id,
                queued: result.is_ok(),
                ready_state,
                buffered_amount,
                cause: result.err(),
            });
        }
        ApplicationRequest::ObserveNetwork { socket_id } => {
            inputs.push(ApplicationInput::NetworkObserved {
                socket_id: *socket_id,
                ready_state: state.sockets.get(socket_id).map_or(3, socket_state),
                buffered_amount: state.sockets.get(socket_id).map_or(0, |socket| {
                    socket
                        .accepted
                        .iter()
                        .map(|(bytes, _)| *bytes)
                        .sum::<usize>()
                }) as f64,
            });
        }
        ApplicationRequest::NetworkClose {
            socket_id,
            code,
            reason,
        } => {
            let handle = state.handle.clone();
            let result = if let Some(socket) = state.sockets.get_mut(socket_id) {
                socket.closing = Some((*code, reason.clone()));
                socket
                    .peer
                    .as_ref()
                    .map_or(Ok(()), |peer| handle.try_disconnect(peer.id))
            } else {
                Ok(())
            };
            inputs.push(ApplicationInput::NetworkCloseObserved {
                socket_id: *socket_id,
                present: state.sockets.contains_key(socket_id),
                requested: result.is_ok(),
                ready_state: state.sockets.get(socket_id).map(socket_state),
                cause: result.err(),
            });
        }
        _ => return Ok(None),
    }
    Ok(Some(inputs))
}

fn socket_state(socket: &Socket) -> u8 {
    if socket.closing.is_some() {
        2
    } else if socket.peer.is_some() {
        1
    } else {
        0
    }
}

/// Admit finite raw completions before acknowledging the Host's retained bytes.
pub(crate) fn poll(world: &mut World, mut admit: impl FnMut(&mut World, ApplicationInput)) {
    let Some(host) = world.get_resource::<NativeHost>() else {
        return;
    };
    let ids: Vec<_> = host
        .0
        .lock()
        .expect("Native host poisoned")
        .sockets
        .keys()
        .copied()
        .collect();
    let mut count = 0;
    let mut byte_budget = birdman_game_runtime::APPLICATION_NETWORK_BATCH_BYTES;
    for socket_id in ids {
        if count >= birdman_game_runtime::APPLICATION_NETWORK_BATCH_MESSAGES {
            break;
        }
        let (inputs, acknowledgement) = {
            let host = world.resource::<NativeHost>();
            let mut state = host.0.lock().expect("Native host poisoned");
            poll_socket(&mut state, socket_id, &mut byte_budget)
        };
        for input in inputs {
            admit(world, input);
            count += 1;
        }
        if let Some((peer, id, result)) = acknowledgement {
            let mut state = world
                .resource::<NativeHost>()
                .0
                .lock()
                .expect("Native host poisoned");
            if let Err(pending) = state.handle.try_transmitted(peer, id, result)
                && let Some(socket) = state.sockets.get_mut(&socket_id)
            {
                socket.acknowledgement = Some(pending);
            }
        }
    }
}

type TransportAcknowledgement = (ApplicationPeerId, u64, Result<(), String>);

fn poll_socket(
    state: &mut State,
    socket_id: u32,
    byte_budget: &mut usize,
) -> (Vec<ApplicationInput>, Option<TransportAcknowledgement>) {
    let mut inputs = Vec::new();
    let mut acknowledgement = None;
    let handle = state.handle.clone();
    let Some(socket) = state.sockets.get_mut(&socket_id) else {
        return (inputs, None);
    };
    let previous_bytes: usize = socket.accepted.iter().map(|(bytes, _)| *bytes).sum();
    socket.accepted.retain_mut(|(_, receipt)| {
        matches!(receipt.try_recv(), Err(oneshot::error::TryRecvError::Empty))
    });
    let buffered_bytes: usize = socket.accepted.iter().map(|(bytes, _)| *bytes).sum();
    if previous_bytes != buffered_bytes {
        inputs.push(ApplicationInput::NetworkObserved {
            socket_id,
            ready_state: socket_state(socket),
            buffered_amount: buffered_bytes as f64,
        });
    }
    if let Some(opening) = &mut socket.opening {
        match opening.try_recv() {
            Ok(result) => {
                socket.opening = None;
                match result {
                    Ok(peer) => {
                        socket.peer = Some(peer);
                        inputs.push(ApplicationInput::NetworkOpened {
                            socket_id,
                            failure: None,
                        });
                    }
                    Err(cause) => inputs.push(ApplicationInput::NetworkOpened {
                        socket_id,
                        failure: Some(cause),
                    }),
                }
            }
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(error) => {
                socket.opening = None;
                inputs.push(ApplicationInput::NetworkOpened {
                    socket_id,
                    failure: Some(error.to_string()),
                });
            }
        }
    }
    if let Some(peer) = &mut socket.peer {
        if state.initial_operator
            && socket.closing.is_none()
            && handle
                .snapshot()
                .application
                .peers
                .iter()
                .any(|item| item.id == peer.id && item.ready)
        {
            match handle.administer(HostManagementCommand::Application(
                ApplicationServerCommand::AssignOperator {
                    peer: Some(peer.id),
                },
            )) {
                Ok(()) => state.initial_operator = false,
                Err(cause) => state.diagnostic = Some(cause),
            }
        }
        if let Some((id, result)) = socket.acknowledgement.take() {
            acknowledgement = Some((peer.id, id, result));
        } else if let Some(transmission) = socket
            .pending_transmission
            .take()
            .or_else(|| peer.transmissions.try_recv().ok())
        {
            let length = match &transmission.payload {
                ApplicationNetworkPayload::Text(text) => text.len(),
                ApplicationNetworkPayload::Binary(bytes) => bytes.len(),
                ApplicationNetworkPayload::Close { reason } => reason.len(),
            };
            if length > *byte_budget {
                socket.pending_transmission = Some(transmission);
                return (inputs, None);
            }
            *byte_budget -= length;
            let id = transmission.id;
            let mut result = Ok(());
            match transmission.payload {
                ApplicationNetworkPayload::Text(text) => {
                    let overflow = text.len() > socket.maximum_text;
                    let mut end = text.len().min(socket.maximum_text.saturating_add(1));
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    inputs.push(ApplicationInput::NetworkMessage {
                        socket_id,
                        payload: ApplicationNetworkData::Text {
                            text: text[..end].to_owned(),
                        },
                        overflow,
                    });
                }
                ApplicationNetworkPayload::Binary(bytes) => {
                    inputs.push(ApplicationInput::NetworkMessage {
                        socket_id,
                        payload: ApplicationNetworkData::Binary {
                            bytes: bytes
                                .iter()
                                .take(socket.maximum_binary.saturating_add(1))
                                .copied()
                                .collect(),
                        },
                        overflow: bytes.len() > socket.maximum_binary,
                    });
                }
                ApplicationNetworkPayload::Close { reason } => {
                    socket.closing = Some((1000, reason));
                    result = handle.try_disconnect(peer.id);
                }
            }
            acknowledgement = Some((peer.id, id, result));
        }
        if socket.closing.is_some()
            && let Err(cause) = handle.try_disconnect(peer.id)
        {
            state.diagnostic = Some(cause);
        }
        if !matches!(
            peer.disconnected.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ) {
            let (code, reason) = socket
                .closing
                .take()
                .unwrap_or((1006, "Host peer retired".into()));
            inputs.push(ApplicationInput::NetworkClosed {
                socket_id,
                code,
                reason,
                was_clean: code == 1000,
            });
            state.sockets.remove(&socket_id);
        }
    } else if socket.opening.is_none() {
        let closed = socket.closing.take();
        if let Some((code, reason)) = closed {
            inputs.push(ApplicationInput::NetworkClosed {
                socket_id,
                code,
                reason,
                was_clean: true,
            });
        }
        state.sockets.remove(&socket_id);
    }
    (inputs, acknowledgement)
}

#[derive(Component)]
struct HostPanel;
#[derive(Component, Clone)]
enum ManagementButton {
    Toggle,
    Start,
    Stop,
    Pause,
    Operator(Option<ApplicationPeerId>),
    Disconnect(ApplicationPeerId),
}

fn update_panel(world: &mut World) {
    if !world.contains_resource::<NativeHost>() || !world.contains_resource::<NativeFont>() {
        return;
    }
    let pressed: Vec<_> = world
        .query_filtered::<(&Interaction, &ManagementButton), Changed<Interaction>>()
        .iter(world)
        .filter(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, button)| button.clone())
        .collect();
    let (expanded, snapshot, diagnostic) = {
        let mut state = world
            .resource::<NativeHost>()
            .0
            .lock()
            .expect("Native host poisoned");
        for button in pressed {
            let command = match button {
                ManagementButton::Toggle => {
                    state.expanded = !state.expanded;
                    None
                }
                ManagementButton::Start => Some(HostManagementCommand::Start(state.config.clone())),
                ManagementButton::Stop => Some(HostManagementCommand::Stop),
                ManagementButton::Pause => Some(HostManagementCommand::Application(
                    ApplicationServerCommand::Pause,
                )),
                ManagementButton::Operator(peer) => Some(HostManagementCommand::Application(
                    ApplicationServerCommand::AssignOperator { peer },
                )),
                ManagementButton::Disconnect(peer) => Some(HostManagementCommand::Application(
                    ApplicationServerCommand::Disconnect { peer },
                )),
            };
            if let Some(command) = command {
                state.diagnostic = state.handle.administer(command).err();
            }
        }
        let snapshot = state.handle.snapshot();
        let signature = format!(
            "{}:{:?}:{}:{}:{:?}:{:?}:{:?}",
            state.expanded,
            snapshot.listening,
            snapshot.starting,
            snapshot.https,
            snapshot.application.peers,
            snapshot.application.operator,
            (
                &state.diagnostic,
                &snapshot.diagnostic,
                &snapshot.application.diagnostic
            )
        );
        if signature == state.panel_signature {
            return;
        }
        state.panel_signature = signature.clone();
        (state.expanded, snapshot, state.diagnostic.clone())
    };
    let old: Vec<_> = world
        .query_filtered::<Entity, With<HostPanel>>()
        .iter(world)
        .collect();
    for entity in old {
        world.despawn(entity);
    }
    let font = world.resource::<NativeFont>().0.clone();
    let root = world
        .spawn((
            HostPanel,
            Node {
                position_type: PositionType::Absolute,
                right: px(8),
                top: px(8),
                width: px(300),
                padding: UiRect::all(px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.04, 0.06, 0.96)),
            GlobalZIndex(100),
        ))
        .id();
    panel_button(
        world,
        root,
        &font,
        if expanded {
            "配信管理を閉じる"
        } else {
            "配信管理"
        },
        ManagementButton::Toggle,
    );
    if !expanded {
        return;
    }
    let address = snapshot.listening.map_or_else(
        || {
            if snapshot.starting {
                "配信を準備中".into()
            } else {
                "配信を停止中".into()
            }
        },
        |address| {
            if address.ip().is_unspecified() {
                format!("待受: {address}（接続にはPCのLANアドレスを使用）")
            } else {
                format!(
                    "{}://{address}/",
                    if snapshot.https { "https" } else { "http" }
                )
            }
        },
    );
    panel_text(world, root, &font, address);
    if snapshot.listening.is_some() {
        panel_button(world, root, &font, "配信を停止", ManagementButton::Stop);
    } else if !snapshot.starting {
        panel_button(world, root, &font, "配信を開始", ManagementButton::Start);
    }
    panel_button(
        world,
        root,
        &font,
        "飛行を一時停止",
        ManagementButton::Pause,
    );
    for peer in snapshot.application.peers {
        let operator = snapshot.application.operator == Some(peer.id);
        panel_text(
            world,
            root,
            &font,
            format!(
                "接続 {} · {}",
                peer.id,
                if operator { "操作担当" } else { "閲覧" }
            ),
        );
        if !operator && peer.ready {
            panel_button(
                world,
                root,
                &font,
                "操作を許可",
                ManagementButton::Operator(Some(peer.id)),
            );
        }
        panel_button(
            world,
            root,
            &font,
            "切断",
            ManagementButton::Disconnect(peer.id),
        );
    }
    if snapshot.application.operator.is_some() {
        panel_button(
            world,
            root,
            &font,
            "操作権を解除",
            ManagementButton::Operator(None),
        );
    }
    if let Some(cause) = diagnostic
        .or(snapshot.diagnostic)
        .or(snapshot.application.diagnostic)
    {
        panel_text(world, root, &font, cause);
    }
}

fn panel_text(world: &mut World, parent: Entity, font: &Handle<Font>, text: String) {
    world.spawn((
        ChildOf(parent),
        Text::new(text),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}
fn panel_button(
    world: &mut World,
    parent: Entity,
    font: &Handle<Font>,
    text: &str,
    command: ManagementButton,
) {
    let button = world
        .spawn((
            ChildOf(parent),
            Button,
            command,
            Node {
                padding: UiRect::axes(px(8), px(5)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.22, 0.3)),
        ))
        .id();
    panel_text(world, button, font, text.to_owned());
}
