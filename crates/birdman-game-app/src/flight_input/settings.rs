use serde::{Deserialize, Serialize};

/// Keyboard response curve applied to the fixed-tick ramp position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum KeyboardResponseCurve {
    /// Constant normalized ramp speed.
    Linear,
    /// Smoothstep of the ramp position, with finite arrival at each endpoint.
    Smoothstep,
}

/// Device response selection sealed independently of the physical controller.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct KeyboardControlSettings {
    /// Seconds from neutral to full ramp position.
    pub ramp_seconds: f64,
    /// Curve of the normalized ramp position.
    pub curve: KeyboardResponseCurve,
    /// Maximum normalized nose-up and turn demand.
    pub sensitivity: f64,
    /// Speed of the pilot's target position, in metres per second.
    pub pilot_target_rate_mps: f64,
}

impl Default for KeyboardControlSettings {
    fn default() -> Self {
        Self {
            ramp_seconds: 0.5,
            curve: KeyboardResponseCurve::Linear,
            sensitivity: 1.0,
            pilot_target_rate_mps: 0.4,
        }
    }
}

impl KeyboardControlSettings {
    /// Validates the complete selection before it can replace the retained settings.
    pub(crate) fn is_valid(self) -> bool {
        self.ramp_seconds.is_finite()
            && (0.1..=2.0).contains(&self.ramp_seconds)
            && self.sensitivity.is_finite()
            && (0.1..=1.0).contains(&self.sensitivity)
            && self.pilot_target_rate_mps.is_finite()
            && (0.1..=0.8).contains(&self.pilot_target_rate_mps)
    }

    pub(super) fn demand(self, ramp: f64) -> f64 {
        let magnitude = ramp.abs();
        let shaped = match self.curve {
            KeyboardResponseCurve::Linear => magnitude,
            KeyboardResponseCurve::Smoothstep => magnitude * magnitude * (3.0 - 2.0 * magnitude),
        };
        ramp.signum() * shaped * self.sensitivity
    }
}
