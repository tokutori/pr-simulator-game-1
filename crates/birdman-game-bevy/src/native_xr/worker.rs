use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderInstance};
use bevy_mod_openxr::{resources::OxrFrameWaiter, session::OxrSession};
use birdman_game_app::application::XrVisibility;
use birdman_game_presentation::{camera, xr::*};
use birdman_game_runtime::{ApplicationInput, ApplicationXrFrameObservation, DisplayCompletion};
use std::{sync::mpsc, thread, time::Duration};

pub(super) enum Command {
    Start(u32),
    Stop(u32),
    Release,
    LossPending(String),
    Drawn { frame: u64 },
}

#[derive(Clone)]
pub(super) struct Frame {
    pub id: u64,
    pub time: openxr::Time,
    pub views: Vec<openxr::View>,
    pub base: std::sync::Arc<openxr::Space>,
    pub should_render: bool,
    pub xr: ApplicationXrFrameObservation,
    pub sources: Vec<XrInputSourceObservation>,
}

pub(super) enum Event {
    Input(ApplicationInput),
    Frame(Box<Frame>),
    Stopped,
    LossPending,
    Fatal(String),
    Released,
}

struct Actions {
    set: openxr::ActionSet,
    aim: openxr::Action<openxr::Posef>,
    select: openxr::Action<bool>,
    squeeze: openxr::Action<bool>,
    stick: openxr::Action<openxr::Vector2f>,
    pad: openxr::Action<openxr::Vector2f>,
    touch: openxr::Action<bool>,
    hands: [openxr::Path; 2],
    spaces: [openxr::Space; 2],
}

impl Actions {
    fn new(session: &OxrSession) -> Result<Self, String> {
        let instance = session.instance();
        let hands =
            ["/user/hand/left", "/user/hand/right"].map(|path| instance.string_to_path(path));
        let hands = [hands[0].map_err(error)?, hands[1].map_err(error)?];
        let set = instance
            .create_action_set("birdman_input", "Birdman controller input", 0)
            .map_err(error)?;
        let aim = set
            .create_action("aim", "Aim pose", &hands)
            .map_err(error)?;
        let select = set
            .create_action("select", "Primary action", &hands)
            .map_err(error)?;
        let squeeze = set
            .create_action("squeeze", "Squeeze", &hands)
            .map_err(error)?;
        let stick = set
            .create_action("stick", "Thumbstick", &hands)
            .map_err(error)?;
        let pad = set
            .create_action("pad", "Touchpad", &hands)
            .map_err(error)?;
        let touch = set
            .create_action("touch", "Touchpad contact", &hands)
            .map_err(error)?;
        let mut bound = false;
        for (profile, primary, secondary, thumbstick, touchpad) in [
            (
                "/interaction_profiles/khr/simple_controller",
                "select/click",
                None,
                false,
                false,
            ),
            (
                "/interaction_profiles/oculus/touch_controller",
                "trigger/value",
                Some("squeeze/value"),
                true,
                false,
            ),
            (
                "/interaction_profiles/valve/index_controller",
                "trigger/value",
                Some("squeeze/value"),
                true,
                true,
            ),
            (
                "/interaction_profiles/microsoft/motion_controller",
                "trigger/value",
                Some("squeeze/click"),
                true,
                true,
            ),
            (
                "/interaction_profiles/htc/vive_controller",
                "trigger/value",
                Some("squeeze/click"),
                false,
                true,
            ),
        ] {
            let mut bindings = Vec::new();
            for hand in ["left", "right"] {
                let path = |component: &str| {
                    instance.string_to_path(&format!("/user/hand/{hand}/input/{component}"))
                };
                bindings.push(openxr::Binding::new(&aim, path("aim/pose").map_err(error)?));
                bindings.push(openxr::Binding::new(&select, path(primary).map_err(error)?));
                if let Some(secondary) = secondary {
                    bindings.push(openxr::Binding::new(
                        &squeeze,
                        path(secondary).map_err(error)?,
                    ));
                }
                if thumbstick {
                    bindings.push(openxr::Binding::new(
                        &stick,
                        path("thumbstick").map_err(error)?,
                    ));
                }
                if touchpad {
                    bindings.push(openxr::Binding::new(&pad, path("trackpad").map_err(error)?));
                    bindings.push(openxr::Binding::new(
                        &touch,
                        path("trackpad/touch").map_err(error)?,
                    ));
                }
            }
            match instance.suggest_interaction_profile_bindings(
                instance.string_to_path(profile).map_err(error)?,
                &bindings,
            ) {
                Ok(()) => bound = true,
                Err(openxr::sys::Result::ERROR_PATH_UNSUPPORTED) => {}
                Err(cause) => return Err(error(cause)),
            }
        }
        if !bound {
            return Err("OpenXR runtime accepted no controller interaction profile".into());
        }
        session.attach_action_sets(&[&set]).map_err(error)?;
        let left = aim
            .create_space(session, hands[0], openxr::Posef::IDENTITY)
            .map_err(error)?;
        let right = aim
            .create_space(session, hands[1], openxr::Posef::IDENTITY)
            .map_err(error)?;
        Ok(Self {
            set,
            aim,
            select,
            squeeze,
            stick,
            pad,
            touch,
            hands,
            spaces: [left, right],
        })
    }

    fn observe(
        &self,
        session: &OxrSession,
        base: &openxr::Space,
        time: openxr::Time,
    ) -> openxr::Result<Vec<XrInputSourceObservation>> {
        session.sync_actions(&[openxr::ActiveActionSet::new(&self.set)])?;
        self.hands
            .iter()
            .enumerate()
            .map(|(index, &hand)| {
                let select = self.select.state(session, hand)?;
                let squeeze = self.squeeze.state(session, hand)?;
                let stick = self.stick.state(session, hand)?;
                let pad = self.pad.state(session, hand)?;
                let touch = self.touch.state(session, hand)?;
                let active = self.aim.is_active(session, hand)?;
                let ray = if active {
                    let location = self.spaces[index].locate(base, time)?;
                    valid_space(location.location_flags).then(|| pose(location.pose))
                } else {
                    None
                };
                Ok(XrInputSourceObservation {
                    source_id: index as u32 + 1,
                    target_ray_mode: "tracked-pointer".into(),
                    handedness: if index == 0 { "left" } else { "right" }.into(),
                    tracking_from_ray: ray,
                    select_pressed: select.is_active.then_some(select.current_state),
                    gamepad: Some(XrInputGamepadObservation {
                        mapping: "xr-standard".into(),
                        axes: vec![
                            pad.is_active.then_some(f64::from(pad.current_state.x)),
                            pad.is_active.then_some(f64::from(pad.current_state.y)),
                            stick.is_active.then_some(f64::from(stick.current_state.x)),
                            stick.is_active.then_some(f64::from(stick.current_state.y)),
                        ],
                        buttons: vec![
                            XrInputButtonObservation {
                                pressed: select.is_active && select.current_state,
                                touched: false,
                            },
                            XrInputButtonObservation {
                                pressed: squeeze.is_active && squeeze.current_state,
                                touched: false,
                            },
                            XrInputButtonObservation {
                                pressed: false,
                                touched: touch.is_active && touch.current_state,
                            },
                        ],
                    }),
                })
            })
            .collect()
    }
}

fn error(cause: impl std::fmt::Display) -> String {
    format!("OpenXR: {cause}")
}

fn report_status(
    events: &mpsc::SyncSender<Event>,
    id: u32,
    status: openxr::sys::Result,
) -> Result<(), String> {
    if matches!(
        status,
        openxr::sys::Result::SESSION_LOSS_PENDING | openxr::sys::Result::ERROR_SESSION_LOST
    ) {
        events.send(Event::LossPending).map_err(error)?;
        events
            .send(Event::Input(ApplicationInput::DisplayEnded {
                resource_id: id,
                cause: Some(error(status)),
            }))
            .map_err(error)?;
    }
    Ok(())
}

pub(super) fn pose(value: openxr::Posef) -> camera::Pose {
    camera::Pose {
        position: camera::Vector3 {
            x: f64::from(value.position.x),
            y: f64::from(value.position.y),
            z: f64::from(value.position.z),
        },
        orientation: camera::Quaternion {
            w: f64::from(value.orientation.w),
            x: f64::from(value.orientation.x),
            y: f64::from(value.orientation.y),
            z: f64::from(value.orientation.z),
        },
    }
}

fn valid_space(flags: openxr::SpaceLocationFlags) -> bool {
    flags.contains(
        openxr::SpaceLocationFlags::ORIENTATION_VALID | openxr::SpaceLocationFlags::POSITION_VALID,
    )
}

fn visibility(state: openxr::SessionState) -> (XrVisibility, bool) {
    match state {
        openxr::SessionState::FOCUSED => (XrVisibility::Visible, true),
        openxr::SessionState::VISIBLE => (XrVisibility::Visible, false),
        _ => (XrVisibility::Hidden, false),
    }
}

/// The worker owns only wait/event/input I/O. Queue-touching calls run on Bevy's Render thread.
pub(super) fn run(
    owned: std::sync::Arc<std::sync::Mutex<bevy_mod_openxr::init::OxrRenderResources>>,
    session: OxrSession,
    mut waiter: OxrFrameWaiter,
    resource_id: u32,
    commands: mpsc::Receiver<Command>,
    events: mpsc::SyncSender<Event>,
    _graphics: (RenderDevice, RenderInstance),
) {
    let mut initialized = false;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_session(
            &session,
            &mut waiter,
            resource_id,
            &commands,
            &events,
            &mut initialized,
        )
    }));
    let (result, panic_payload) = match caught {
        Ok(result) => (result, None),
        Err(cause) => {
            let message = format!(
                "OpenXR I/O thread panicked: {}",
                cause
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| cause.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic payload")
            );
            (Err(message), Some(cause))
        }
    };
    if let Err(cause) = result {
        if !initialized && panic_payload.is_none() {
            let _ = events.send(Event::Input(ApplicationInput::DisplayCompleted {
                request_id: resource_id,
                completion: DisplayCompletion::Failed { cause },
            }));
            return;
        }
        let _ = events.send(Event::Fatal(cause));
        // Unknown native work retains the session and device until process failure.
        loop {
            thread::park_timeout(Duration::from_secs(60));
        }
    }
    drop(owned);
    drop(waiter);
    drop(session);
    let _ = events.send(Event::Released);
}

fn run_session(
    session: &OxrSession,
    waiter: &mut OxrFrameWaiter,
    id: u32,
    commands: &mpsc::Receiver<Command>,
    events: &mpsc::SyncSender<Event>,
    initialized: &mut bool,
) -> Result<(), String> {
    let raw: &openxr::Session<openxr::AnyGraphics> = session;
    let base = std::sync::Arc::new(
        raw.create_reference_space(openxr::ReferenceSpaceType::LOCAL, openxr::Posef::IDENTITY)
            .map_err(error)?,
    );
    let head = raw
        .create_reference_space(openxr::ReferenceSpaceType::VIEW, openxr::Posef::IDENTITY)
        .map_err(error)?;
    let actions = Actions::new(session)?;
    events
        .send(Event::Input(ApplicationInput::DisplayCompleted {
            request_id: id,
            completion: DisplayCompletion::PermissionGranted {
                resource_id: Some(id),
            },
        }))
        .map_err(error)?;
    *initialized = true;
    let mut state = openxr::SessionState::IDLE;
    let mut start = None;
    let mut stop = None;
    let mut running = false;
    let mut lost: Option<String> = None;
    let mut release = false;
    let mut sequence = 0u64;
    let mut reset = None;
    loop {
        let mut buffer = openxr::EventDataBuffer::default();
        while let Some(event) = session.instance().poll_event(&mut buffer).map_err(error)? {
            match event {
                openxr::Event::SessionStateChanged(value)
                    if value.session() == session.as_raw() =>
                {
                    state = value.state();
                    if state == openxr::SessionState::LOSS_PENDING {
                        lost.get_or_insert_with(|| format!("OpenXR session entered {state:?}"));
                    }
                    if lost.is_some() {
                        events.send(Event::LossPending).map_err(error)?;
                    }
                    let (visibility, input_focused) = visibility(state);
                    events
                        .send(Event::Input(ApplicationInput::XrSessionState {
                            resource_id: id,
                            visibility,
                            input_focused,
                        }))
                        .map_err(error)?;
                    if matches!(
                        state,
                        openxr::SessionState::LOSS_PENDING | openxr::SessionState::EXITING
                    ) {
                        events
                            .send(Event::Input(ApplicationInput::DisplayEnded {
                                resource_id: id,
                                cause: Some(format!("OpenXR session entered {state:?}")),
                            }))
                            .map_err(error)?;
                    }
                }
                openxr::Event::ReferenceSpaceChangePending(value)
                    if value.session() == session.as_raw()
                        && value.reference_space_type() == openxr::ReferenceSpaceType::LOCAL =>
                {
                    reset = Some((
                        value.change_time(),
                        value
                            .pose_valid()
                            .then(|| pose(value.pose_in_previous_space())),
                    ));
                }
                openxr::Event::InstanceLossPending(_) => {
                    return Err("OpenXR instance loss pending".into());
                }
                _ => {}
            }
        }
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Start(request) => start = Some(request),
                Command::Stop(request) => {
                    start = None;
                    stop = Some(request);
                }
                Command::Release => {
                    start = None;
                    release = true;
                }
                Command::LossPending(cause) => {
                    lost.get_or_insert(cause);
                }
                Command::Drawn { .. } => {
                    return Err("OpenXR completion did not belong to a waited frame".into());
                }
            }
        }
        if lost.is_none()
            && !release
            && stop.is_none()
            && state == openxr::SessionState::READY
            && let Some(request_id) = start.take()
        {
            match session.begin(openxr::ViewConfigurationType::PRIMARY_STEREO) {
                Ok(status) => {
                    running = true;
                    if status == openxr::sys::Result::SESSION_LOSS_PENDING {
                        lost.get_or_insert_with(|| error(status));
                    }
                    report_status(events, id, status)?;
                    events
                        .send(Event::Input(ApplicationInput::DisplayCompleted {
                            request_id,
                            completion: DisplayCompletion::Started { resource_id: id },
                        }))
                        .map_err(error)?;
                }
                Err(cause) => {
                    events.send(Event::Stopped).map_err(error)?;
                    events
                        .send(Event::Input(ApplicationInput::DisplayCompleted {
                            request_id,
                            completion: DisplayCompletion::Failed {
                                cause: error(cause),
                            },
                        }))
                        .map_err(error)?;
                }
            }
        }
        if lost.is_some() && running {
            running = false;
            events.send(Event::Stopped).map_err(error)?;
        }
        // Every issued frame has already returned from Render before this point.
        if state == openxr::SessionState::STOPPING && running {
            let status = match session.end() {
                Ok(status) => status,
                Err(openxr::sys::Result::ERROR_SESSION_LOST) => {
                    openxr::sys::Result::ERROR_SESSION_LOST
                }
                Err(cause) => return Err(error(cause)),
            };
            if matches!(
                status,
                openxr::sys::Result::SESSION_LOSS_PENDING | openxr::sys::Result::ERROR_SESSION_LOST
            ) {
                lost.get_or_insert_with(|| error(status));
            }
            report_status(events, id, status)?;
            running = false;
            events.send(Event::Stopped).map_err(error)?;
            if let Some(request_id) = stop.take() {
                events
                    .send(Event::Input(ApplicationInput::DisplayCompleted {
                        request_id,
                        completion: if let Some(cause) = &lost {
                            DisplayCompletion::Failed {
                                cause: cause.clone(),
                            }
                        } else {
                            DisplayCompletion::Stopped
                        },
                    }))
                    .map_err(error)?;
            } else if lost.is_none() {
                events
                    .send(Event::Input(ApplicationInput::DisplayEnded {
                        resource_id: id,
                        cause: None,
                    }))
                    .map_err(error)?;
            }
        }
        if !running {
            if let Some(cause) = &lost
                && let Some(request_id) = start.take()
            {
                events
                    .send(Event::Input(ApplicationInput::DisplayCompleted {
                        request_id,
                        completion: DisplayCompletion::Failed {
                            cause: cause.clone(),
                        },
                    }))
                    .map_err(error)?;
            }
            if let Some(request_id) = stop.take() {
                events
                    .send(Event::Input(ApplicationInput::DisplayCompleted {
                        request_id,
                        completion: if let Some(cause) = &lost {
                            DisplayCompletion::Failed {
                                cause: cause.clone(),
                            }
                        } else {
                            DisplayCompletion::Stopped
                        },
                    }))
                    .map_err(error)?;
            }
            if release {
                return Ok(());
            }
            match commands.recv_timeout(Duration::from_millis(10)) {
                Ok(Command::Start(request)) => start = Some(request),
                Ok(Command::Stop(request)) => {
                    start = None;
                    stop = Some(request);
                }
                Ok(Command::Release) => {
                    start = None;
                    release = true;
                }
                Ok(Command::LossPending(cause)) => {
                    lost.get_or_insert(cause);
                }
                Ok(Command::Drawn { .. }) => {
                    return Err("OpenXR received a draw while stopped".into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("OpenXR I/O command channel disconnected before release".into());
                }
            }
            continue;
        }
        let (frame, status) = match waiter.wait() {
            Ok(result) => result,
            Err(openxr::sys::Result::ERROR_SESSION_LOST) => {
                lost.get_or_insert_with(|| error(openxr::sys::Result::ERROR_SESSION_LOST));
                running = false;
                events.send(Event::LossPending).map_err(error)?;
                events.send(Event::Stopped).map_err(error)?;
                events
                    .send(Event::Input(ApplicationInput::DisplayEnded {
                        resource_id: id,
                        cause: Some(error(openxr::sys::Result::ERROR_SESSION_LOST)),
                    }))
                    .map_err(error)?;
                continue;
            }
            Err(cause) => return Err(error(cause)),
        };
        if status == openxr::sys::Result::SESSION_LOSS_PENDING {
            lost.get_or_insert_with(|| error(status));
        }
        report_status(events, id, status)?;
        if reset
            .as_ref()
            .is_some_and(|(at, _)| frame.predicted_display_time.as_nanos() >= at.as_nanos())
        {
            let (_, previous_from_new) = reset.take().unwrap();
            events
                .send(Event::Input(ApplicationInput::XrReferenceReset {
                    resource_id: id,
                    previous_from_new,
                }))
                .map_err(error)?;
        }
        let (visibility, input_focused) = visibility(state);
        let observation = (|| {
            let (flags, views) = raw.locate_views(
                openxr::ViewConfigurationType::PRIMARY_STEREO,
                frame.predicted_display_time,
                &base,
            )?;
            let head_location = head.locate(&base, frame.predicted_display_time)?;
            let sources = actions.observe(session, &base, frame.predicted_display_time)?;
            Ok::<_, openxr::sys::Result>((flags, views, head_location, sources))
        })();
        let (flags, views, head_location, sources) = match observation {
            Ok(observation) => observation,
            Err(openxr::sys::Result::ERROR_SESSION_LOST) => {
                lost.get_or_insert_with(|| error(openxr::sys::Result::ERROR_SESSION_LOST));
                running = false;
                events.send(Event::LossPending).map_err(error)?;
                events.send(Event::Stopped).map_err(error)?;
                events
                    .send(Event::Input(ApplicationInput::DisplayEnded {
                        resource_id: id,
                        cause: Some(error(openxr::sys::Result::ERROR_SESSION_LOST)),
                    }))
                    .map_err(error)?;
                continue;
            }
            Err(cause) => return Err(error(cause)),
        };
        let valid = flags.contains(
            openxr::ViewStateFlags::ORIENTATION_VALID | openxr::ViewStateFlags::POSITION_VALID,
        );
        let viewer = XrViewerObservation {
            source: XrViewerSource::RuntimeDerived,
            tracking_from_head: valid_space(head_location.location_flags)
                .then(|| pose(head_location.pose)),
            views: if valid {
                views
                    .iter()
                    .enumerate()
                    .map(|(index, view)| XrViewObservation {
                        eye: match index {
                            0 => Some(XrEye::Left),
                            1 => Some(XrEye::Right),
                            _ => None,
                        },
                        tracking_from_eye: pose(view.pose),
                        projection: xr_projection_from_fov(
                            [
                                view.fov.angle_left,
                                view.fov.angle_right,
                                view.fov.angle_down,
                                view.fov.angle_up,
                            ]
                            .map(f64::from),
                        )
                        .map_or_else(Vec::new, |value| value.to_vec()),
                        depth_range: XrDepthRange::NegativeOneToOne,
                    })
                    .collect()
            } else {
                Vec::new()
            },
        };
        sequence = sequence
            .checked_add(1)
            .ok_or("OpenXR frame identifiers exhausted")?;
        events
            .send(Event::Frame(Box::new(Frame {
                id: sequence,
                time: frame.predicted_display_time,
                views,
                base: base.clone(),
                should_render: frame.should_render
                    && valid
                    && valid_space(head_location.location_flags)
                    && status != openxr::sys::Result::SESSION_LOSS_PENDING,
                xr: ApplicationXrFrameObservation {
                    observation: viewer,
                    visibility,
                    input_focused,
                },
                sources,
            })))
            .map_err(error)?;
        loop {
            match commands.recv().map_err(error)? {
                Command::Drawn {
                    frame: completed, ..
                } if completed == sequence => break,
                Command::Stop(request) => {
                    start = None;
                    stop = Some(request);
                }
                Command::Release => {
                    start = None;
                    release = true;
                }
                Command::LossPending(cause) => {
                    lost.get_or_insert(cause);
                }
                _ => return Err("OpenXR frame completion did not match the waited frame".into()),
            }
        }
    }
}
