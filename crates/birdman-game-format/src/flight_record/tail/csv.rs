use super::super::FlightRecordFormatError;
use super::{
    TailFlightRecordControlsDocument, TailFlightRecordDocument, TailFlightRecordSampleDocument,
    TailPilotPositionCommandDocument,
};
use alloc::{format, string::String, string::ToString, vec, vec::Vec};

const INPUT_COLUMNS: &[&str] = &[
    "input_available",
    "pilot_intent_nose_up_normalized",
    "pilot_intent_turn_right_normalized",
    "target_angular_rate_body_q_rad_s",
    "target_angular_rate_body_r_rad_s",
    "pilot_position_command_kind",
    "pilot_position_command_normalized",
    "resolved_pilot_position_target_body_forward_m",
    "manual_horizontal_tail_incidence_target_rad",
    "manual_vertical_tail_incidence_target_rad",
    "fbw_horizontal_tail_incidence_target_rad",
    "fbw_vertical_tail_incidence_target_rad",
    "mixed_horizontal_tail_incidence_target_rad",
    "mixed_vertical_tail_incidence_target_rad",
];

const COLUMNS: &[&str] = &[
    "log_export_version",
    "record_schema_version",
    "control_layout",
    "catalog_version",
    "scenario_id",
    "scenario_version",
    "aircraft_model_version",
    "environment_version",
    "controller_profile_version",
    "seed_u64",
    "maximum_flight_ticks",
    "physics_hz",
    "score_definition_version",
    "physics_model_version",
    "personal_best_key_hex",
    "difficulty_preset",
    "information_level",
    "assistance_level",
    "weather_class",
    "hud_profile_available",
    "hud_telemetry",
    "hud_attitude",
    "hud_wind",
    "hud_flight_path",
    "hud_angle_of_attack",
    "hud_warnings",
    "aircraft_configuration_id_json",
    "controller_profile_id_json",
    "tick_index",
    "fraction",
    "time_s",
    "datum_position_ned_n_m",
    "datum_position_ned_e_m",
    "datum_position_ned_d_m",
    "datum_velocity_ned_n_mps",
    "datum_velocity_ned_e_mps",
    "datum_velocity_ned_d_mps",
    "attitude_body_to_ned_w",
    "attitude_body_to_ned_x",
    "attitude_body_to_ned_y",
    "attitude_body_to_ned_z",
    "angular_velocity_body_p_rad_s",
    "angular_velocity_body_q_rad_s",
    "angular_velocity_body_r_rad_s",
    "pilot_position_body_forward_m",
    "pilot_velocity_relative_body_forward_mps",
    "physical_horizontal_tail_incidence_rad",
    "physical_vertical_tail_incidence_rad",
    "wind_at_cg_ned_n_mps",
    "wind_at_cg_ned_e_mps",
    "wind_at_cg_ned_d_mps",
    "composite_cg_position_ned_n_m",
    "composite_cg_position_ned_e_m",
    "composite_cg_position_ned_d_m",
    "altitude_m",
    "airspeed_mps",
    "groundspeed_mps",
    "angle_of_attack_rad",
    "sideslip_angle_rad",
    "attitude_roll_rad",
    "attitude_pitch_rad",
    "attitude_heading_rad",
    "input_available",
    "pilot_intent_nose_up_normalized",
    "pilot_intent_turn_right_normalized",
    "target_angular_rate_body_q_rad_s",
    "target_angular_rate_body_r_rad_s",
    "pilot_position_command_kind",
    "pilot_position_command_normalized",
    "resolved_pilot_position_target_body_forward_m",
    "manual_horizontal_tail_incidence_target_rad",
    "manual_vertical_tail_incidence_target_rad",
    "fbw_horizontal_tail_incidence_target_rad",
    "fbw_vertical_tail_incidence_target_rad",
    "mixed_horizontal_tail_incidence_target_rad",
    "mixed_vertical_tail_incidence_target_rad",
    "terminal_reason",
    "terminal_disposition",
    "terminal_tick",
    "terminal_fraction",
    "terminal_time_s",
    "terminal_score_available",
    "terminal_course_parallel_m",
    "terminal_cross_track_m",
    "terminal_net_horizontal_m",
    "terminal_failure_available",
    "terminal_failure_json",
    "acceleration_estimate_status",
    "acceleration_estimate_method",
    "acceleration_stencil_start_s",
    "acceleration_stencil_end_s",
    "estimated_datum_acceleration_ned_n_mps2",
    "estimated_datum_acceleration_ned_e_mps2",
    "estimated_datum_acceleration_ned_d_mps2",
    "estimated_angular_acceleration_body_p_rad_s2",
    "estimated_angular_acceleration_body_q_rad_s2",
    "estimated_angular_acceleration_body_r_rad_s2",
    "estimated_pilot_acceleration_relative_body_forward_mps2",
];

pub(super) fn encode(
    document: &TailFlightRecordDocument,
) -> Result<Vec<u8>, FlightRecordFormatError> {
    document.validate()?;
    let columns = COLUMNS;
    let header = &document.header;
    let mut metadata = vec![
        "2".to_string(),
        document.schema_version.to_string(),
        "tail_incidence".to_string(),
        header.catalog_version.to_string(),
        header.scenario_id.to_string(),
        header.scenario_version.to_string(),
        header.aircraft_model_version.to_string(),
        header.environment_version.to_string(),
        header.controller_profile_version.to_string(),
        header.seed.to_string(),
        header.maximum_flight_ticks.to_string(),
        header.physics_hz.to_string(),
        header.score_definition_version.to_string(),
        header.physics_model_version.to_string(),
        header.personal_best_key.map_or_else(String::new, |digest| {
            digest.iter().map(|byte| format!("{byte:02x}")).collect()
        }),
        enum_name(&header.difficulty.preset)?,
        enum_name(&header.difficulty.information)?,
        enum_name(&header.difficulty.assistance)?,
        enum_name(&header.difficulty.weather)?,
        header.difficulty.hud_profile.is_some().to_string(),
    ];
    if let Some(profile) = header.difficulty.hud_profile {
        metadata.extend(
            [
                profile.telemetry,
                profile.attitude,
                profile.wind,
                profile.flight_path,
                profile.angle_of_attack,
                profile.warnings,
            ]
            .map(|value| value.to_string()),
        );
    } else {
        metadata.extend(core::iter::repeat_n(String::new(), 6));
    }
    metadata.extend([
        json(&document.control_identity.aircraft_configuration_id)?,
        json(&document.control_identity.controller_profile_id)?,
    ]);
    let finalization = &document.finalization;
    let mut terminal = vec![
        enum_name(&finalization.reason)?,
        enum_name(&finalization.disposition)?,
        finalization.terminal_tick.to_string(),
        finalization.terminal_fraction.to_string(),
        time_s(
            finalization.terminal_tick,
            finalization.terminal_fraction,
            header.physics_hz,
        )
        .to_string(),
        finalization.score_m.is_some().to_string(),
    ];
    terminal.extend(finalization.score_m.map_or_else(
        || vec![String::new(); 3],
        |score| score.map(|value| value.to_string()).into(),
    ));
    terminal.extend([
        finalization.failure.is_some().to_string(),
        finalization
            .failure
            .as_ref()
            .map(json)
            .transpose()?
            .unwrap_or_default(),
    ]);
    let mut output = String::new();
    write_row(&mut output, columns.iter().copied());
    for (index, sample) in document.samples.iter().enumerate() {
        let mut row = metadata.clone();
        row.extend([
            sample.tick_index.to_string(),
            sample.fraction.to_string(),
            time_s(sample.tick_index, sample.fraction, header.physics_hz).to_string(),
        ]);
        let state = &sample.state;
        numbers(&mut row, state.datum_position_ned_m);
        numbers(&mut row, state.datum_velocity_ned_mps);
        numbers(&mut row, state.attitude_body_to_ned);
        numbers(&mut row, state.angular_velocity_body_rad_s);
        numbers(&mut row, [state.pilot_position_m, state.pilot_velocity_mps]);
        let TailFlightRecordControlsDocument::TailIncidence {
            physical_incidence,
            input_from_previous,
        } = &sample.controls;
        numbers(
            &mut row,
            [
                physical_incidence.horizontal_tail_rad,
                physical_incidence.vertical_tail_rad,
            ],
        );
        numbers(&mut row, state.wind_at_cg_ned_mps);
        numbers(&mut row, state.telemetry.composite_cg_position_ned_m);
        numbers(
            &mut row,
            [
                state.telemetry.altitude_m,
                state.telemetry.airspeed_mps,
                state.telemetry.groundspeed_mps,
            ],
        );
        row.extend([
            optional_number(state.telemetry.angle_of_attack_rad),
            optional_number(state.telemetry.sideslip_angle_rad),
        ]);
        numbers(&mut row, state.telemetry.attitude_euler_rad);
        row.push(input_from_previous.is_some().to_string());
        if let Some(input) = input_from_previous {
            numbers(
                &mut row,
                [
                    input.nose_up,
                    input.turn_right,
                    input.desired_pitch_rate_rad_s,
                    input.desired_yaw_rate_rad_s,
                ],
            );
            match input.pilot_position_command {
                TailPilotPositionCommandDocument::Hold {} => {
                    row.extend(["hold".to_string(), String::new()])
                }
                TailPilotPositionCommandDocument::Set { normalized } => {
                    row.extend(["set".to_string(), normalized.to_string()])
                }
            }
            row.push(input.resolved_pilot_position_target_m.to_string());
            for incidence in [
                input.manual_incidence_target,
                input.fbw_incidence_target,
                input.mixed_incidence_target,
            ] {
                numbers(
                    &mut row,
                    [incidence.horizontal_tail_rad, incidence.vertical_tail_rad],
                );
            }
        } else {
            row.extend(core::iter::repeat_n(String::new(), INPUT_COLUMNS.len() - 1));
        }
        row.extend(terminal.iter().cloned());
        append_estimate(
            &mut row,
            &document.samples,
            index,
            header.physics_hz,
            motion,
        );
        if row.len() != columns.len() {
            return Err(FlightRecordFormatError::EncodingFailed);
        }
        write_row(&mut output, row.iter().map(String::as_str));
    }
    Ok(output.into_bytes())
}

fn json(value: &impl serde::Serialize) -> Result<String, FlightRecordFormatError> {
    serde_json::to_string(value).map_err(|_| FlightRecordFormatError::EncodingFailed)
}

fn time_s(tick_index: u64, fraction: f64, physics_hz: u32) -> f64 {
    (tick_index as f64 + fraction) / f64::from(physics_hz)
}

fn motion(sample: &TailFlightRecordSampleDocument) -> MotionSample {
    let state = &sample.state;
    let [north, east, down] = state.datum_velocity_ned_mps;
    let [roll, pitch, yaw] = state.angular_velocity_body_rad_s;
    MotionSample {
        tick_index: sample.tick_index,
        fraction: sample.fraction,
        velocities: [
            north,
            east,
            down,
            roll,
            pitch,
            yaw,
            state.pilot_velocity_mps,
        ],
    }
}

enum AccelerationEstimate {
    Available {
        values: [f64; 7],
        start_s: f64,
        end_s: f64,
        method: &'static str,
    },
    Unavailable(&'static str),
}

struct MotionSample {
    tick_index: u64,
    fraction: f64,
    velocities: [f64; 7],
}

fn append_estimate<Sample>(
    row: &mut Vec<String>,
    samples: &[Sample],
    index: usize,
    physics_hz: u32,
    motion: impl Fn(&Sample) -> MotionSample,
) {
    match estimate(samples, index, physics_hz, motion) {
        AccelerationEstimate::Available {
            values,
            start_s,
            end_s,
            method,
        } => {
            row.extend([
                "available".to_string(),
                method.to_string(),
                start_s.to_string(),
                end_s.to_string(),
            ]);
            numbers(row, values);
        }
        AccelerationEstimate::Unavailable(reason) => {
            row.extend([reason.to_string(), "unavailable".to_string()]);
            row.extend(core::iter::repeat_n(String::new(), 9));
        }
    }
}

fn enum_name(value: &impl serde::Serialize) -> Result<String, FlightRecordFormatError> {
    match serde_json::to_value(value).map_err(|_| FlightRecordFormatError::EncodingFailed)? {
        serde_json::Value::String(name) => Ok(name),
        _ => Err(FlightRecordFormatError::EncodingFailed),
    }
}

fn optional_number(value: Option<impl ToString>) -> String {
    value.map_or_else(String::new, |number| number.to_string())
}

fn numbers<const COUNT: usize>(row: &mut Vec<String>, values: [f64; COUNT]) {
    row.extend(values.map(|value| value.to_string()));
}

fn write_row<'value>(output: &mut String, values: impl Iterator<Item = &'value str>) {
    for (index, value) in values.enumerate() {
        if index > 0 {
            output.push(',');
        }
        if value.contains([',', '"', '\n', '\r']) {
            output.push('"');
            output.push_str(&value.replace('"', "\"\""));
            output.push('"');
        } else {
            output.push_str(value);
        }
    }
    output.push('\n');
}

fn interval_s(start: &MotionSample, end: &MotionSample, physics_hz: u32) -> f64 {
    ((end.tick_index - start.tick_index) as f64 + end.fraction - start.fraction)
        / f64::from(physics_hz)
}

fn motion_time_s(sample: &MotionSample, physics_hz: u32) -> f64 {
    (sample.tick_index as f64 + sample.fraction) / f64::from(physics_hz)
}

fn estimate<Sample>(
    samples: &[Sample],
    index: usize,
    physics_hz: u32,
    motion: impl Fn(&Sample) -> MotionSample,
) -> AccelerationEstimate {
    if samples.len() < 2 {
        return AccelerationEstimate::Unavailable("insufficient_samples");
    }
    let interior = index > 0 && index + 1 < samples.len();
    let start_index = if index == 0 { 0 } else { index - 1 };
    let end_index = if interior { index + 1 } else { start_index + 1 };
    let start = motion(&samples[start_index]);
    let end = motion(&samples[end_index]);
    let span = interval_s(&start, &end, physics_hz);
    if !span.is_finite()
        || span <= 0.0
        || motion_time_s(&end, physics_hz) <= motion_time_s(&start, physics_hz)
    {
        return AccelerationEstimate::Unavailable("invalid_interval");
    }
    let start_values = start.velocities;
    let end_values = end.velocities;
    let mut values: [f64; 7] =
        core::array::from_fn(|axis| (end_values[axis] - start_values[axis]) / span);
    if interior {
        let current = motion(&samples[index]);
        let before = interval_s(&start, &current, physics_hz);
        let after = interval_s(&current, &end, physics_hz);
        if !before.is_finite()
            || !after.is_finite()
            || before <= 0.0
            || after <= 0.0
            || motion_time_s(&current, physics_hz) <= motion_time_s(&start, physics_hz)
            || motion_time_s(&end, physics_hz) <= motion_time_s(&current, physics_hz)
        {
            return AccelerationEstimate::Unavailable("invalid_interval");
        }
        let current_values = current.velocities;
        values = core::array::from_fn(|axis| {
            (after / span) * ((current_values[axis] - start_values[axis]) / before)
                + (before / span) * ((end_values[axis] - current_values[axis]) / after)
        });
    }
    if values.iter().any(|value| !value.is_finite()) {
        return AccelerationEstimate::Unavailable("non_finite_estimate");
    }
    AccelerationEstimate::Available {
        values,
        start_s: motion_time_s(&start, physics_hz),
        end_s: motion_time_s(&end, physics_hz),
        method: if interior {
            "nonuniform_three_point"
        } else {
            "one_sided_two_point"
        },
    }
}
