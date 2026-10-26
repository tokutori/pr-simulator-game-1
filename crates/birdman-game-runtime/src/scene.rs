use super::ApplicationRuntimeError;
use birdman_game_app::{application::ApplicationSession, launch_venue};
use birdman_game_core::SessionPhase;
use serde::Serialize;

use birdman_game_presentation::camera::{
    CameraError, Pose, Quaternion, Vector3, camera_local_pose, compose_pose, flight_camera_pose,
    flight_relative_pose, registered_pilot_eye,
};
use birdman_game_presentation::game_ui::{
    GameUiLayout, GameUiPresentationMode, GameUiReplayCamera, GameUiState, GameUiViewport,
};
use birdman_game_presentation::render_pose::{
    FlightRenderPose, live_render_pose, prepared_render_pose,
};
use birdman_game_presentation::replay_camera::replay_camera_view;
use birdman_game_presentation::xr::{
    XrCameraMount, XrCameraRig, XrDepthRange, XrEye, XrEyeGeometry, XrReferenceFrame,
    XrTrackingOrigin, XrViewerFrame, xr_reference_change, xr_reference_frame,
};

/// One eye's final render-world pose and unchanged projection convention.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationEyeProjection {
    /// Exact configured Phone framebuffer viewport; HMD viewports come from the runtime.
    pub viewport_px: Option<[u32; 4]>,
    /// Primary eye identity.
    pub eye: XrEye,
    /// Eye-to-render-world transform.
    pub world_from_eye: Pose,
    /// Column-major projection matrix.
    pub projection: [f64; 16],
    /// Projection depth convention.
    pub depth_range: XrDepthRange,
}

/// Camera geometry ready for direct transfer to a drawing engine.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationCameraProjection {
    /// Conservative head-local OpenGL culling volume; absent means culling is disabled.
    pub culling_projection: Option<[f64; 16]>,
    /// Final monoscopic OpenGL projection, absent for stereo displays.
    pub screen_projection: Option<[f64; 16]>,
    /// Current centre camera, absent while tracking has no valid head observation.
    pub world_from_head: Option<Pose>,
    /// Final left and right eye transforms, empty for Screen or unavailable optics.
    pub eyes: Vec<ApplicationEyeProjection>,
}

/// Physical display metadata and its final render-space placement.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationSceneProjection {
    /// Final directional-light placement and shadow volume.
    pub lighting: Option<birdman_game_scene::lake_sky::LakeSceneShadow>,
    /// Final reflection-pass camera and world-to-clip matrix.
    pub reflection: Option<birdman_game_presentation::camera::PlanarReflection>,
    /// Whether the registered world geometry belongs to this admitted environment.
    pub venue_visible: bool,
    /// Final render-world water translation, independent of aircraft rotation and altitude.
    pub water_world_from_surface: Pose,
    /// Complete accepted mutable attributes and final articulated part transforms.
    pub geometry: Option<crate::ApplicationGeometryFrame>,
    /// Accepted simulation time for water animation.
    pub water_time_seconds: f64,
    /// Aircraft datum and orientation already converted into render-world axes.
    pub world_from_aircraft: Pose,
    /// Current display's camera geometry.
    pub camera: Option<ApplicationCameraProjection>,
}

pub(super) struct SceneContext<'a> {
    pub application: &'a ApplicationSession,
    pub ui: &'a GameUiState,
    pub viewport: &'a GameUiViewport,
    pub layout: &'a GameUiLayout,
    pub display: Option<(GameUiPresentationMode, Option<u32>)>,
    pub xr_viewer: Option<&'a XrViewerFrame>,
    pub phone_head: Option<Pose>,
    pub render_extent_px: Option<[u32; 2]>,
    pub venue_visible: bool,
}

pub(super) struct SceneFrame {
    pub aircraft: Option<FlightRenderPose>,
    pub projection: ApplicationSceneProjection,
    pub viewer: Option<XrViewerFrame>,
    pub rig: Option<XrCameraRig>,
    pub reference: Option<XrReferenceFrame>,
}

#[derive(Default)]
pub(super) struct SceneOwner {
    resource: Option<(GameUiPresentationMode, Option<u32>)>,
    origin: XrTrackingOrigin,
    rig: Option<XrCameraRig>,
    reference: Option<XrReferenceFrame>,
    look: [f64; 2],
    chase: bool,
}

impl SceneOwner {
    pub(super) fn toggle_camera(&mut self) {
        self.chase = !self.chase;
        self.look = [0.0; 2];
    }

    pub(super) fn observe_look(&mut self, delta: [f64; 2]) -> Result<(), CameraError> {
        if !delta.into_iter().all(f64::is_finite) {
            return Err(CameraError::InvalidPose);
        }
        self.look[0] = (self.look[0] - delta[0] * 0.003) % core::f64::consts::TAU;
        self.look[1] = (self.look[1] - delta[1] * 0.003).clamp(-1.2, 1.2);
        Ok(())
    }

    pub(super) fn reset_reference(
        &mut self,
        resource_id: u32,
        previous_from_new: Pose,
    ) -> Result<(), CameraError> {
        if self.resource == Some((GameUiPresentationMode::Hmd, Some(resource_id))) {
            self.origin.reset_reference(previous_from_new)?;
        }
        Ok(())
    }

    pub(super) fn reference_head(&self) -> Option<Pose> {
        self.reference.map(|reference| reference.head)
    }

    pub(super) fn tracking_change(
        &self,
        new_tracking_from_previous: Pose,
    ) -> Result<Option<Pose>, CameraError> {
        self.rig
            .map(|rig| xr_reference_change(rig.reference_from_tracking, new_tracking_from_previous))
            .transpose()
    }

    pub(super) fn project(
        &mut self,
        context: SceneContext<'_>,
    ) -> Result<SceneFrame, ApplicationRuntimeError> {
        let mode = context
            .display
            .map_or(GameUiPresentationMode::Screen, |(mode, _)| mode);
        let phase = context.application.core_session().snapshot().phase();
        if !matches!(
            phase,
            SessionPhase::FlightRunning
                | SessionPhase::FlightPaused { .. }
                | SessionPhase::Result
                | SessionPhase::Replay
                | SessionPhase::Attract
        ) {
            self.look = [0.0; 2];
        }
        let replay = if matches!(phase, SessionPhase::Replay | SessionPhase::Attract) {
            let session = context.application.core_session();
            let record = session
                .playback_record()
                .ok_or(ApplicationRuntimeError::Session(
                    birdman_game_core::GameSessionError::InvalidTransition,
                ))?;
            let clock = session
                .playback_clock()
                .ok_or(ApplicationRuntimeError::Session(
                    birdman_game_core::GameSessionError::InvalidTransition,
                ))?;
            Some(
                replay_camera_view(
                    record,
                    context.application.display_control_identity(),
                    clock.time_seconds(),
                    context.ui.replay_camera,
                    mode,
                    phase == SessionPhase::Attract,
                )
                .map_err(ApplicationRuntimeError::ReplayCamera)?,
            )
        } else {
            None
        };
        let aircraft = if let Some(view) = &replay {
            Some(view.render_pose())
        } else if matches!(
            phase,
            SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. } | SessionPhase::Result
        ) {
            live_render_pose(context.application).map_err(ApplicationRuntimeError::RenderPose)?
        } else {
            prepared_render_pose(context.application)
                .map_err(ApplicationRuntimeError::RenderPose)?
        };
        let world_from_aircraft = match aircraft {
            Some(flight) => {
                let datum = flight.datum_position_ned;
                let rotation = flight.attitude_body_to_ned;
                flight_relative_pose(
                    [datum.north, datum.east, datum.down],
                    Quaternion {
                        w: rotation.w,
                        x: rotation.x,
                        y: rotation.y,
                        z: rotation.z,
                    },
                    Pose::IDENTITY,
                )
            }
            None => Ok(launch_venue()
                .map_err(ApplicationRuntimeError::Venue)
                .map(|venue| {
                    let half_heading = venue.platform.heading_rad() / 2.0;
                    Pose {
                        position: Vector3 {
                            x: 0.0,
                            y: venue.platform.front_lip_above_water_meters,
                            z: 0.0,
                        },
                        orientation: Quaternion {
                            w: half_heading.cos(),
                            x: 0.0,
                            y: -half_heading.sin(),
                            z: 0.0,
                        },
                    }
                })?),
        }
        .map_err(ApplicationRuntimeError::Camera)?;
        let Some(display) = context.display else {
            self.rig = None;
            self.reference = None;
            return Ok(SceneFrame {
                aircraft,
                projection: ApplicationSceneProjection {
                    lighting: None,
                    reflection: None,
                    venue_visible: context.venue_visible,
                    water_world_from_surface: Pose {
                        position: Vector3 {
                            x: world_from_aircraft.position.x,
                            y: 0.0,
                            z: world_from_aircraft.position.z,
                        },
                        ..Pose::IDENTITY
                    },
                    geometry: None,
                    water_time_seconds: aircraft.map_or(0.0, |pose| pose.simulation_time_seconds),
                    world_from_aircraft,
                    camera: None,
                },
                viewer: None,
                rig: None,
                reference: None,
            });
        };
        if self.resource != Some(display) {
            self.resource = Some(display);
            self.origin = XrTrackingOrigin::default();
        }
        let chase = replay.as_ref().map_or(self.chase, |view| {
            view.camera() != GameUiReplayCamera::Pilot
        });
        let look = if mode == GameUiPresentationMode::Screen {
            self.look
        } else {
            [0.0; 2]
        };
        let cinematic = replay.as_ref().and_then(|view| view.cinematic());
        let fov = cinematic.map_or(
            birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
            |camera| camera.vertical_field_of_view_degrees(),
        );
        let mount = if let Some(flight) = aircraft.filter(|_| {
            matches!(
                phase,
                SessionPhase::FlightRunning
                    | SessionPhase::FlightPaused { .. }
                    | SessionPhase::Result
                    | SessionPhase::Replay
                    | SessionPhase::Attract
            )
        }) {
            if let Some(camera) = cinematic {
                let [x, y, z] = camera.position_m();
                let [w, first, second, third] = camera.orientation_wxyz();
                XrCameraMount::External {
                    world_from_tracking: Pose {
                        position: Vector3 { x, y, z },
                        orientation: Quaternion {
                            w,
                            x: first,
                            y: second,
                            z: third,
                        },
                    },
                }
            } else if chase {
                XrCameraMount::External {
                    world_from_tracking: flight_camera_pose(&flight, true, look)
                        .map_err(ApplicationRuntimeError::Camera)?,
                }
            } else {
                XrCameraMount::Aircraft {
                    aircraft_from_tracking: camera_local_pose(
                        registered_pilot_eye().map_err(ApplicationRuntimeError::Camera)?,
                        flight.pilot_position_meters,
                        flight.initial_pilot_position_meters,
                        false,
                        look,
                    )
                    .map_err(ApplicationRuntimeError::Camera)?,
                }
            }
        } else {
            XrCameraMount::Stationary {
                world_from_tracking: if mode == GameUiPresentationMode::Hmd {
                    Pose::IDENTITY
                } else {
                    exhibition_camera(
                        context.viewport,
                        context.layout.width_px,
                        mode == GameUiPresentationMode::PhoneVr,
                    )
                    .map_err(ApplicationRuntimeError::Camera)?
                },
            }
        };
        let rig = self
            .origin
            .camera_rig(world_from_aircraft, mount)
            .map_err(ApplicationRuntimeError::Camera)?;
        let viewer = match mode {
            GameUiPresentationMode::Screen => None,
            GameUiPresentationMode::PhoneVr => Some(
                phone_viewer(context.viewport, context.render_extent_px, fov)
                    .map_err(ApplicationRuntimeError::Camera)?,
            ),
            GameUiPresentationMode::Hmd => context.xr_viewer.cloned(),
        };
        let head = match mode {
            GameUiPresentationMode::Screen => Some(Pose::IDENTITY),
            GameUiPresentationMode::PhoneVr => context.phone_head,
            GameUiPresentationMode::Hmd => match &viewer {
                Some(XrViewerFrame::RuntimeDerived {
                    tracking_from_head, ..
                }) => Some(*tracking_from_head),
                Some(XrViewerFrame::Unavailable {
                    tracking_from_head, ..
                }) => *tracking_from_head,
                _ => None,
            },
        };
        let world_from_head = head
            .map(|head| compose_pose(rig.world_from_tracking, head))
            .transpose()
            .map_err(ApplicationRuntimeError::Camera)?;
        let reference = head
            .map(|head| xr_reference_frame(rig, head))
            .transpose()
            .map_err(ApplicationRuntimeError::Camera)?;
        let eyes = match (&viewer, world_from_head) {
            (
                Some(
                    XrViewerFrame::Configured { eyes, .. }
                    | XrViewerFrame::RuntimeDerived { eyes, .. },
                ),
                Some(head),
            ) => eyes
                .iter()
                .map(|eye| {
                    Ok(ApplicationEyeProjection {
                        viewport_px: if mode == GameUiPresentationMode::PhoneVr {
                            context.render_extent_px.map(|[width, height]| {
                                let left = width / 2;
                                match eye.eye {
                                    XrEye::Left => [0, 0, left, height],
                                    XrEye::Right => [left, 0, width - left, height],
                                }
                            })
                        } else {
                            None
                        },
                        eye: eye.eye,
                        world_from_eye: compose_pose(head, eye.head_from_eye)?,
                        projection: eye.projection,
                        depth_range: eye.depth_range,
                    })
                })
                .collect::<Result<Vec<_>, CameraError>>()
                .map_err(ApplicationRuntimeError::Camera)?,
            _ => Vec::new(),
        };
        self.rig = Some(rig);
        self.reference = reference;
        Ok(SceneFrame {
            aircraft,
            projection: ApplicationSceneProjection {
                lighting: None,
                reflection: None,
                venue_visible: context.venue_visible,
                water_world_from_surface: Pose {
                    position: Vector3 {
                        x: world_from_aircraft.position.x,
                        y: 0.0,
                        z: world_from_aircraft.position.z,
                    },
                    ..Pose::IDENTITY
                },
                geometry: None,
                water_time_seconds: aircraft.map_or(0.0, |pose| pose.simulation_time_seconds),
                world_from_aircraft,
                camera: Some(ApplicationCameraProjection {
                    culling_projection: viewer
                        .as_ref()
                        .and_then(birdman_game_presentation::xr::xr_stereo_culling_projection),
                    screen_projection: if mode == GameUiPresentationMode::Screen {
                        Some(screen_projection(context.viewport, fov)?)
                    } else {
                        None
                    },
                    world_from_head,
                    eyes,
                }),
            },
            viewer,
            rig: Some(rig),
            reference,
        })
    }
}

fn exhibition_camera(
    viewport: &GameUiViewport,
    panel_width_px: f64,
    phone: bool,
) -> Result<Pose, CameraError> {
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
    {
        return Err(CameraError::InvalidDistance);
    }
    let horizontal_half_fov =
        (birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() / 2.0).tan()
            * viewport.width
            / viewport.height;
    let target_x = -(panel_width_px / viewport.width + 24.0 / viewport.width).min(0.98);
    let yaw = (target_x * horizontal_half_fov).atan() - 272.168_516_861_668_76_f64.to_radians();
    let pitch = if phone { 0.0 } else { -3.0_f64.to_radians() };
    compose_pose(
        Pose {
            position: Vector3 {
                x: 0.0,
                y: 12.0,
                z: 0.0,
            },
            orientation: Quaternion {
                w: (yaw / 2.0).cos(),
                x: 0.0,
                y: (yaw / 2.0).sin(),
                z: 0.0,
            },
        },
        Pose {
            orientation: Quaternion {
                w: pitch.cos(),
                x: pitch.sin(),
                y: 0.0,
                z: 0.0,
            },
            ..Pose::IDENTITY
        },
    )
}

fn phone_viewer(
    viewport: &GameUiViewport,
    extent: Option<[u32; 2]>,
    vertical_fov_degrees: f64,
) -> Result<XrViewerFrame, CameraError> {
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
    {
        return Err(CameraError::InvalidDistance);
    }
    let near = 0.05;
    let far = 100_000.0;
    let top = near * (vertical_fov_degrees.to_radians() / 2.0).tan();
    let [width, height] = extent.map_or([viewport.width, viewport.height], |extent| {
        extent.map(f64::from)
    });
    let left_width = (width / 2.0).floor();
    if left_width <= 0.0 || height <= 0.0 {
        return Err(CameraError::InvalidDistance);
    }
    let eye = |eye, offset: f64| {
        let eye_width = if eye == XrEye::Left {
            left_width
        } else {
            width - left_width
        };
        let half_width = top * eye_width / height;
        let shift = -offset * near / 10.0;
        XrEyeGeometry {
            eye,
            head_from_eye: Pose {
                position: Vector3 {
                    x: offset,
                    y: 0.0,
                    z: 0.0,
                },
                ..Pose::IDENTITY
            },
            projection: [
                near / half_width,
                0.0,
                0.0,
                0.0,
                0.0,
                near / top,
                0.0,
                0.0,
                shift / half_width,
                0.0,
                -(far + near) / (far - near),
                -1.0,
                0.0,
                0.0,
                -2.0 * far * near / (far - near),
                0.0,
            ],
            depth_range: XrDepthRange::NegativeOneToOne,
        }
    };
    Ok(XrViewerFrame::Configured {
        tracking_from_head: (),
        eyes: [eye(XrEye::Left, -0.032), eye(XrEye::Right, 0.032)],
    })
}

fn screen_projection(
    viewport: &GameUiViewport,
    vertical_fov_degrees: f64,
) -> Result<[f64; 16], ApplicationRuntimeError> {
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
    {
        return Err(ApplicationRuntimeError::Camera(
            CameraError::InvalidDistance,
        ));
    }
    let focal = 1.0 / (vertical_fov_degrees.to_radians() / 2.0).tan();
    let near = 0.05;
    let far = 100_000.0;
    Ok([
        focal * viewport.height / viewport.width,
        0.0,
        0.0,
        0.0,
        0.0,
        focal,
        0.0,
        0.0,
        0.0,
        0.0,
        -(far + near) / (far - near),
        -1.0,
        0.0,
        0.0,
        -2.0 * far * near / (far - near),
        0.0,
    ])
}
