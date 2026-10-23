use super::ApplicationRuntimeError;
use super::scene::SceneFrame;
use super::{ApplicationPointerObservation, ApplicationSpatialObservation};
use birdman_game_presentation::camera::{Pose, Vector3, compose_pose};
use birdman_game_presentation::game_ui::{GameUiAnchor, GameUiFrame, GameUiView};
use birdman_game_presentation::xr::*;
use serde::{Deserialize, Serialize};

/// Actual font resource and shaping locale reported by platform font I/O.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationSurfaceObservation {
    /// Loaded font resource identity.
    pub font: MenuFont,
    /// Platform text-shaping locale.
    pub locale: String,
}

/// A direct platform text-measurement request in logical pixels.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationTextProbe {
    /// Font resource to select for this measurement.
    pub font: MenuFont,
    /// Platform text-shaping locale.
    pub locale: String,
    /// Font height in logical pixels.
    pub font_size_px: f64,
    /// Exact contextual string to measure.
    pub text: String,
}

/// Unmodified platform text metrics in logical pixels.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationGlyphMetrics {
    /// Horizontal advance.
    pub advance_px: f64,
    /// Signed left ink bearing.
    pub left_px: f64,
    /// Signed right ink bearing.
    pub right_px: f64,
    /// Signed ink ascent above the baseline.
    pub ascent_px: f64,
    /// Signed ink descent below the baseline.
    pub descent_px: f64,
}

/// Spatial menu painting and final mounted poses.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationMenuProjection {
    /// This display has no spatial menu.
    Absent,
    /// Font or tracking resources are still pending.
    Pending,
    /// Optical or font observations could not produce a drawable surface.
    Unavailable {
        /// Precise shared failure.
        reason: MenuLayoutFailure,
    },
    /// Prepared painting with final mounted poses.
    Ready {
        /// Final render-world panel pose.
        world_from_panel: Pose,
        /// Panel pose below a runtime-tracking parent.
        tracking_from_panel: Pose,
        /// Texture allocation in pixels.
        canvas_size: [u32; 2],
        /// Drawing commands, without interaction policy or hit geometry.
        paint: Box<ApplicationMenuPaint>,
        /// Visible cursor points in panel-local right/up metres.
        cursors: Vec<ApplicationMenuCursor>,
    },
}

/// Spatial menu drawing commands ready for a Canvas or native surface adapter.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationMenuPaint {
    /// Surface dimensions in metres.
    pub size: MenuSize,
    /// Published title.
    pub title: String,
    /// Background colour.
    pub background: String,
    /// Background opacity.
    pub opacity: f64,
    /// Clipped document painting.
    pub content: MenuPaintLayer,
    /// Fixed footer painting.
    pub footer: Vec<SurfacePaintItem>,
}

/// One cursor's final visible position and dwell indicator.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationMenuCursor {
    /// Panel-local right/up coordinates in metres.
    pub point: [f64; 2],
    /// Completed fraction of the selection indicator.
    pub progress: f64,
}

/// Head-fixed painting with its final world anchor.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationHeadHudProjection {
    /// This display has no head-fixed surface.
    Absent,
    /// Font or head tracking is still pending.
    Pending,
    /// Optical or font observations rejected the surface.
    Unavailable {
        /// Precise shared failure.
        reason: MenuLayoutFailure,
    },
    /// Final head-fixed pose and drawing commands.
    Ready {
        /// Final render-world surface pose.
        world_from_surface: Pose,
        /// Head-relative surface and texture drawing commands.
        paint: Box<HeadHudPaintProjection>,
    },
}

/// Complete spatial UI produced alongside the same camera observation.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationSpatialProjection {
    /// Opaque identity to echo after this projection has actually been submitted.
    pub presentation_id: u32,
    /// Retained world/aircraft-referenced menu.
    pub menu: ApplicationMenuProjection,
    /// Head-referenced smart-glasses instruments.
    pub head_hud: ApplicationHeadHudProjection,
}

#[derive(Default)]
pub(super) struct SurfaceOwner {
    menu: MenuSession,
    placement: XrSpatialSession,
    cache: SurfaceFontCache,
    font: Option<ApplicationSurfaceObservation>,
    geometry: Option<MenuGeometryIdentity>,
    pointer: XrPointerSession,
    published: Option<PublishedMenu>,
    offered: Option<PublishedMenu>,
    presentation_id: u32,
    gamepad: Option<(u32, u32)>,
    cursors: Vec<ApplicationMenuCursor>,
    resource: Option<(
        birdman_game_presentation::game_ui::GameUiPresentationMode,
        Option<u32>,
    )>,
}

struct PublishedMenu {
    presentation_id: u32,
    context: MenuContext,
    view_revision: u32,
    panel_pose: Pose,
    page_progress: f64,
    surface: XrPointerSurface,
}

pub(super) struct SurfaceAction {
    pub view_revision: u32,
    pub input: Option<birdman_game_presentation::game_ui::GameUiControlInput>,
    pub back: bool,
}

pub(super) struct SurfaceDisplay {
    pub resource: Option<(
        birdman_game_presentation::game_ui::GameUiPresentationMode,
        Option<u32>,
    )>,
    pub frame_rate_label: String,
}

impl SurfaceOwner {
    pub(super) fn retire_input(&mut self) {
        self.pointer = XrPointerSession::default();
        self.published = None;
        self.offered = None;
        self.gamepad = None;
        self.cursors.clear();
    }

    fn acknowledge(
        &mut self,
        presentation_id: Option<u32>,
    ) -> Result<bool, ApplicationRuntimeError> {
        if self
            .published
            .as_ref()
            .is_some_and(|menu| Some(menu.presentation_id) == presentation_id)
        {
            return Ok(self
                .menu
                .state()
                .accepts(&self.published.as_ref().unwrap().context));
        }
        if !self.offered.as_ref().is_some_and(|menu| {
            Some(menu.presentation_id) == presentation_id
                && self.menu.state().accepts(&menu.context)
        }) {
            return Ok(false);
        }
        let menu = self.offered.take().unwrap();
        if self.published.as_ref().is_some_and(|old| {
            old.context != menu.context || old.view_revision != menu.view_revision
        }) {
            self.pointer = XrPointerSession::default();
            self.cursors.clear();
        }
        self.pointer
            .set_surface(Some(menu.surface.clone()))
            .map_err(ApplicationRuntimeError::SpatialInput)?;
        self.published = Some(menu);
        Ok(true)
    }

    pub(super) fn observe(
        &mut self,
        observation: ApplicationSpatialObservation,
        head: Option<Pose>,
        focused: bool,
        timestamp_ms: f64,
    ) -> Result<Option<SurfaceAction>, ApplicationRuntimeError> {
        if !self.acknowledge(observation.presented_id)? {
            self.pointer = XrPointerSession::default();
            if let Some(menu) = &self.published {
                self.pointer
                    .set_surface(Some(menu.surface.clone()))
                    .map_err(ApplicationRuntimeError::SpatialInput)?;
            }
            self.gamepad = None;
            self.cursors.clear();
            return Ok(None);
        }
        let menu = self.published.as_ref().unwrap();
        let gamepads: Vec<_> = observation
            .gamepads
            .iter()
            .filter_map(super::ApplicationGamepadObservation::menu)
            .collect();
        let gamepad = gamepads
            .iter()
            .find(|pad| self.gamepad == Some((pad.index, pad.generation)))
            .or_else(|| {
                gamepads
                    .iter()
                    .min_by_key(|pad| (pad.index, pad.generation))
            })
            .copied();
        self.gamepad = gamepad.map(|pad| (pad.index, pad.generation));
        let projection = self
            .pointer
            .observe(XrPointerObservation {
                timestamp_ms,
                focused,
                panel_pose: menu.panel_pose,
                head_pose: head,
                pointers: observation.pointers.into_iter().map(pointer_ray).collect(),
                gamepad,
            })
            .map_err(ApplicationRuntimeError::SpatialInput)?;
        self.cursors = projection
            .primary
            .iter()
            .map(|hit| cursor(hit, projection.progress))
            .chain(projection.secondary.iter().map(|hit| cursor(hit, 1.0)))
            .collect();
        let menu = self.published.as_ref().unwrap();
        self.menu.update(MenuSessionInput::MenuFocus {
            context: menu.context.clone(),
            focus: match projection
                .primary
                .as_ref()
                .and_then(|hit| hit.target.as_ref())
            {
                Some(XrPointerTarget::Control { id }) => MenuFocus::Control {
                    control_id: id.clone(),
                },
                _ => MenuFocus::None,
            },
        });
        if let Some(hit) = projection.activation {
            return Ok(self.activate(hit));
        }
        if projection.back {
            return Ok(Some(SurfaceAction {
                view_revision: menu.view_revision,
                input: None,
                back: true,
            }));
        }
        if projection.scroll_pages != 0.0 {
            self.scroll(MenuScrollIntent::Delta {
                viewport_pages: projection.scroll_pages,
                page_progress: menu.page_progress,
            });
        }
        Ok(None)
    }

    pub(super) fn select(
        &mut self,
        presentation_id: u32,
        pointer: ApplicationPointerObservation,
        timestamp_ms: f64,
    ) -> Result<Option<SurfaceAction>, ApplicationRuntimeError> {
        if !self.acknowledge(Some(presentation_id))? {
            return Ok(None);
        }
        let panel_pose = self.published.as_ref().unwrap().panel_pose;
        let Some(hit) =
            self.pointer
                .select_on_panel(pointer_ray(pointer), timestamp_ms, panel_pose)
        else {
            return Ok(None);
        };
        Ok(self.activate(hit))
    }

    fn activate(&mut self, hit: XrPointerHit) -> Option<SurfaceAction> {
        let menu = self.published.as_ref()?;
        match hit.target? {
            XrPointerTarget::Page { next } => {
                self.scroll(MenuScrollIntent::Page {
                    direction: if next {
                        MenuPageDirection::Next
                    } else {
                        MenuPageDirection::Previous
                    },
                    page_progress: menu.page_progress,
                });
                None
            }
            XrPointerTarget::Control { .. } => Some(SurfaceAction {
                view_revision: menu.view_revision,
                input: hit.intent,
                back: false,
            }),
        }
    }

    fn scroll(&mut self, intent: MenuScrollIntent) {
        if let Some(menu) = &self.published {
            self.menu.update(MenuSessionInput::MenuScroll {
                context: menu.context.clone(),
                intent,
            });
            self.retire_input();
        }
    }

    pub(super) fn recenter_menu(
        &mut self,
        head: Option<Pose>,
    ) -> Result<(), birdman_game_presentation::camera::CameraError> {
        if let Some(head) = head {
            self.placement.recenter_menu(head)?;
        }
        self.invalidate();
        Ok(())
    }

    pub(super) fn recenter_tracking(
        &mut self,
        change: Option<Pose>,
    ) -> Result<(), birdman_game_presentation::camera::CameraError> {
        if let Some(change) = change {
            self.placement.recenter_tracking(change)?;
        }
        self.invalidate();
        Ok(())
    }

    fn invalidate(&mut self) {
        self.retire_input();
        if let MenuSessionState::Active {
            scope, generation, ..
        } = self.menu.state()
        {
            self.menu.update(MenuSessionInput::MenuScrollInvalidated {
                context: MenuContext {
                    scope: scope.clone(),
                    generation: *generation,
                },
            });
        }
    }

    pub(super) fn project(
        &mut self,
        view: &GameUiView,
        frame: &GameUiFrame,
        scene: &SceneFrame,
        font: Option<&ApplicationSurfaceObservation>,
        display: SurfaceDisplay,
        measure: &mut impl FnMut(
            &ApplicationTextProbe,
        ) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
    ) -> Result<ApplicationSpatialProjection, ApplicationRuntimeError> {
        let SurfaceDisplay {
            resource: display,
            frame_rate_label,
        } = display;
        let active = display.is_some_and(|(mode, _)| {
            mode != birdman_game_presentation::game_ui::GameUiPresentationMode::Screen
        });
        if self.resource != display {
            self.resource = display;
            self.placement.close_menu();
            self.geometry = None;
            self.invalidate();
        }
        let presentation_id = self
            .presentation_id
            .checked_add(1)
            .ok_or(ApplicationRuntimeError::PresentationExhausted)?;
        let mut published = None;
        if self.font.as_ref() != font {
            self.font = font.cloned();
            self.cache = SurfaceFontCache::default();
            self.invalidate();
        }
        let panel = active
            .then(|| {
                view.panels
                    .iter()
                    .find(|panel| panel.anchor == GameUiAnchor::Menu)
            })
            .flatten();
        let target = match panel {
            Some(panel) => MenuTarget::Active {
                scope: MenuScope {
                    panel_id: panel.id.clone(),
                    view_key: view.interaction_scope.clone(),
                },
            },
            None => MenuTarget::Closed,
        };
        self.menu.update(MenuSessionInput::MenuScrollSynchronized {
            generation: self.menu.state().generation(),
            target,
        });
        if panel.is_none() {
            self.placement.close_menu();
            self.geometry = None;
        }
        let menu = match (
            panel,
            scene.viewer.as_ref(),
            scene.rig,
            scene.reference,
            font,
        ) {
            (None, ..) => ApplicationMenuProjection::Absent,
            (Some(panel), Some(viewer), Some(rig), Some(reference), Some(font)) => {
                let (generation, progress) = match self.menu.state() {
                    MenuSessionState::Active {
                        generation,
                        progress,
                        ..
                    } => (*generation, *progress),
                    _ => {
                        return Err(ApplicationRuntimeError::Ui(
                            birdman_game_presentation::game_ui::GameUiError::StaleView,
                        ));
                    }
                };
                let reply = prepare_game_ui_menu(
                    view,
                    frame,
                    MenuPreparationInput {
                        progress,
                        generation,
                        view_revision: view.view_revision,
                        panel_id: panel.id.clone(),
                        viewer: Box::new(viewer.clone()),
                        font: font.font.clone(),
                        locale: font.locale.clone(),
                    },
                    &mut self.cache,
                    |request, size, value| measure_pixels(request, size, value, 1536.0, measure),
                );
                match reply {
                    MenuLayoutReply::Unavailable { reason } => {
                        ApplicationMenuProjection::Unavailable { reason }
                    }
                    MenuLayoutReply::Ready { mut projection } => {
                        if self
                            .geometry
                            .as_ref()
                            .is_some_and(|geometry| geometry != &projection.geometry_key)
                        {
                            self.invalidate();
                        }
                        self.geometry = Some(projection.geometry_key.clone());
                        projection.surface.revision = self.menu.state().generation();
                        let [x, y, z] = panel.local_position;
                        let placement = self.placement.place_panel(
                            XrPanelObservation::Ready {
                                geometry: XrPanelGeometry {
                                    scope: XrMenuScope {
                                        scene: format!("{:?}", view.scene),
                                        overlay: Some(format!(
                                            "{:?}:{}",
                                            view.active_overlay, view.selections.credits_open
                                        )),
                                        panel_id: panel.id.clone(),
                                    },
                                    anchor: panel.anchor,
                                    local_pose: Pose {
                                        position: Vector3 { x, y, z },
                                        ..Pose::IDENTITY
                                    },
                                    size_m: [projection.size.width, projection.size.height],
                                },
                            },
                            viewer,
                            Some(reference),
                        );
                        match placement {
                            XrPanelPlacement::Ready {
                                reference_from_panel,
                            } => {
                                let poses = xr_panel_camera_poses(rig, reference_from_panel)
                                    .map_err(ApplicationRuntimeError::Camera)?;
                                let MenuSessionState::Active {
                                    scope, generation, ..
                                } = self.menu.state()
                                else {
                                    unreachable!();
                                };
                                let context = MenuContext {
                                    scope: scope.clone(),
                                    generation: *generation,
                                };
                                let cursors = self
                                    .published
                                    .as_ref()
                                    .filter(|menu| {
                                        menu.context == context
                                            && menu.view_revision == view.view_revision
                                            && menu.surface == projection.surface
                                    })
                                    .map_or_else(Vec::new, |_| self.cursors.clone());
                                published = Some(PublishedMenu {
                                    presentation_id,
                                    context,
                                    view_revision: view.view_revision,
                                    panel_pose: poses.tracking_from_panel,
                                    page_progress: projection.page_progress,
                                    surface: projection.surface,
                                });
                                let paint = ApplicationMenuPaint {
                                    size: projection.size,
                                    title: projection.title,
                                    background: projection.background,
                                    opacity: projection.opacity,
                                    content: projection.content,
                                    footer: projection.footer,
                                };
                                ApplicationMenuProjection::Ready {
                                    world_from_panel: poses.world_from_panel,
                                    tracking_from_panel: poses.tracking_from_panel,
                                    canvas_size: canvas_size(paint.size, 1536.0),
                                    paint: Box::new(paint),
                                    cursors,
                                }
                            }
                            XrPanelPlacement::Unavailable { reason } => {
                                ApplicationMenuProjection::Unavailable {
                                    reason: fit_failure(reason),
                                }
                            }
                            XrPanelPlacement::Absent => ApplicationMenuProjection::Absent,
                        }
                    }
                }
            }
            _ => ApplicationMenuProjection::Pending,
        };
        let head_hud = match (
            active,
            scene.viewer.as_ref(),
            scene
                .projection
                .camera
                .as_ref()
                .and_then(|camera| camera.world_from_head),
            font,
        ) {
            (false, ..) => ApplicationHeadHudProjection::Absent,
            (true, Some(viewer), Some(head), Some(font)) => {
                match prepare_game_ui_head_hud(
                    view,
                    frame,
                    HeadHudPreparationInput {
                        view_revision: view.view_revision,
                        viewer: Box::new(viewer.clone()),
                        profile: XrHeadHudProfile::default(),
                        font: font.font.clone(),
                        locale: font.locale.clone(),
                        frame_rate_label: Some(frame_rate_label),
                    },
                    &mut self.cache,
                    |request, size, value| measure_pixels(request, size, value, 2048.0, measure),
                ) {
                    HeadHudPaintReply::Absent => ApplicationHeadHudProjection::Absent,
                    HeadHudPaintReply::Unavailable { reason } => {
                        ApplicationHeadHudProjection::Unavailable { reason }
                    }
                    HeadHudPaintReply::Ready { projection } => {
                        ApplicationHeadHudProjection::Ready {
                            world_from_surface: compose_pose(head, projection.local_pose)
                                .map_err(ApplicationRuntimeError::Camera)?,
                            paint: projection,
                        }
                    }
                }
            }
            _ => ApplicationHeadHudProjection::Pending,
        };
        if published.is_none() {
            self.retire_input();
        }
        self.offered = published;
        self.presentation_id = presentation_id;
        Ok(ApplicationSpatialProjection {
            presentation_id,
            menu,
            head_hud,
        })
    }
}

fn pointer_ray(observation: ApplicationPointerObservation) -> XrPointerRay {
    XrPointerRay {
        source: XrPointerSource::Controller {
            id: observation.source_id,
        },
        reference_from_space: observation.tracking_from_ray,
        origin: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        direction: Vector3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
    }
}

fn cursor(hit: &XrPointerHit, progress: f64) -> ApplicationMenuCursor {
    ApplicationMenuCursor {
        point: hit.point,
        progress: if matches!(hit.source, XrPointerSource::Controller { .. }) {
            1.0
        } else {
            progress
        },
    }
}

fn fit_failure(reason: XrFitReason) -> MenuLayoutFailure {
    match reason {
        XrFitReason::NotStereo => MenuLayoutFailure::NotStereo,
        XrFitReason::ViewerUnavailable => MenuLayoutFailure::ViewerUnavailable,
        XrFitReason::UnsupportedViewConfiguration => {
            MenuLayoutFailure::UnsupportedViewConfiguration
        }
        XrFitReason::InvalidViewGeometry => MenuLayoutFailure::InvalidViewGeometry,
        XrFitReason::InsufficientViewArea => MenuLayoutFailure::InsufficientViewArea,
    }
}

fn canvas_size(size: MenuSize, maximum: f64) -> [u32; 2] {
    let scale = maximum / size.width.max(size.height);
    [
        (size.width * scale).round().max(1.0) as u32,
        (size.height * scale).round().max(1.0) as u32,
    ]
}

fn measure_pixels(
    request: &MenuTextRequest,
    size: MenuSize,
    text: &str,
    maximum: f64,
    measure: &mut impl FnMut(
        &ApplicationTextProbe,
    ) -> Result<ApplicationGlyphMetrics, MenuLayoutFailure>,
) -> Result<MenuGlyphMetrics, MenuLayoutFailure> {
    let [width, height] = canvas_size(size, maximum);
    let horizontal = size.width / f64::from(width);
    let vertical = size.height / f64::from(height);
    let observed = measure(&ApplicationTextProbe {
        font: request.font.clone(),
        locale: request.locale.clone(),
        font_size_px: request.font_meters / vertical,
        text: text.to_owned(),
    })?;
    Ok(MenuGlyphMetrics {
        advance: observed.advance_px * horizontal,
        left: observed.left_px * horizontal,
        right: observed.right_px * horizontal,
        ascent: observed.ascent_px * vertical,
        descent: observed.descent_px * vertical,
    })
}
