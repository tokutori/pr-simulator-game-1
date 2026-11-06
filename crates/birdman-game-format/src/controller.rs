//! Independent controller documents and aircraft/controller pair diagnostics.

use alloc::{format, string::String, vec::Vec};
use birdman_game_core::{
    ControllerReference, PidConfig, PitchController, TailAngleOfAttackGuard, TailControlError,
    TailControlProfile, YawController,
};
use serde::{Deserialize, Serialize};

/// Supported independent controller document contract.
pub const CONTROLLER_SCHEMA_VERSION: u32 = 1;
/// Maximum original controller JSON size, before decoding.
pub const MAX_CONTROLLER_JSON_BYTES: usize = 64 * 1024;

/// A controller's fixed legal cascade and operating limits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ControllerDocument {
    /// Independent JSON schema version.
    pub schema_version: u32,
    /// Stable author-assigned controller identifier.
    pub controller_id: String,
    /// Pitch's unit-correct cascade.
    pub pitch: PitchControllerDocument,
    /// Yaw's unit-correct cascade.
    pub yaw: YawControllerDocument,
    /// Controller usage intervals for elevator and rudder, in radians.
    pub incidence_bounds_rad: [[f64; 2]; 2],
    /// Body-q and body-r target intervals in radians per second.
    pub body_rate_bounds_rad_per_second: [[f64; 2]; 2],
    /// Feedback fraction used by the shared control mode.
    pub shared_authority: f64,
    /// Protection applied after mixing in every control mode.
    pub angle_of_attack_guard: ControllerAlphaGuardDocument,
}

/// A PID stage; integral bounds limit its already integrated output contribution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PidDocument {
    /// Proportional gain, in the stage's output/error units.
    pub kp: f64,
    /// Integral gain, in output/(error seconds).
    pub ki: f64,
    /// Measurement-derivative gain, in output seconds/error.
    pub kd: f64,
    /// Inclusive bounds on the integrated output contribution.
    pub integral_bounds: [f64; 2],
    /// Inclusive stage-output bounds.
    pub output_bounds: [f64; 2],
    /// Nonnegative first-order measurement-derivative filter time constant.
    pub derivative_filter_seconds: f64,
}

/// Legal pitch cascades, always ending at a body-q rate PID.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "template", rename_all = "snake_case", deny_unknown_fields)]
pub enum PitchControllerDocument {
    /// Explicit body-q target to incidence feedback.
    Rate {
        /// Body-q rate PID.
        rate: PidDocument,
    },
    /// Trim-relative pitch target to pitch Euler-rate, then body-q feedback.
    AttitudeRate {
        /// Pitch-angle PID.
        attitude: PidDocument,
        /// Body-q rate PID.
        rate: PidDocument,
        /// Normalized pitch input endpoints, relative to trim pitch.
        pitch_offset_bounds_rad: [f64; 2],
    },
    /// Airspeed to trim-relative pitch, then attitude and body-q feedback.
    AirspeedAttitudeRate {
        /// Airspeed PID using observed minus requested speed.
        airspeed: PidDocument,
        /// Pitch-angle PID.
        attitude: PidDocument,
        /// Body-q rate PID.
        rate: PidDocument,
        /// Positive requested airspeed range.
        airspeed_bounds_mps: [f64; 2],
        /// Permitted trim-relative pitch commands.
        pitch_offset_bounds_rad: [f64; 2],
    },
}

/// Legal yaw cascades, always ending at a body-r rate PID.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "template", rename_all = "snake_case", deny_unknown_fields)]
pub enum YawControllerDocument {
    /// Explicit body-r target to incidence feedback.
    Rate {
        /// Body-r rate PID.
        rate: PidDocument,
    },
    /// Held/integrated heading target to heading Euler-rate, then body-r feedback.
    HeadingRate {
        /// Periodic heading PID.
        heading: PidDocument,
        /// Body-r rate PID.
        rate: PidDocument,
        /// Normalized turn-input endpoints for heading target integration.
        heading_rate_bounds_rad_per_second: [f64; 2],
    },
}

/// Datum-angle protection parameters; trim alpha comes from the selected aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ControllerAlphaGuardDocument {
    /// Open operating interval inside the canonical angle-of-attack domain.
    pub alpha_interval_rad: [f64; 2],
    /// Positive look-ahead duration.
    pub preview_seconds: f64,
    /// Positive recovery gain.
    pub pitch_gain_seconds: f64,
}

/// Stable independent/pair diagnostic categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerValidationCode {
    /// Controller identity.
    Identity,
    /// PID coefficient or stage-output interval.
    Pid,
    /// Requested rate, attitude or speed interval.
    Target,
    /// Controller usage incidence interval.
    Incidence,
    /// Shared feedback fraction.
    Authority,
    /// Datum-angle protection.
    Guard,
    /// A constraint involving the selected aircraft.
    Pair,
}

/// The aircraft-side parameter participating in a pair constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerPeerValue {
    /// Aircraft JSON pointer or explicitly derived trim quantity.
    pub path: String,
    /// Rendered aircraft value.
    pub value: String,
}

/// One independently actionable authoring or aircraft-pair diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerValidationIssue {
    /// Stable category.
    pub code: ControllerValidationCode,
    /// Controller JSON pointer.
    pub path: String,
    /// Rendered source value.
    pub value: String,
    /// Required relation.
    pub expected: String,
    /// Aircraft-side value for pair diagnostics.
    pub aircraft: Option<ControllerPeerValue>,
}

impl core::fmt::Display for ControllerValidationIssue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:?} {}: {}; expected {}",
            self.code, self.path, self.value, self.expected
        )?;
        if let Some(peer) = &self.aircraft {
            write!(f, " (aircraft {}: {})", peer.path, peer.value)?;
        }
        Ok(())
    }
}

/// Original JSON admission, semantic collection or core construction failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerFormatError {
    /// Original bytes exceed the document budget.
    InputTooLarge,
    /// Unsupported independent document contract.
    UnsupportedSchemaVersion,
    /// JSON syntax, type or unknown-field diagnostic, with original location.
    Json(String),
    /// Independent semantic violations.
    Validation(Vec<ControllerValidationIssue>),
    /// A resolved numeric configuration failed core validation.
    Core(TailControlError),
}

impl core::fmt::Display for ControllerFormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InputTooLarge => f.write_str("Controller JSON exceeds 65536 bytes"),
            Self::UnsupportedSchemaVersion => {
                f.write_str("Controller schema version is unsupported")
            }
            Self::Json(cause) => write!(f, "Controller JSON: {cause}"),
            Self::Core(cause) => write!(f, "Controller configuration: {cause}"),
            Self::Validation(issues) => {
                for (index, issue) in issues.iter().enumerate() {
                    if index > 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{issue}")?;
                }
                Ok(())
            }
        }
    }
}

impl core::error::Error for ControllerFormatError {}

fn issue(
    issues: &mut Vec<ControllerValidationIssue>,
    code: ControllerValidationCode,
    path: &str,
    value: impl core::fmt::Debug,
    expected: &str,
) {
    issues.push(ControllerValidationIssue {
        code,
        path: path.into(),
        value: format!("{value:?}"),
        expected: expected.into(),
        aircraft: None,
    });
}

fn interval(
    issues: &mut Vec<ControllerValidationIssue>,
    code: ControllerValidationCode,
    path: &str,
    bounds: [f64; 2],
    positive_width: bool,
    neutral: bool,
) {
    if bounds.iter().any(|v| !v.is_finite())
        || bounds[0] > bounds[1]
        || positive_width && bounds[0] == bounds[1]
        || neutral && (bounds[0] > 0.0 || bounds[1] < 0.0)
    {
        issue(
            issues,
            code,
            path,
            bounds,
            if neutral {
                "finite ordered bounds containing zero"
            } else {
                "finite strictly increasing bounds"
            },
        );
    }
}

impl PidDocument {
    /// Converts one independently validated stage to the allocation-free core type.
    pub fn core_config(self) -> Result<PidConfig, TailControlError> {
        PidConfig::try_new(
            self.kp,
            self.ki,
            self.kd,
            self.integral_bounds,
            self.output_bounds,
            self.derivative_filter_seconds,
        )
    }

    fn collect(self, path: &str, issues: &mut Vec<ControllerValidationIssue>) {
        use ControllerValidationCode::Pid as C;
        for (field, value) in [
            ("kp", self.kp),
            ("ki", self.ki),
            ("kd", self.kd),
            ("derivative_filter_seconds", self.derivative_filter_seconds),
        ] {
            if !value.is_finite() || value < 0.0 {
                issue(
                    issues,
                    C,
                    &format!("{path}/{field}"),
                    value,
                    "finite nonnegative value",
                );
            }
        }
        interval(
            issues,
            C,
            &format!("{path}/integral_bounds"),
            self.integral_bounds,
            false,
            true,
        );
        interval(
            issues,
            C,
            &format!("{path}/output_bounds"),
            self.output_bounds,
            false,
            true,
        );
    }
}

impl ControllerDocument {
    /// Decodes bounded original bytes and collects independent semantic violations.
    pub fn decode_json(bytes: &[u8]) -> Result<Self, ControllerFormatError> {
        if bytes.len() > MAX_CONTROLLER_JSON_BYTES {
            return Err(ControllerFormatError::InputTooLarge);
        }
        let document: Self = serde_json::from_slice(bytes)
            .map_err(|e| ControllerFormatError::Json(format!("{e}")))?;
        document.validate()?;
        Ok(document)
    }

    /// Validates the controller independently of the currently selected aircraft.
    pub fn validate(&self) -> Result<(), ControllerFormatError> {
        if self.schema_version != CONTROLLER_SCHEMA_VERSION {
            return Err(ControllerFormatError::UnsupportedSchemaVersion);
        }
        let issues = self.validation_issues();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(ControllerFormatError::Validation(issues))
        }
    }

    /// Returns all independent field-level authoring violations.
    pub fn validation_issues(&self) -> Vec<ControllerValidationIssue> {
        use ControllerValidationCode as C;
        let mut issues = Vec::new();
        if self.controller_id.is_empty()
            || self.controller_id.len() > 128
            || !self
                .controller_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            issue(
                &mut issues,
                C::Identity,
                "/controller_id",
                &self.controller_id,
                "1..=128 ASCII letters, digits, hyphens, underscores or dots",
            );
        }
        let pitch_offsets = match self.pitch {
            PitchControllerDocument::Rate { rate } => {
                rate.collect("/pitch/rate", &mut issues);
                None
            }
            PitchControllerDocument::AttitudeRate {
                attitude,
                rate,
                pitch_offset_bounds_rad,
            } => {
                attitude.collect("/pitch/attitude", &mut issues);
                rate.collect("/pitch/rate", &mut issues);
                Some(pitch_offset_bounds_rad)
            }
            PitchControllerDocument::AirspeedAttitudeRate {
                airspeed,
                attitude,
                rate,
                airspeed_bounds_mps,
                pitch_offset_bounds_rad,
            } => {
                airspeed.collect("/pitch/airspeed", &mut issues);
                attitude.collect("/pitch/attitude", &mut issues);
                rate.collect("/pitch/rate", &mut issues);
                interval(
                    &mut issues,
                    C::Target,
                    "/pitch/airspeed_bounds_mps",
                    airspeed_bounds_mps,
                    true,
                    false,
                );
                if airspeed_bounds_mps[0] <= 0.0 {
                    issue(
                        &mut issues,
                        C::Target,
                        "/pitch/airspeed_bounds_mps/0",
                        airspeed_bounds_mps[0],
                        "positive airspeed",
                    );
                }
                Some(pitch_offset_bounds_rad)
            }
        };
        if let Some(bounds) = pitch_offsets {
            interval(
                &mut issues,
                C::Target,
                "/pitch/pitch_offset_bounds_rad",
                bounds,
                true,
                true,
            );
        }
        match self.yaw {
            YawControllerDocument::Rate { rate } => rate.collect("/yaw/rate", &mut issues),
            YawControllerDocument::HeadingRate {
                heading,
                rate,
                heading_rate_bounds_rad_per_second,
            } => {
                heading.collect("/yaw/heading", &mut issues);
                rate.collect("/yaw/rate", &mut issues);
                interval(
                    &mut issues,
                    C::Target,
                    "/yaw/heading_rate_bounds_rad_per_second",
                    heading_rate_bounds_rad_per_second,
                    true,
                    true,
                );
            }
        }
        for axis in 0..2 {
            interval(
                &mut issues,
                C::Incidence,
                &format!("/incidence_bounds_rad/{axis}"),
                self.incidence_bounds_rad[axis],
                true,
                true,
            );
            for (index, angle) in self.incidence_bounds_rad[axis].into_iter().enumerate() {
                if angle.abs() > core::f64::consts::PI {
                    issue(
                        &mut issues,
                        C::Incidence,
                        &format!("/incidence_bounds_rad/{axis}/{index}"),
                        angle,
                        "angle in [-pi,pi]",
                    );
                }
            }
            interval(
                &mut issues,
                C::Target,
                &format!("/body_rate_bounds_rad_per_second/{axis}"),
                self.body_rate_bounds_rad_per_second[axis],
                true,
                true,
            );
        }
        if !self.shared_authority.is_finite() || !(0.0..=1.0).contains(&self.shared_authority) {
            issue(
                &mut issues,
                C::Authority,
                "/shared_authority",
                self.shared_authority,
                "finite feedback fraction in [0,1]",
            );
        }
        let guard = self.angle_of_attack_guard;
        interval(
            &mut issues,
            C::Guard,
            "/angle_of_attack_guard/alpha_interval_rad",
            guard.alpha_interval_rad,
            true,
            false,
        );
        for (index, value) in guard.alpha_interval_rad.into_iter().enumerate() {
            if value <= -core::f64::consts::FRAC_PI_2 || value >= core::f64::consts::FRAC_PI_2 {
                issue(
                    &mut issues,
                    C::Guard,
                    &format!("/angle_of_attack_guard/alpha_interval_rad/{index}"),
                    value,
                    "strictly inside (-pi/2,pi/2)",
                );
            }
        }
        for (field, value) in [
            ("preview_seconds", guard.preview_seconds),
            ("pitch_gain_seconds", guard.pitch_gain_seconds),
        ] {
            if !value.is_finite() || value <= 0.0 {
                issue(
                    &mut issues,
                    C::Guard,
                    &format!("/angle_of_attack_guard/{field}"),
                    value,
                    "finite positive value",
                );
            }
        }
        issues
    }

    /// Reports constraints against the latest aircraft, without rejecting independent selection.
    pub fn pair_issues(
        &self,
        aircraft: &crate::aircraft::AircraftDocument,
        trim_pitch_rad: f64,
        trim_alpha_rad: f64,
    ) -> Vec<ControllerValidationIssue> {
        let mut issues = Vec::new();
        let mut pair = |path: &str,
                        value: String,
                        expected: &str,
                        aircraft_path: String,
                        aircraft_value: String| {
            issues.push(ControllerValidationIssue {
                code: ControllerValidationCode::Pair,
                path: path.into(),
                value,
                expected: expected.into(),
                aircraft: Some(ControllerPeerValue {
                    path: aircraft_path,
                    value: aircraft_value,
                }),
            });
        };
        for (axis, role) in [
            crate::aircraft::AircraftSurfaceRole::HorizontalTail,
            crate::aircraft::AircraftSurfaceRole::VerticalTail,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some((index, surface)) = aircraft
                .surfaces
                .iter()
                .enumerate()
                .find(|(_, s)| s.role == role)
                && let Some(actuator) = surface.actuator
            {
                let used = self.incidence_bounds_rad[axis];
                let physical = actuator.incidence_bounds_rad;
                if used[0] < physical[0] || used[1] > physical[1] {
                    pair(
                        &format!("/incidence_bounds_rad/{axis}"),
                        format!("{used:?}"),
                        "contained in the aircraft mechanical interval",
                        format!("/surfaces/{index}/actuator/incidence_bounds_rad"),
                        format!("{physical:?}"),
                    );
                }
            }
        }
        let alpha = self.angle_of_attack_guard.alpha_interval_rad;
        if trim_alpha_rad <= alpha[0] || trim_alpha_rad >= alpha[1] {
            pair(
                "/angle_of_attack_guard/alpha_interval_rad",
                format!("{alpha:?}"),
                "strictly contains aircraft trim alpha",
                "/trim/derived_alpha_rad".into(),
                format!("{trim_alpha_rad:?}"),
            );
        }
        let offsets = match self.pitch {
            PitchControllerDocument::Rate { .. } => None,
            PitchControllerDocument::AttitudeRate {
                pitch_offset_bounds_rad,
                ..
            } => Some(pitch_offset_bounds_rad),
            PitchControllerDocument::AirspeedAttitudeRate {
                airspeed_bounds_mps,
                pitch_offset_bounds_rad,
                ..
            } => {
                if aircraft.trim.airspeed_mps < airspeed_bounds_mps[0]
                    || aircraft.trim.airspeed_mps > airspeed_bounds_mps[1]
                {
                    pair(
                        "/pitch/airspeed_bounds_mps",
                        format!("{airspeed_bounds_mps:?}"),
                        "contains aircraft trim airspeed",
                        "/trim/airspeed_mps".into(),
                        format!("{:?}", aircraft.trim.airspeed_mps),
                    );
                }
                Some(pitch_offset_bounds_rad)
            }
        };
        if let Some(offsets) = offsets {
            let resolved = [trim_pitch_rad + offsets[0], trim_pitch_rad + offsets[1]];
            if resolved
                .iter()
                .any(|v| !v.is_finite() || v.abs() >= core::f64::consts::FRAC_PI_2)
            {
                pair(
                    "/pitch/pitch_offset_bounds_rad",
                    format!("{offsets:?}"),
                    "trim plus offset remains in regular pitch (-pi/2,pi/2)",
                    "/trim/derived_pitch_rad".into(),
                    format!("{trim_pitch_rad:?}"),
                );
            }
        }
        issues
    }

    /// Resolves a validated document against the selected aircraft's actual trim.
    pub fn core_profile(
        &self,
        reference: ControllerReference,
        trim_alpha_rad: f64,
    ) -> Result<TailControlProfile, ControllerFormatError> {
        self.validate()?;
        let build = || -> Result<TailControlProfile, TailControlError> {
            let pitch = match self.pitch {
                PitchControllerDocument::Rate { rate } => PitchController::Rate {
                    rate: rate.core_config()?,
                },
                PitchControllerDocument::AttitudeRate {
                    attitude,
                    rate,
                    pitch_offset_bounds_rad,
                } => PitchController::AttitudeRate {
                    attitude: attitude.core_config()?,
                    rate: rate.core_config()?,
                    pitch_offset_bounds_rad,
                },
                PitchControllerDocument::AirspeedAttitudeRate {
                    airspeed,
                    attitude,
                    rate,
                    airspeed_bounds_mps,
                    pitch_offset_bounds_rad,
                } => PitchController::AirspeedAttitudeRate {
                    airspeed: airspeed.core_config()?,
                    attitude: attitude.core_config()?,
                    rate: rate.core_config()?,
                    airspeed_bounds_mps,
                    pitch_offset_bounds_rad,
                },
            };
            let yaw = match self.yaw {
                YawControllerDocument::Rate { rate } => YawController::Rate {
                    rate: rate.core_config()?,
                },
                YawControllerDocument::HeadingRate {
                    heading,
                    rate,
                    heading_rate_bounds_rad_per_second,
                } => YawController::HeadingRate {
                    heading: heading.core_config()?,
                    rate: rate.core_config()?,
                    heading_rate_bounds_rad_per_second,
                },
            };
            let profile = TailControlProfile::try_new(
                pitch,
                yaw,
                self.incidence_bounds_rad,
                self.body_rate_bounds_rad_per_second,
                self.shared_authority,
                reference,
            )?;
            let guard = self.angle_of_attack_guard;
            let guard = TailAngleOfAttackGuard::try_new(
                guard.alpha_interval_rad,
                trim_alpha_rad,
                guard.preview_seconds,
                guard.pitch_gain_seconds,
            )
            .map_err(TailControlError::Actuator)?;
            Ok(profile.with_angle_of_attack_guard(guard))
        };
        build().map_err(ControllerFormatError::Core)
    }
}
