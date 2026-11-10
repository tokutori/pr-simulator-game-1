use super::ApplicationRuntimeError;
use birdman_game_app::{application::ApplicationSession, launch_venue};
use birdman_game_core::SessionPhase;
use serde::{Deserialize, Serialize};

use birdman_game_presentation::camera::{
    CameraError, Pose, Quaternion, Vector3, camera_local_pose, compose_pose, flight_relative_pose,
    pilot_eye,
};
use birdman_game_presentation::game_ui::{
    GameUiFlightCamera, GameUiPresentationMode, GameUiReplayCamera, GameUiState, GameUiViewport,
};
use birdman_game_presentation::render_pose::{live_render_pose, prepared_render_pose};
use birdman_game_presentation::replay_camera::replay_camera_view;
use birdman_game_presentation::xr::{
    XrCameraMount, XrCameraRig, XrDepthRange, XrEye, XrReferenceFrame, XrTrackingOrigin,
    XrViewerFrame, xr_reference_frame,
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
    /// Final camera-centred water translation, snapped to the one-metre world XZ lattice.
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
    pub source: &'a SceneSource,
    pub viewport: &'a GameUiViewport,
    pub display: Option<(GameUiPresentationMode, Option<u32>)>,
    pub xr_viewer: Option<&'a XrViewerFrame>,
    pub phone_head: Option<Pose>,
    pub render_extent_px: Option<[u32; 2]>,
    pub venue_visible: bool,
}

/// Immutable accepted application values; tracking and display resources belong to the endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SceneSource {
    look_enabled: bool,
    preparation_required: bool,
    aircraft: Option<SceneAircraft>,
    world_from_aircraft: Pose,
    camera: CameraSource,
    pub(super) geometry: Option<crate::ApplicationGeometryFrame>,
    pub(super) aircraft_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneAircraft {
    pilot_position_meters: f64,
    initial_pilot_position_meters: f64,
    simulation_time_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CameraSource {
    Stationary {
        world_from_tracking: Pose,
    },
    Airframe {
        pilot_eye_offset_frd_m: [f64; 3],
        chase: bool,
    },
    Cinematic {
        world_from_tracking: Pose,
        vertical_field_of_view_degrees: f64,
    },
}

impl SceneSource {
    pub(super) fn allows_look(phase: SessionPhase) -> bool {
        matches!(
            phase,
            SessionPhase::FlightRunning
                | SessionPhase::FlightPaused { .. }
                | SessionPhase::Result
                | SessionPhase::Replay
                | SessionPhase::Attract
        )
    }
    pub(super) fn look_enabled(&self) -> bool {
        self.look_enabled
    }

    pub(super) fn requires_current_preparation(phase: SessionPhase) -> bool {
        matches!(
            phase,
            SessionPhase::BriefingPreparing
                | SessionPhase::BriefingReady
                | SessionPhase::BriefingFailed { .. }
        )
    }
    pub(super) fn preparation_required(&self) -> bool {
        self.preparation_required
    }

    pub(super) fn validate(&self) -> Result<(), ApplicationRuntimeError> {
        let invalid = || ApplicationRuntimeError::Camera(CameraError::InvalidPose);
        compose_pose(Pose::IDENTITY, self.world_from_aircraft)
            .map_err(ApplicationRuntimeError::Camera)?;
        if let Some(aircraft) = self.aircraft
            && (![
                aircraft.pilot_position_meters,
                aircraft.initial_pilot_position_meters,
                aircraft.simulation_time_seconds,
            ]
            .into_iter()
            .all(f64::is_finite)
                || aircraft.simulation_time_seconds < 0.0)
        {
            return Err(invalid());
        }
        match self.camera {
            CameraSource::Stationary {
                world_from_tracking,
            }
            | CameraSource::Cinematic {
                world_from_tracking,
                ..
            } => {
                compose_pose(Pose::IDENTITY, world_from_tracking)
                    .map_err(ApplicationRuntimeError::Camera)?;
            }
            CameraSource::Airframe {
                pilot_eye_offset_frd_m,
                ..
            } => {
                if self.aircraft.is_none()
                    || !pilot_eye_offset_frd_m.into_iter().all(f64::is_finite)
                {
                    return Err(invalid());
                }
            }
        }
        if let CameraSource::Cinematic {
            vertical_field_of_view_degrees,
            ..
        } = self.camera
            && (!vertical_field_of_view_degrees.is_finite()
                || !(0.0..180.0).contains(&vertical_field_of_view_degrees))
        {
            return Err(invalid());
        }
        if let Some(geometry) = &self.geometry {
            for part in &geometry.part_poses {
                compose_pose(Pose::IDENTITY, part.pose).map_err(ApplicationRuntimeError::Camera)?;
            }
            for update in &geometry.updates {
                for descriptor in [update.position, update.normal] {
                    descriptor
                        .validate()
                        .map_err(ApplicationRuntimeError::Assets)?;
                    if !matches!(descriptor.data_type, crate::ApplicationBufferType::F32)
                        || descriptor.components != 3
                    {
                        return Err(invalid());
                    }
                }
                if update.position.count != update.normal.count {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }

    pub(super) fn capture(
        application: &ApplicationSession,
        ui: &GameUiState,
        catalog: &mut super::assets::AssetCatalog,
    ) -> Result<Self, ApplicationRuntimeError> {
        let phase = application.core_session().snapshot().phase();
        let replay = if matches!(phase, SessionPhase::Replay | SessionPhase::Attract)
            && application.replay_available()
        {
            let session = application.core_session();
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
                    application.display_control_identity(),
                    application.aircraft().map_err(|_| {
                        ApplicationRuntimeError::Camera(CameraError::AircraftUnavailable)
                    })?,
                    clock.time_seconds(),
                    ui.replay_camera,
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
            live_render_pose(application).map_err(ApplicationRuntimeError::RenderPose)?
        } else {
            prepared_render_pose(application).map_err(ApplicationRuntimeError::RenderPose)?
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
                .map_err(ApplicationRuntimeError::Camera)?
            }
            None => {
                let venue = launch_venue().map_err(ApplicationRuntimeError::Venue)?;
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
            }
        };
        let camera = if aircraft.is_some()
            && matches!(
                phase,
                SessionPhase::Countdown { .. }
                    | SessionPhase::FlightRunning
                    | SessionPhase::FlightPaused { .. }
                    | SessionPhase::Result
                    | SessionPhase::Replay
                    | SessionPhase::Attract
            ) {
            if let Some(camera) = replay.as_ref().and_then(|view| view.cinematic()) {
                let [x, y, z] = camera.position_m();
                let [w, first, second, third] = camera.orientation_wxyz();
                CameraSource::Cinematic {
                    world_from_tracking: Pose {
                        position: Vector3 { x, y, z },
                        orientation: Quaternion {
                            w,
                            x: first,
                            y: second,
                            z: third,
                        },
                    },
                    vertical_field_of_view_degrees: camera.vertical_field_of_view_degrees(),
                }
            } else {
                CameraSource::Airframe {
                    pilot_eye_offset_frd_m: application
                        .aircraft()
                        .map_err(|_| {
                            ApplicationRuntimeError::Camera(CameraError::AircraftUnavailable)
                        })?
                        .definition()
                        .document()
                        .structure
                        .pilot_eye_offset_frd_m,
                    chase: replay
                        .as_ref()
                        .map_or(ui.flight_camera == GameUiFlightCamera::Chase, |view| {
                            view.camera() != GameUiReplayCamera::Pilot
                        }),
                }
            }
        } else {
            CameraSource::Stationary {
                world_from_tracking: birdman_game_presentation::camera::exhibition_camera_pose(
                    launch_venue()
                        .map_err(ApplicationRuntimeError::Venue)?
                        .platform
                        .horizontal_direction_ned(),
                )
                .map_err(ApplicationRuntimeError::Camera)?,
            }
        };
        let mut geometry = catalog
            .project_geometry(
                aircraft,
                application.aircraft().map_err(|cause| {
                    ApplicationRuntimeError::Application(
                        birdman_game_app::application::ApplicationError::Preparation(
                            birdman_game_app::HybridSessionPreparationError::Aircraft(cause),
                        ),
                    )
                })?,
            )
            .map_err(ApplicationRuntimeError::Assets)?;
        if phase == SessionPhase::Replay
            && !application.replay_available()
            && let Some(geometry) = &mut geometry
        {
            geometry.aircraft_visible = false;
            geometry.main_wing_visible = false;
        }
        Ok(Self {
            preparation_required: Self::requires_current_preparation(phase),
            look_enabled: Self::allows_look(phase),
            aircraft: aircraft.map(|flight| SceneAircraft {
                pilot_position_meters: flight.pilot_position_meters,
                initial_pilot_position_meters: flight.initial_pilot_position_meters,
                simulation_time_seconds: flight.simulation_time_seconds,
            }),
            world_from_aircraft,
            camera,
            geometry,
            aircraft_identity: application
                .aircraft()
                .map_err(|_| ApplicationRuntimeError::Camera(CameraError::AircraftUnavailable))?
                .identity(),
        })
    }
}

pub(super) struct SceneFrame {
    pub projection: ApplicationSceneProjection,
    pub tracking: super::surface::SurfaceTracking,
}

#[derive(Default)]
pub(super) struct SceneOwner {
    resource: Option<(GameUiPresentationMode, Option<u32>)>,
    origin: XrTrackingOrigin,
    rig: Option<XrCameraRig>,
    reference: Option<XrReferenceFrame>,
    look: [f64; 2],
    water_world_from_surface: Option<Pose>,
}

impl SceneOwner {
    pub(super) fn reset_look(&mut self) {
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

    pub(super) fn project(
        &mut self,
        context: SceneContext<'_>,
    ) -> Result<SceneFrame, ApplicationRuntimeError> {
        let mode = context
            .display
            .map_or(GameUiPresentationMode::Screen, |(mode, _)| mode);
        let aircraft = context.source.aircraft;
        let world_from_aircraft = context.source.world_from_aircraft;
        if !context.source.look_enabled {
            self.look = [0.0; 2];
        }
        let Some(display) = context.display else {
            self.rig = None;
            self.reference = None;
            return Ok(SceneFrame {
                projection: ApplicationSceneProjection {
                    lighting: None,
                    reflection: None,
                    venue_visible: context.venue_visible,
                    water_world_from_surface: self
                        .water_world_from_surface
                        .unwrap_or(Pose::IDENTITY),
                    geometry: None,
                    water_time_seconds: aircraft.map_or(0.0, |pose| pose.simulation_time_seconds),
                    world_from_aircraft,
                    camera: None,
                },
                tracking: super::surface::SurfaceTracking {
                    world_from_head: None,
                    viewer: None,
                    rig: None,
                    reference: None,
                },
            });
        };
        if self.resource != Some(display) {
            self.resource = Some(display);
            self.origin = XrTrackingOrigin::default();
        }
        let look = if mode == GameUiPresentationMode::Screen {
            self.look
        } else {
            [0.0; 2]
        };
        let (mount, fov) = match &context.source.camera {
            CameraSource::Stationary {
                world_from_tracking,
            } => (
                XrCameraMount::Stationary {
                    world_from_tracking: *world_from_tracking,
                },
                birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
            ),
            CameraSource::Airframe {
                pilot_eye_offset_frd_m,
                chase,
            } => {
                let flight = aircraft.ok_or(ApplicationRuntimeError::Camera(
                    CameraError::AircraftUnavailable,
                ))?;
                let eye = pilot_eye(*pilot_eye_offset_frd_m);
                let mount = if *chase {
                    XrCameraMount::External {
                        world_from_tracking: compose_pose(
                            world_from_aircraft,
                            camera_local_pose(
                                eye,
                                flight.pilot_position_meters,
                                flight.initial_pilot_position_meters,
                                true,
                                look,
                            )
                            .map_err(ApplicationRuntimeError::Camera)?,
                        )
                        .map_err(ApplicationRuntimeError::Camera)?,
                    }
                } else {
                    XrCameraMount::Aircraft {
                        aircraft_from_tracking: camera_local_pose(
                            eye,
                            flight.pilot_position_meters,
                            flight.initial_pilot_position_meters,
                            false,
                            look,
                        )
                        .map_err(ApplicationRuntimeError::Camera)?,
                    }
                };
                (
                    mount,
                    birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
                )
            }
            CameraSource::Cinematic {
                world_from_tracking,
                vertical_field_of_view_degrees,
            } => (
                XrCameraMount::External {
                    world_from_tracking: *world_from_tracking,
                },
                *vertical_field_of_view_degrees,
            ),
        };
        let rig = self
            .origin
            .camera_rig(world_from_aircraft, mount)
            .map_err(ApplicationRuntimeError::Camera)?;
        let viewer = match mode {
            GameUiPresentationMode::Screen => None,
            GameUiPresentationMode::PhoneVr => Some(
                birdman_game_presentation::camera::configured_phone_viewer(
                    [context.viewport.width, context.viewport.height],
                    context.render_extent_px,
                    fov,
                )
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
        let eyes: Vec<ApplicationEyeProjection> = match (&viewer, world_from_head) {
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
        let water_center = match eyes.as_slice() {
            [left, right] => Some(Vector3 {
                x: left.world_from_eye.position.x * 0.5 + right.world_from_eye.position.x * 0.5,
                y: 0.0,
                z: left.world_from_eye.position.z * 0.5 + right.world_from_eye.position.z * 0.5,
            }),
            _ => world_from_head.map(|head| head.position),
        };
        if let Some(center) = water_center {
            self.water_world_from_surface = Some(Pose {
                position: Vector3 {
                    x: center.x.round(),
                    y: 0.0,
                    z: center.z.round(),
                },
                ..Pose::IDENTITY
            });
        }
        self.rig = Some(rig);
        self.reference = reference;
        Ok(SceneFrame {
            projection: ApplicationSceneProjection {
                lighting: None,
                reflection: None,
                venue_visible: context.venue_visible,
                water_world_from_surface: self.water_world_from_surface.unwrap_or(Pose::IDENTITY),
                geometry: None,
                water_time_seconds: aircraft.map_or(0.0, |pose| pose.simulation_time_seconds),
                world_from_aircraft,
                camera: Some(ApplicationCameraProjection {
                    culling_projection: viewer
                        .as_ref()
                        .and_then(birdman_game_presentation::xr::xr_stereo_culling_projection),
                    screen_projection: if mode == GameUiPresentationMode::Screen {
                        Some(
                            birdman_game_presentation::camera::configured_screen_projection(
                                [context.viewport.width, context.viewport.height],
                                fov,
                            )
                            .map_err(ApplicationRuntimeError::Camera)?,
                        )
                    } else {
                        None
                    },
                    world_from_head,
                    eyes,
                }),
            },
            tracking: super::surface::SurfaceTracking {
                world_from_head,
                viewer,
                rig: Some(rig),
                reference,
            },
        })
    }
}
