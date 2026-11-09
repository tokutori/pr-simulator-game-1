//! The same exclusive transport composition is used by browser and native clients.
use super::*;
use birdman_game_presentation::{
    camera::Pose,
    game_ui::{GameUiControlInput, game_ui_boot_view},
    screen_render_auto::ScreenRenderScope,
};
use url::Url;

/// Shared application entry point: local authority or one remote connection and display endpoint.
pub struct ApplicationRuntime {
    // Taken only during an owned enum transition; public methods restore it before returning.
    mode: Option<Mode>,
}
enum Mode {
    Local(Box<LocalRuntime>),
    Discovering(Box<Discovery>),
    Remote {
        connection: Box<remote::RemoteSession>,
        endpoint: Box<endpoint::ApplicationEndpoint>,
    },
}
struct Discovery {
    endpoint: endpoint::ApplicationEndpoint,
    platform: ApplicationPlatformObservation,
    host: Url,
    allow_local: bool,
    stage: DiscoveryStage,
    observed_ms: f64,
    revision: u32,
    diagnostic: Option<String>,
    shutdown: shutdown::ShutdownOwner,
    cancelling_read: Option<u32>,
    cancel_wake: Option<u32>,
}
enum DiscoveryStage {
    Initial,
    Reading { request_id: u32, wake_id: u32 },
    Failed,
    Closed,
}
enum Resolution {
    Waiting,
    Local,
    Remote(wire::ApplicationHostBootstrap, String),
}

impl ApplicationRuntime {
    /// Adopts one explicit local authority; native standalone callers use this transport.
    pub fn new(
        application: ApplicationSession,
        platform: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        Ok(Self {
            mode: Some(Mode::Local(Box::new(LocalRuntime::new(
                application,
                platform,
            )?))),
        })
    }
    /// Resolves the document's same-origin host before creating a local domain session.
    pub fn discover(
        platform: ApplicationPlatformObservation,
    ) -> Result<Self, ApplicationRuntimeError> {
        let Some(location) = &platform.location else {
            return Self::new(ApplicationSession::default(), platform);
        };
        let host = host_url(&location.url)?;
        Self::discover_at(platform, host, true)
    }
    /// Connects a native or browser display to an explicitly selected HTTP(S) host.
    pub fn connect(
        platform: ApplicationPlatformObservation,
        host_url_text: &str,
    ) -> Result<Self, ApplicationRuntimeError> {
        let host = host_url(host_url_text)?;
        if platform.location.as_ref().is_some_and(|location| {
            location.secure_context
                && location.url.starts_with("https:")
                && host.scheme() != "https"
        }) {
            return Err(ApplicationRuntimeError::SpatialInput(
                "an HTTPS document requires an HTTPS host",
            ));
        }
        Self::discover_at(platform, host, false)
    }
    fn discover_at(
        platform: ApplicationPlatformObservation,
        host: Url,
        allow_local: bool,
    ) -> Result<Self, ApplicationRuntimeError> {
        let endpoint = endpoint::ApplicationEndpoint::new(
            platform.clone(),
            ScreenRenderScope::Background,
            request_ids::RequestIds::default(),
        )?;
        Ok(Self {
            mode: Some(Mode::Discovering(Box::new(Discovery {
                endpoint,
                platform,
                host,
                allow_local,
                stage: DiscoveryStage::Initial,
                observed_ms: 0.0,
                revision: 0,
                diagnostic: None,
                shutdown: shutdown::ShutdownOwner::default(),
                cancelling_read: None,
                cancel_wake: None,
            }))),
        })
    }
    /// Exposes canonical state only for an explicitly local runtime and verification consumer.
    pub fn local_application(&self) -> Option<&ApplicationSession> {
        match self.mode.as_ref().expect("runtime transition completed") {
            Mode::Local(local) => Some(local.application()),
            _ => None,
        }
    }
    fn endpoint(&self) -> &endpoint::ApplicationEndpoint {
        match self.mode.as_ref().expect("runtime transition completed") {
            Mode::Local(local) => &local.endpoint,
            Mode::Discovering(discovery) => &discovery.endpoint,
            Mode::Remote { endpoint, .. } => endpoint,
        }
    }
    /// Borrows immutable bytes from the exact consumer lease and buffer revision.
    pub fn asset_buffer(
        &self,
        lease_id: u32,
        buffer_id: u32,
        revision: u32,
    ) -> Option<ApplicationBufferSlice<'_>> {
        self.endpoint().asset_leases.buffer(
            lease_id,
            ApplicationBufferKey {
                buffer_id,
                revision,
            },
        )
    }
    /// Copies an exact immutable consumer-scoped upload.
    pub fn asset_bytes(&self, lease_id: u32, buffer_id: u32, revision: u32) -> Option<Vec<u8>> {
        self.asset_buffer(lease_id, buffer_id, revision)
            .map(|buffer| buffer.to_bytes())
    }
    /// Admits an observation and performs synchronous platform text measurement.
    pub fn update(
        &mut self,
        observation: ApplicationObservation,
        measure: &mut impl FnMut(
            &ApplicationTextProbe,
        ) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let (mut output, work) = self.begin_update(observation)?;
        if let Some(work) = work {
            match self.admit_presentation(work.project(measure)) {
                Ok(drawing) => output.drawing = drawing,
                Err(cause) => {
                    output.drawing = None;
                    output.diagnostic.get_or_insert_with(|| cause.to_string());
                }
            }
        }
        Ok(output)
    }
    /// Returns the unique presentation work without keeping an application guard during font I/O.
    pub fn begin_update(
        &mut self,
        observation: ApplicationObservation,
    ) -> Result<(ApplicationOutput, Option<PresentationWork>), ApplicationRuntimeError> {
        let mode = self.mode.take().expect("runtime transition completed");
        let (mode, result) = match mode {
            Mode::Local(mut local) => {
                let result = local.begin_update(observation);
                (Mode::Local(local), result)
            }
            Mode::Remote {
                mut connection,
                mut endpoint,
            } => {
                let result = connection.begin_update(&mut endpoint, observation);
                (
                    Mode::Remote {
                        connection,
                        endpoint,
                    },
                    result,
                )
            }
            Mode::Discovering(mut discovery) => {
                let mut requests = Vec::new();
                match discovery.observe(&observation, &mut requests) {
                    Ok(Resolution::Waiting) => {
                        let result = discovery.output(requests).map(|output| (output, None));
                        (Mode::Discovering(discovery), result)
                    }
                    Err(cause) => {
                        discovery.fail(cause.to_string());
                        let result = discovery.output(requests).map(|output| (output, None));
                        (Mode::Discovering(discovery), result)
                    }
                    Ok(Resolution::Local) => {
                        let platform = discovery.platform;
                        let mut local = LocalRuntime::with_endpoint(
                            ApplicationSession::default(),
                            discovery.endpoint,
                            platform.capabilities.replay_available,
                        );
                        let result = local
                            .begin_update(ApplicationObservation {
                                observed_ms: observation.observed_ms,
                                input: ApplicationInput::Platform {
                                    observation: platform,
                                },
                            })
                            .map(|(mut output, work)| {
                                requests.append(&mut output.requests);
                                output.requests = requests;
                                (output, work)
                            });
                        (Mode::Local(Box::new(local)), result)
                    }
                    Ok(Resolution::Remote(bootstrap, url)) => {
                        match remote::RemoteSession::new(
                            bootstrap,
                            url,
                            observation.observed_ms,
                            discovery.platform.clone(),
                        ) {
                            Ok(mut connection) => {
                                let mut endpoint = discovery.endpoint;
                                let result = connection
                                    .begin_update(
                                        &mut endpoint,
                                        ApplicationObservation {
                                            observed_ms: observation.observed_ms,
                                            input: ApplicationInput::Platform {
                                                observation: discovery.platform,
                                            },
                                        },
                                    )
                                    .map(|(mut output, work)| {
                                        requests.append(&mut output.requests);
                                        output.requests = requests;
                                        (output, work)
                                    });
                                (
                                    Mode::Remote {
                                        connection: Box::new(connection),
                                        endpoint: Box::new(endpoint),
                                    },
                                    result,
                                )
                            }
                            Err(cause) => {
                                discovery.fail(cause.to_string());
                                let result =
                                    discovery.output(requests).map(|output| (output, None));
                                (Mode::Discovering(discovery), result)
                            }
                        }
                    }
                }
            }
        };
        self.mode = Some(mode);
        result
    }
    /// Admits a measured surface only into the same display and transport lifetime.
    pub fn admit_presentation(
        &mut self,
        result: PresentationResult,
    ) -> Result<Option<ApplicationDrawingOutput>, ApplicationRuntimeError> {
        match self.mode.as_mut().expect("runtime transition completed") {
            Mode::Local(local) => local.admit_presentation(result),
            Mode::Remote {
                connection,
                endpoint,
            } => connection.admit_presentation(endpoint, result),
            Mode::Discovering(_) => Err(ApplicationRuntimeError::SpatialInput(
                "discovery has no spatial presentation work",
            )),
        }
    }
}
fn host_url(raw: &str) -> Result<Url, ApplicationRuntimeError> {
    let url = Url::parse(raw)
        .map_err(|_| ApplicationRuntimeError::SpatialInput("host URL is invalid"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ApplicationRuntimeError::SpatialInput(
            "host must be an HTTP(S) origin without credentials",
        ));
    }
    Ok(url)
}
impl Discovery {
    fn fail(&mut self, cause: String) {
        self.endpoint.boot.fail(Some(&cause));
        self.diagnostic = Some(cause);
        self.stage = DiscoveryStage::Failed;
    }
    fn cancel(&mut self, requests: &mut Vec<ApplicationRequest>) {
        if let DiscoveryStage::Reading {
            request_id,
            wake_id,
        } = self.stage
        {
            self.cancelling_read = Some(request_id);
            requests.push(ApplicationRequest::CancelHostRead { request_id });
            requests.push(ApplicationRequest::CancelWake {
                request_id: wake_id,
            });
        }
    }
    fn observe(
        &mut self,
        observation: &ApplicationObservation,
        requests: &mut Vec<ApplicationRequest>,
    ) -> Result<Resolution, ApplicationRuntimeError> {
        if !observation.observed_ms.is_finite() || observation.observed_ms < self.observed_ms {
            return Err(ApplicationRuntimeError::InvalidTimestamp);
        }
        self.observed_ms = observation.observed_ms;
        if matches!(&observation.input,ApplicationInput::HostRead {request_id,..} if Some(*request_id)==self.cancelling_read)
        {
            self.cancelling_read = None;
            if let Some(request_id) = self.cancel_wake.take() {
                requests.push(ApplicationRequest::CancelWake { request_id });
            }
        }
        let admitted = self.shutdown.observe(&observation.input);
        self.endpoint.observe_presentation(
            &observation.input,
            self.observed_ms,
            ScreenRenderScope::Background,
            requests,
        )?;
        self.endpoint.observe_terminal(&observation.input, admitted);
        if matches!(&observation.input,ApplicationInput::Wake {request_id} if Some(*request_id)==self.cancel_wake)
        {
            self.cancel_wake = None;
            if let Some(request_id) = self.cancelling_read {
                requests.push(ApplicationRequest::CancelHostRead { request_id });
            }
        }
        match &observation.input {
            ApplicationInput::Platform { observation } => {
                self.platform = observation.clone();
            }
            ApplicationInput::HostRead {
                request_id,
                status,
                content_type,
                bytes,
                failure,
            } => {
                if let DiscoveryStage::Reading {
                    request_id: expected,
                    wake_id,
                } = self.stage
                    && *request_id == expected
                {
                    requests.push(ApplicationRequest::CancelWake {
                        request_id: wake_id,
                    });
                    if *status == 404 && self.allow_local {
                        return Ok(Resolution::Local);
                    }
                    if let Some(cause) = failure {
                        self.fail(cause.clone());
                        return Ok(Resolution::Waiting);
                    }
                    if *status != 200
                        || bytes.len() > APPLICATION_NETWORK_SERVER_TEXT_BYTES
                        || !content_type.as_deref().is_some_and(|value| {
                            value.split(';').next().is_some_and(|kind| {
                                kind.trim().eq_ignore_ascii_case("application/json")
                            })
                        })
                    {
                        self.fail(format!("Host discovery response rejected: HTTP {status}"));
                        return Ok(Resolution::Waiting);
                    }
                    let bootstrap: wire::ApplicationHostBootstrap = serde_json::from_slice(bytes)
                        .map_err(|_| {
                        ApplicationRuntimeError::SpatialInput("host descriptor JSON is invalid")
                    })?;
                    let mut websocket =
                        self.host.join(&bootstrap.websocket_path).map_err(|_| {
                            ApplicationRuntimeError::SpatialInput("host WebSocket path is invalid")
                        })?;
                    websocket
                        .set_scheme(if self.host.scheme() == "https" {
                            "wss"
                        } else {
                            "ws"
                        })
                        .map_err(|_| {
                            ApplicationRuntimeError::SpatialInput(
                                "host WebSocket scheme is invalid",
                            )
                        })?;
                    return Ok(Resolution::Remote(bootstrap, websocket.into()));
                }
            }
            ApplicationInput::Wake { request_id } => {
                if matches!(self.stage,DiscoveryStage::Reading {wake_id,..} if wake_id==*request_id)
                {
                    self.cancel(requests);
                    self.fail("Host discovery timed out".into());
                }
            }
            ApplicationInput::Control {
                view_revision,
                input: GameUiControlInput::Activate { control_id },
            } if *view_revision == self.revision
                && matches!(self.stage, DiscoveryStage::Failed) =>
            {
                if control_id == "game-boot-retry" && self.cancelling_read.is_none() {
                    self.endpoint.retry_boot(requests);
                    self.stage = DiscoveryStage::Initial;
                } else if control_id == "game-boot-reload" {
                    self.shutdown.request_reload();
                    self.close(requests);
                }
            }
            ApplicationInput::Dispose | ApplicationInput::PageHide { persisted: false } => {
                self.close(requests);
            }
            _ => {}
        }
        if matches!(self.stage, DiscoveryStage::Initial) {
            let request_id = self.endpoint.allocate_request()?;
            let wake_id = self.endpoint.allocate_request()?;
            requests.push(ApplicationRequest::ReadHost {
                request_id,
                url: self
                    .host
                    .join("/api/host")
                    .map_err(|_| ApplicationRuntimeError::SpatialInput("host URL is invalid"))?
                    .into(),
                accept: "application/json".into(),
                maximum_bytes: APPLICATION_NETWORK_SERVER_TEXT_BYTES as u32,
            });
            requests.push(ApplicationRequest::ScheduleWake {
                request_id: wake_id,
                at_ms: self.observed_ms + wire::CONNECTION_TIMEOUT_MS,
                realtime: false,
            });
            self.stage = DiscoveryStage::Reading {
                request_id,
                wake_id,
            };
        }
        Ok(Resolution::Waiting)
    }
    fn close(&mut self, requests: &mut Vec<ApplicationRequest>) {
        self.cancel(requests);
        let renderer = self.endpoint.close_presentation(requests);
        requests.extend(self.shutdown.begin(renderer));
        self.stage = DiscoveryStage::Closed;
    }
    fn output(
        &mut self,
        mut requests: Vec<ApplicationRequest>,
    ) -> Result<ApplicationOutput, ApplicationRuntimeError> {
        let mut view = self.endpoint.boot.view(
            game_ui_boot_view(self.endpoint.platform.viewport)
                .map_err(ApplicationRuntimeError::Ui)?,
        );
        if self.cancelling_read.is_some() {
            for control in view
                .panels
                .iter_mut()
                .flat_map(|panel| panel.controls.iter_mut())
            {
                if control.id == "game-boot-retry" {
                    control.enabled = false;
                }
            }
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(ApplicationRuntimeError::PresentationExhausted)?;
        view.view_revision = self.revision;
        let view = Arc::new(view);
        self.endpoint.retained = Some(Arc::clone(&view));
        let drawing = ApplicationDrawingOutput {
            asset_lease: None,
            display: Default::default(),
            presentation: ApplicationDrawingPresentation {
                frame_rate_label: String::new(),
                mode: Some(GameUiPresentationMode::Screen),
                screen_ui_visible: !matches!(self.stage, DiscoveryStage::Closed),
                screen_hud_visible: false,
                scene_visible: false,
            },
            graphics: self
                .endpoint
                .graphics
                .projection(false, self.observed_ms)
                .map_err(ApplicationRuntimeError::Graphics)?,
            scene: ApplicationSceneProjection {
                lighting: None,
                reflection: None,
                venue_visible: false,
                water_world_from_surface: Pose::IDENTITY,
                geometry: None,
                water_time_seconds: 0.0,
                world_from_aircraft: Pose::IDENTITY,
                camera: None,
            },
            spatial: None,
            ui: ApplicationUiProjection::View { view },
        };
        if self.cancelling_read.is_some() && self.cancel_wake.is_none() {
            let request_id = self.endpoint.allocate_request()?;
            self.cancel_wake = Some(request_id);
            requests.push(ApplicationRequest::ScheduleWake {
                request_id,
                at_ms: self.observed_ms + wire::HEARTBEAT_INTERVAL_MS,
                realtime: false,
            });
        }
        self.shutdown.requests(&requests);
        if let Some(request) = self.shutdown.release_renderer() {
            self.shutdown.requests(std::slice::from_ref(&request));
            requests.push(request);
        }
        let mut lifecycle = self.shutdown.lifecycle();
        if lifecycle == ApplicationLifecycle::Drained && self.cancelling_read.is_some() {
            lifecycle = ApplicationLifecycle::Closing;
        }
        if lifecycle == ApplicationLifecycle::Drained
            && let Some(request) = self.shutdown.take_reload()
        {
            requests.push(request);
        }
        Ok(ApplicationOutput {
            drawing: Some(drawing),
            requests,
            keyboard_consumed: false,
            lifecycle,
            diagnostic: self.diagnostic.take(),
        })
    }
}
