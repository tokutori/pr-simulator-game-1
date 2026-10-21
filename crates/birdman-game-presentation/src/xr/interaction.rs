use super::{XrPose, XrVector};
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
}

/// Measured surface geometry owned until its scope or layout changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPointerSurface {
    /// Adapter's exact scene, overlay, panel and scroll context identity.
    pub scope: String,
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
                region.bounds.valid()
                    && match &region.target {
                        XrPointerTarget::Control { id } => !id.is_empty() && id.len() <= 1024,
                        XrPointerTarget::Page { .. } => true,
                    }
            })
    }

    fn hit(&self, point: [f64; 2]) -> Option<XrPointerTarget> {
        let surface = [
            point[0] + self.size_m[0] / 2.0,
            self.size_m[1] / 2.0 - point[1],
        ];
        self.regions
            .iter()
            .find(|region| region.bounds.contains(surface))
            .map(|region| region.target.clone())
    }
}

/// Pointing ray in the same reference frame as the panel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XrPointerRay {
    /// Stable input source.
    pub source: XrPointerSource,
    /// Ray origin in reference coordinates, in metres.
    pub origin: XrVector,
    /// Unit pointing direction in reference coordinates.
    pub direction: XrVector,
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
    pub panel_pose: XrPose,
    /// Tracked controllers and head gaze observed in the same frame.
    pub pointers: Vec<XrPointerRay>,
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
}

#[derive(Clone)]
struct Focus {
    hit: XrPointerHit,
    started_ms: f64,
    activated: bool,
}

/// Owns source handover, one-second dwell and duplicate activation admission.
#[derive(Default)]
pub struct XrPointerSession {
    surface: Option<XrPointerSurface>,
    focus: Option<Focus>,
    previous_activation: Option<(XrPointerTarget, f64)>,
    observation: Option<XrPointerObservation>,
}

impl XrPointerSession {
    /// Replaces measured geometry atomically and resets input on scope/layout changes.
    pub fn set_surface(&mut self, surface: Option<XrPointerSurface>) -> Result<(), &'static str> {
        if surface.as_ref().is_some_and(|value| !value.valid()) {
            return Err("Invalid XR pointer surface");
        }
        if self.surface != surface {
            *self = Self {
                surface,
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
            || observation.pointers.len() > 16
        {
            return Err("Invalid XR pointer observation");
        }
        let mut result = XrPointerProjection::default();
        if observation.focused
            && let Some(surface) = &self.surface
        {
            let mut hits: Vec<_> = observation
                .pointers
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
                    hits.iter()
                        .position(|hit| hit.source == XrPointerSource::Gaze)
                });
            if let Some(index) = preferred {
                result.primary = Some(hits.remove(index));
            }
            result.secondary = hits
                .into_iter()
                .filter(|hit| matches!(hit.source, XrPointerSource::Controller { .. }))
                .collect();
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
        self.observation = Some(observation);
        Ok(result)
    }

    /// Uses the same panel geometry for explicit trigger selection.
    pub fn select(&mut self, pointer: XrPointerRay, timestamp_ms: f64) -> Option<XrPointerHit> {
        let observation = self.observation.as_ref()?;
        if !observation.focused
            || !timestamp_ms.is_finite()
            || timestamp_ms < observation.timestamp_ms
            || !observation
                .pointers
                .iter()
                .any(|value| value.source == pointer.source)
        {
            return None;
        }
        let hit = pointer_hit(self.surface.as_ref()?, observation.panel_pose, pointer)?;
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

fn pointer_hit(
    surface: &XrPointerSurface,
    panel: XrPose,
    pointer: XrPointerRay,
) -> Option<XrPointerHit> {
    let panel = panel.normalized()?.inverse();
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
    let direction = panel.rotate(XrVector {
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
    Some(XrPointerHit {
        source: pointer.source,
        point,
        target: surface.hit(point),
    })
}

#[cfg(test)]
mod tests;
