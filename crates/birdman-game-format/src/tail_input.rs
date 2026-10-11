use crate::TailPilotPositionCommandDocument;

/// Current two-tail logical input document version.
pub const TAIL_INPUT_SCHEMA_VERSION: u32 = 2;

/// Control layout accepted by the current live session boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TailControlLayoutDocument {
    /// Independently controlled horizontal and vertical tail incidence.
    TailIncidence,
}

/// Wire representation of one logical input interval before core validation.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TailInputDocument {
    /// Wire version checked before advancing the session.
    #[cfg_attr(feature = "schema", schemars(extend("const" = TAIL_INPUT_SCHEMA_VERSION)))]
    pub schema_version: u32,
    /// Current two-tail control layout without a surplus roll axis.
    pub control_layout: TailControlLayoutDocument,
    /// Normalized positive nose-up intent in the inclusive interval [-1, 1].
    #[cfg_attr(feature = "schema", schemars(range(min = -1.0, max = 1.0)))]
    pub nose_up: f64,
    /// Normalized positive right-turn intent in the inclusive interval [-1, 1].
    #[cfg_attr(feature = "schema", schemars(range(min = -1.0, max = 1.0)))]
    pub turn_right: f64,
    /// Desired body-positive pitch rate in rad/s, bounded by the core controller contract.
    pub desired_pitch_rate_rad_s: f64,
    /// Desired body-positive yaw rate in rad/s, bounded by the core controller contract.
    pub desired_yaw_rate_rad_s: f64,
    /// Preserve the target or resolve a new normalized longitudinal-position target.
    pub pilot_position_command: TailPilotPositionCommandDocument,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_roundtrip_preserves_hold_set_and_explicit_rate_targets() {
        for pilot_position_command in [
            serde_json::json!({"kind": "hold"}),
            serde_json::json!({"kind": "set", "normalized": -1.0}),
        ] {
            let value = serde_json::json!({
                "schema_version": 2,
                "control_layout": "tail_incidence",
                "nose_up": -1.0,
                "turn_right": 1.0,
                "desired_pitch_rate_rad_s": 0.201,
                "desired_yaw_rate_rad_s": -0.2,
                "pilot_position_command": pilot_position_command,
            });
            let document: TailInputDocument = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(document).unwrap(), value);
            for field in value.as_object().unwrap().keys() {
                let mut missing = value.clone();
                missing.as_object_mut().unwrap().remove(field);
                assert!(serde_json::from_value::<TailInputDocument>(missing).is_err());
                let mut absent = value.clone();
                absent[field] = serde_json::Value::Null;
                assert!(serde_json::from_value::<TailInputDocument>(absent).is_err());
            }
            let mut surplus = value;
            surplus["roll"] = serde_json::json!(0.0);
            assert!(serde_json::from_value::<TailInputDocument>(surplus).is_err());
        }
    }
}
