use super::{Pose, Vector3};
use crate::game_ui::{GameUiControlInput, range_value};
use serde::{Deserialize, Serialize};

/// Runtime input identity, stable for one presentation session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum XrPointerSource {
    /// Head-directed dwell input.
    Gaze,
    /// Runtime-assigned controller identity.
    Controller {
        /// Session-local runtime source identifier.
        id: u32,
    },
    /// Standard gamepad cursor, identified by connection lifetime.
    Gamepad {
        /// Adapter's device slot.
        index: u32,
        /// Connection generation for this slot.
        generation: u32,
    },
}

/// Semantic target of a measured interactive rectangle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum XrPointerTarget {
    /// Shared UI control identifier.
    Control {
        /// Identifier from the shared UI control.
        id: String,
    },
    /// Previous or next measured menu page.
    Page {
        /// True advances to the next page.
        next: bool,
    },
}

/// Rectangle in surface coordinates: left, top, width and height, in metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct XrPointerRect(pub [f64; 4]);

impl XrPointerRect {
    fn valid(self) -> bool {
        self.0.into_iter().all(f64::is_finite) && self.0[2] > 0.0 && self.0[3] > 0.0
    }

    fn contains(self, point: [f64; 2]) -> bool {
        point[0] >= self.0[0]
            && point[0] <= self.0[0] + self.0[2]
            && point[1] >= self.0[1]
            && point[1] <= self.0[1] + self.0[3]
    }
}

/// One enabled control or page rectangle after measurement and clipping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPointerRegion {
    /// Typed control or page target.
    pub target: XrPointerTarget,
    /// Visible hit rectangle in surface coordinates.
    pub bounds: XrPointerRect,
    /// Interactive widget data; an omitted descriptor represents a button.
    #[serde(default)]
    pub control: Option<XrPointerControl>,
}

/// Widget value and uncut track required for shared activation resolution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum XrPointerControl {
    /// Toggle's observed value.
    Toggle {
        /// Current selection.
        value: bool,
    },
    /// Range's shared scale and complete track before clipping.
    Range {
        /// Lower bound.
        minimum: f64,
        /// Upper bound.
        maximum: f64,
        /// Positive step size.
        step: f64,
        /// Complete slider rectangle in surface coordinates.
        track: XrPointerRect,
    },
}

impl XrPointerRegion {
    fn valid(&self) -> bool {
        self.bounds.valid()
            && match self.control {
                Some(XrPointerControl::Range {
                    minimum,
                    maximum,
                    step,
                    track,
                }) => track.valid() && range_value(minimum, maximum, step, 0.0).is_some(),
                _ => true,
            }
    }

    fn intent(&self, point: [f64; 2]) -> Option<GameUiControlInput> {
        let XrPointerTarget::Control { id } = &self.target else {
            return None;
        };
        Some(match self.control {
            None => GameUiControlInput::Activate {
                control_id: id.clone(),
            },
            Some(XrPointerControl::Toggle { value }) => GameUiControlInput::SetToggle {
                control_id: id.clone(),
                value: !value,
            },
            Some(XrPointerControl::Range {
                minimum,
                maximum,
                step,
                track,
            }) => {
                let ratio = (point[0] - track.0[0]) / track.0[2];
                GameUiControlInput::SetRange {
                    control_id: id.clone(),
                    value: range_value(minimum, maximum, step, ratio)?,
                }
            }
        })
    }
}

/// Measured surface geometry owned until its scope or layout changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPointerSurface {
    /// Adapter's exact scene, overlay, panel and scroll context identity.
    pub scope: String,
    /// Layout/input revision within the same semantic menu.
    #[serde(default)]
    pub revision: u64,
    /// Surface width and height in metres.
    pub size_m: [f64; 2],
    /// Enabled rectangles in drawing/hit priority order.
    pub regions: Vec<XrPointerRegion>,
}

impl XrPointerSurface {
    fn valid(&self) -> bool {
        self.scope.len() <= 4096
            && self
                .size_m
                .into_iter()
                .all(|value| value.is_finite() && value > 0.0)
            && self.regions.len() <= 512
            && self.regions.iter().all(|region| {
                region.valid()
                    && match &region.target {
                        XrPointerTarget::Control { id } => !id.is_empty() && id.len() <= 1024,
                        XrPointerTarget::Page { .. } => true,
                    }
            })
    }

    fn hit(&self, source: XrPointerSource, point: [f64; 2]) -> XrPointerHit {
        let surface = [
            point[0] + self.size_m[0] / 2.0,
            self.size_m[1] / 2.0 - point[1],
        ];
        let region = self
            .regions
            .iter()
            .find(|region| region.bounds.contains(surface));
        XrPointerHit {
            source,
            point,
            target: region.map(|region| region.target.clone()),
            intent: region.and_then(|region| region.intent(surface)),
        }
    }
}

/// Pointing ray with its observed space-to-panel-reference transform.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPointerRay {
    /// Stable input source.
    pub source: XrPointerSource,
    /// Reference-from-ray-space pose for this observation.
    pub reference_from_space: Pose,
    /// Ray origin in the observed space, in metres.
    pub origin: Vector3,
    /// Unit pointing direction in the observed space.
    pub direction: Vector3,
}

/// Button state copied from the active XR frame.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrInputButtonObservation {
    /// Platform pressed state.
    pub pressed: bool,
    /// Platform touch state.
    pub touched: bool,
}

/// Unfiltered gamepad values associated with one XR input source.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrInputGamepadObservation {
    /// Platform mapping declaration.
    pub mapping: String,
    /// Raw axes; non-finite observations are None.
    pub axes: Vec<Option<f64>>,
    /// Raw button order, including placeholder buttons.
    pub buttons: Vec<XrInputButtonObservation>,
}

/// One source's same-frame tracking and physical inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XrInputSourceObservation {
    /// Platform handle for this source's connection lifetime.
    pub source_id: u32,
    /// Raw target-ray mode.
    pub target_ray_mode: String,
    /// Raw handedness, independent of the source handle.
    pub handedness: String,
    /// Tracking-space target-ray pose; absent tracking retains button observations.
    #[serde(deserialize_with = "super::deserialize_xr_optional_pose")]
    #[cfg_attr(feature = "schema", schemars(with = "Option<super::XrObservedPose>"))]
    pub tracking_from_ray: Option<Pose>,
    /// Actual gamepad, absent when the source has no gamepad API.
    pub gamepad: Option<XrInputGamepadObservation>,
    /// Raw primary-action state when the API exposes a Boolean action instead of a completed select event.
    #[serde(default)]
    pub select_pressed: Option<bool>,
}

impl XrInputSourceObservation {
    fn ray(&self) -> Option<XrPointerRay> {
        if self.target_ray_mode != "tracked-pointer" {
            return None;
        }
        let pose = self.tracking_from_ray?.normalized()?;
        Some(XrPointerRay {
            source: XrPointerSource::Controller { id: self.source_id },
            reference_from_space: pose,
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
        })
    }
}

/// One same-frame input observation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPointerObservation {
    /// Monotonic presentation timestamp in milliseconds.
    pub timestamp_ms: f64,
    /// Runtime grants input focus.
    pub focused: bool,
    /// Reference-from-panel pose from this rendered frame.
    pub panel_pose: Pose,
    /// Reference-from-head pose used to derive the forward gaze ray.
    pub head_pose: Option<Pose>,
    /// Raw connected XR sources, including sources without current tracking.
    pub sources: Vec<XrInputSourceObservation>,
    /// Connected standard gamepad observed with the rays.
    #[serde(default)]
    pub gamepad: Option<XrGamepadObservation>,
}

/// Raw standard gamepad values used by spatial UI.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrGamepadObservation {
    /// Adapter's device slot.
    pub index: u32,
    /// Connection generation for this slot.
    pub generation: u32,
    /// Left and right stick axes in standard mapping order.
    pub axes: [f64; 4],
    /// Primary and Back button states.
    pub buttons: [bool; 2],
}

impl XrGamepadObservation {
    fn source(self) -> XrPointerSource {
        XrPointerSource::Gamepad {
            index: self.index,
            generation: self.generation,
        }
    }
}

/// Visible metric cursor and its typed target.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct XrPointerHit {
    /// Input source producing this cursor.
    pub source: XrPointerSource,
    /// Panel-local right/up metric coordinates.
    pub point: [f64; 2],
    /// Enabled control or page under the cursor.
    pub target: Option<XrPointerTarget>,
    /// Shared widget input resolved at this hit, including range snapping.
    pub intent: Option<GameUiControlInput>,
}

/// Cursor, focus and activation projected by the shared owner.
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct XrPointerProjection {
    /// Primary focus owner, preferring a controller aimed at the panel.
    pub primary: Option<XrPointerHit>,
    /// Other controllers with visible panel intersections.
    pub secondary: Vec<XrPointerHit>,
    /// Gaze dwell completion; controllers use an explicit trigger.
    pub progress: f64,
    /// Focus changed because the target or source changed.
    pub focus_changed: bool,
    /// At most one accepted action for this observation.
    pub activation: Option<XrPointerHit>,
    /// Back button's newly accepted press.
    pub back: bool,
    /// Right-stick scroll measured in viewport pages.
    pub scroll_pages: f64,
}

#[derive(Clone)]
struct Focus {
    hit: XrPointerHit,
    started_ms: f64,
    activated: bool,
}

struct GamepadInput {
    sample: XrGamepadObservation,
    timestamp_ms: f64,
    cursor: [f64; 2],
}

struct ControllerInput {
    source_id: u32,
    squeeze: Option<bool>,
    select_pressed: Option<bool>,
    primary_started: bool,
}

/// Owns source handover, one-second dwell and duplicate activation admission.
#[derive(Default)]
pub struct XrPointerSession {
    surface: Option<XrPointerSurface>,
    focus: Option<Focus>,
    previous_activation: Option<(XrPointerTarget, f64)>,
    observation: Option<XrPointerObservation>,
    gamepad: Option<GamepadInput>,
    controllers: Vec<ControllerInput>,
    last_timestamp_ms: Option<f64>,
}

impl XrPointerSession {
    /// Retires hit and dwell admission while retaining physical input history.
    pub fn invalidate_selection(&mut self) {
        self.focus = None;
        self.previous_activation = None;
        self.observation = None;
        for controller in &mut self.controllers {
            controller.primary_started = false;
        }
    }

    /// Replaces measured geometry atomically and resets input on scope/layout changes.
    pub fn set_surface(&mut self, surface: Option<XrPointerSurface>) -> Result<(), &'static str> {
        if surface.as_ref().is_some_and(|value| !value.valid()) {
            return Err("Invalid XR pointer surface");
        }
        if self.surface != surface {
            let mut gamepad = self.gamepad.take();
            let scope_changed = self.surface.as_ref().map(|value| &value.scope)
                != surface.as_ref().map(|value| &value.scope);
            if scope_changed && let Some(input) = &mut gamepad {
                input.cursor = [0.5; 2];
            }
            *self = Self {
                surface,
                gamepad,
                controllers: if scope_changed {
                    Vec::new()
                } else {
                    core::mem::take(&mut self.controllers)
                },
                last_timestamp_ms: self.last_timestamp_ms,
                ..Self::default()
            };
        }
        Ok(())
    }

    /// Projects all source intersections, focus handover and dwell from one frame.
    pub fn observe(
        &mut self,
        observation: XrPointerObservation,
    ) -> Result<XrPointerProjection, &'static str> {
        if !observation.timestamp_ms.is_finite()
            || observation.timestamp_ms < 0.0
            || observation.sources.len() > 16
            || observation
                .sources
                .iter()
                .enumerate()
                .any(|(index, source)| {
                    observation.sources[..index]
                        .iter()
                        .any(|previous| previous.source_id == source.source_id)
                })
            || self
                .last_timestamp_ms
                .is_some_and(|previous| observation.timestamp_ms < previous)
        {
            return Err("Invalid XR pointer observation");
        }
        let mut pointers: Vec<_> = observation
            .sources
            .iter()
            .filter_map(XrInputSourceObservation::ray)
            .collect();
        if let Some(head) = observation.head_pose {
            let head = head.normalized().ok_or("Invalid XR head pose")?;
            pointers.push(XrPointerRay {
                source: XrPointerSource::Gaze,
                reference_from_space: Pose::IDENTITY,
                origin: head.position,
                direction: head.rotate(Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: -1.0,
                }),
            });
        }
        let mut result = XrPointerProjection::default();
        let gamepad = self.observe_gamepad(&observation);
        let (navigation, completed) = self.observe_controllers(&observation);
        if observation.focused
            && let Some(surface) = &self.surface
        {
            if let Some((_, _, back, scroll)) = &gamepad {
                result.back = *back;
                result.scroll_pages = if *back { 0.0 } else { *scroll };
            }
            let mut hits: Vec<_> = pointers
                .iter()
                .filter_map(|pointer| pointer_hit(surface, observation.panel_pose, *pointer))
                .collect();
            let retained = self.focus.as_ref().map(|focus| focus.hit.source);
            let preferred = hits
                .iter()
                .position(|hit| {
                    matches!(hit.source, XrPointerSource::Controller { .. })
                        && Some(hit.source) == retained
                })
                .or_else(|| {
                    hits.iter()
                        .position(|hit| matches!(hit.source, XrPointerSource::Controller { .. }))
                })
                .or_else(|| {
                    if gamepad.is_some() {
                        return None;
                    }
                    hits.iter()
                        .position(|hit| hit.source == XrPointerSource::Gaze)
                });
            if let Some(index) = preferred {
                result.primary = Some(hits.remove(index));
            }
            if result.primary.is_none()
                && let Some((hit, pressed, back, scroll)) = gamepad
            {
                result.primary = Some(hit.clone());
                result.back = back;
                result.scroll_pages = if back { 0.0 } else { scroll };
                if pressed && !back && hit.target.is_some() {
                    result.activation = Some(hit);
                }
            }
            result.secondary = hits
                .into_iter()
                .filter(|hit| matches!(hit.source, XrPointerSource::Controller { .. }))
                .collect();
            if let Some((back, scroll)) = navigation {
                result.back |= back;
                result.scroll_pages = if result.back { 0.0 } else { scroll };
                result.activation = None;
            }
        }
        result.focus_changed = self
            .focus
            .as_ref()
            .map(|focus| (&focus.hit.target, focus.hit.source))
            != result.primary.as_ref().map(|hit| (&hit.target, hit.source));
        let timestamp = observation.timestamp_ms;
        if result.focus_changed
            || self
                .focus
                .as_ref()
                .is_some_and(|focus| timestamp < focus.started_ms)
        {
            self.focus = result.primary.clone().map(|hit| Focus {
                hit,
                started_ms: timestamp,
                activated: false,
            });
        }
        if let Some(focus) = &mut self.focus {
            focus.hit = result
                .primary
                .clone()
                .expect("focus requires a primary cursor");
            if focus.hit.source == XrPointerSource::Gaze && focus.hit.target.is_some() {
                if result.back || result.scroll_pages != 0.0 {
                    focus.started_ms = timestamp;
                    focus.activated = false;
                }
                result.progress = ((timestamp - focus.started_ms) / 1000.0).clamp(0.0, 1.0);
                if result.progress >= 1.0 && !focus.activated {
                    focus.activated = true;
                    let hit = focus.hit.clone();
                    if self.admit(&hit, timestamp) {
                        result.activation = Some(hit);
                    }
                }
            }
        }
        if !result.back {
            let selected = completed.iter().find_map(|id| {
                result
                    .primary
                    .iter()
                    .chain(&result.secondary)
                    .find(|hit| hit.source == XrPointerSource::Controller { id: *id })
                    .cloned()
            });
            if let Some(hit) = selected
                && self.admit(&hit, timestamp)
            {
                self.focus = Some(Focus {
                    hit: hit.clone(),
                    started_ms: timestamp,
                    activated: true,
                });
                result.activation = Some(hit);
            }
        }
        if result.back {
            result.activation = None;
            result.scroll_pages = 0.0;
        }
        self.observation = Some(observation);
        self.last_timestamp_ms = Some(timestamp);
        Ok(result)
    }

    fn observe_controllers(
        &mut self,
        observation: &XrPointerObservation,
    ) -> (Option<(bool, f64)>, Vec<u32>) {
        if !observation.focused {
            self.controllers.clear();
            return (None, Vec::new());
        }
        let mut completed = Vec::new();
        self.controllers.retain(|previous| {
            observation.sources.iter().any(|source| {
                source.source_id == previous.source_id
                    && source.target_ray_mode == "tracked-pointer"
                    && (source.select_pressed.is_some()
                        || source
                            .gamepad
                            .as_ref()
                            .is_some_and(|pad| pad.mapping == "xr-standard"))
            })
        });
        let elapsed = self.last_timestamp_ms.map_or(0.0, |previous| {
            ((observation.timestamp_ms - previous) / 1000.0).clamp(0.0, 0.05)
        });
        let mut selected: Option<(bool, u32, bool, f64)> = None;
        for source in &observation.sources {
            let pad = source.gamepad.as_ref().filter(|pad| {
                source.target_ray_mode == "tracked-pointer" && pad.mapping == "xr-standard"
            });
            if source.target_ray_mode != "tracked-pointer"
                || (pad.is_none() && source.select_pressed.is_none())
            {
                continue;
            }
            let squeeze = pad
                .and_then(|pad| pad.buttons.get(1))
                .map(|button| button.pressed);
            let previous = self
                .controllers
                .iter_mut()
                .find(|previous| previous.source_id == source.source_id);
            let back = if let Some(previous) = previous {
                let back = squeeze == Some(true) && previous.squeeze == Some(false);
                previous.squeeze = squeeze;
                if source.select_pressed.is_none() || source.ray().is_none() {
                    previous.primary_started = false;
                }
                if source.ray().is_some()
                    && source.select_pressed == Some(true)
                    && previous.select_pressed == Some(false)
                {
                    previous.primary_started = true;
                }
                if source.select_pressed == Some(false) && previous.select_pressed == Some(true) {
                    if previous.primary_started {
                        completed.push(source.source_id);
                    }
                    previous.primary_started = false;
                }
                previous.select_pressed = source.select_pressed;
                back
            } else {
                self.controllers.push(ControllerInput {
                    source_id: source.source_id,
                    squeeze,
                    select_pressed: source.select_pressed,
                    primary_started: false,
                });
                false
            };
            let thumbstick = gamepad_axis(
                pad.and_then(|pad| pad.axes.get(3))
                    .copied()
                    .flatten()
                    .unwrap_or(0.0),
            );
            let touchpad = if pad
                .and_then(|pad| pad.buttons.get(2))
                .is_some_and(|button| button.touched)
            {
                gamepad_axis(
                    pad.and_then(|pad| pad.axes.get(1))
                        .copied()
                        .flatten()
                        .unwrap_or(0.0),
                )
            } else {
                0.0
            };
            let scroll = if thumbstick != 0.0 {
                thumbstick
            } else {
                touchpad
            } * elapsed
                * 2.0;
            if !back && scroll == 0.0 {
                continue;
            }
            let candidate = (source.handedness == "right", source.source_id, back, scroll);
            if selected.is_none_or(|previous| {
                (candidate.2, candidate.0, core::cmp::Reverse(candidate.1))
                    > (previous.2, previous.0, core::cmp::Reverse(previous.1))
            }) {
                selected = Some(candidate);
            }
        }
        completed.sort_by_key(|id| {
            let right = observation
                .sources
                .iter()
                .any(|source| source.source_id == *id && source.handedness == "right");
            (!right, *id)
        });
        (
            selected.map(|(_, _, back, scroll)| (back, scroll)),
            completed,
        )
    }

    fn observe_gamepad(
        &mut self,
        observation: &XrPointerObservation,
    ) -> Option<(XrPointerHit, bool, bool, f64)> {
        let sample = observation.gamepad.filter(|_| observation.focused);
        let Some(sample) = sample else {
            self.gamepad = None;
            return None;
        };
        let previous = self
            .gamepad
            .as_ref()
            .filter(|input| input.sample.source() == sample.source());
        let elapsed = previous.map_or(0.0, |input| {
            ((observation.timestamp_ms - input.timestamp_ms) / 1000.0).clamp(0.0, 0.05)
        });
        let mut cursor = previous.map_or([0.5; 2], |input| input.cursor);
        for (coordinate, axis) in cursor.iter_mut().zip(sample.axes) {
            *coordinate = (*coordinate + gamepad_axis(axis) * 1.2 * elapsed).clamp(0.0, 1.0);
        }
        let pressed = previous.is_some_and(|input| sample.buttons[0] && !input.sample.buttons[0]);
        let back = previous.is_some_and(|input| sample.buttons[1] && !input.sample.buttons[1]);
        self.gamepad = Some(GamepadInput {
            sample,
            timestamp_ms: observation.timestamp_ms,
            cursor,
        });
        let surface = self.surface.as_ref()?;
        let point = [
            (cursor[0] - 0.5) * surface.size_m[0],
            (0.5 - cursor[1]) * surface.size_m[1],
        ];
        Some((
            surface.hit(sample.source(), point),
            pressed,
            back,
            gamepad_axis(sample.axes[3]) * elapsed * 2.0,
        ))
    }

    /// Selects against the actual submitted panel pose using the retained shared hit surface.
    pub fn select_on_panel(
        &mut self,
        pointer: XrPointerRay,
        timestamp_ms: f64,
        panel_pose: Pose,
    ) -> Option<XrPointerHit> {
        let observation = self.observation.as_ref()?;
        if !observation.focused
            || matches!(pointer.source, XrPointerSource::Gamepad { .. })
            || !timestamp_ms.is_finite()
            || timestamp_ms < observation.timestamp_ms
            || !match pointer.source {
                XrPointerSource::Controller { id } => observation
                    .sources
                    .iter()
                    .any(|value| value.source_id == id && value.ray().is_some()),
                XrPointerSource::Gaze => observation.head_pose.is_some(),
                XrPointerSource::Gamepad { .. } => false,
            }
        {
            return None;
        }
        let hit = pointer_hit(self.surface.as_ref()?, panel_pose, pointer)?;
        if !self.admit(&hit, timestamp_ms) {
            return None;
        }
        self.focus = Some(Focus {
            hit: hit.clone(),
            started_ms: timestamp_ms,
            activated: true,
        });
        Some(hit)
    }

    fn admit(&mut self, hit: &XrPointerHit, timestamp: f64) -> bool {
        let Some(target) = &hit.target else {
            return false;
        };
        if self
            .previous_activation
            .as_ref()
            .is_some_and(|(previous, time)| {
                previous == target && timestamp - time >= 0.0 && timestamp - time <= 500.0
            })
        {
            return false;
        }
        self.previous_activation = Some((target.clone(), timestamp));
        true
    }
}

fn gamepad_axis(value: f64) -> f64 {
    if !value.is_finite() || value.abs() <= 0.2 {
        return 0.0;
    }
    value.signum() * (value.abs().min(1.0) - 0.2) / 0.8
}

fn pointer_hit(
    surface: &XrPointerSurface,
    panel: Pose,
    pointer: XrPointerRay,
) -> Option<XrPointerHit> {
    let panel =
        super::compose_pose(panel.normalized()?.inverse(), pointer.reference_from_space).ok()?;
    let norm = pointer
        .direction
        .array()
        .into_iter()
        .fold(0.0_f64, f64::hypot);
    if !pointer.origin.array().into_iter().all(f64::is_finite)
        || !norm.is_finite()
        || (norm - 1.0).abs() > 1.0e-5
    {
        return None;
    }
    let origin = panel.transform(pointer.origin);
    let direction = panel.rotate(Vector3 {
        x: pointer.direction.x / norm,
        y: pointer.direction.y / norm,
        z: pointer.direction.z / norm,
    });
    if direction.z >= -f64::EPSILON {
        return None;
    }
    let distance = -origin.z / direction.z;
    if !distance.is_finite() || distance < 0.0 {
        return None;
    }
    let point = [
        origin.x + direction.x * distance,
        origin.y + direction.y * distance,
    ];
    if point
        .into_iter()
        .zip(surface.size_m)
        .any(|(coordinate, size)| !coordinate.is_finite() || coordinate.abs() > size / 2.0)
    {
        return None;
    }
    Some(surface.hit(pointer.source, point))
}

#[cfg(test)]
mod tests;
