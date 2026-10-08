use super::super::{FlightRecordFormatError, csv as shared};
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

pub(super) fn encode(
    document: &TailFlightRecordDocument,
) -> Result<Vec<u8>, FlightRecordFormatError> {
    document.validate()?;
    let mut columns = shared::COLUMNS[..26].to_vec();
    columns.extend([
        "aircraft_configuration_id_json",
        "controller_profile_id_json",
    ]);
    columns.extend_from_slice(&shared::COLUMNS[26..44]);
    columns.extend([
        "physical_horizontal_tail_incidence_rad",
        "physical_vertical_tail_incidence_rad",
    ]);
    columns.extend_from_slice(&shared::COLUMNS[47..61]);
    columns.extend_from_slice(INPUT_COLUMNS);
    columns.extend_from_slice(&shared::COLUMNS[75..84]);
    columns.extend(["terminal_failure_available", "terminal_failure_json"]);
    columns.extend_from_slice(&shared::COLUMNS[84..]);
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
        shared::optional_number(header.score_definition_version),
        shared::optional_number(header.physics_model_version),
        header.personal_best_key.map_or_else(String::new, |digest| {
            digest.iter().map(|byte| format!("{byte:02x}")).collect()
        }),
        shared::enum_name(&header.difficulty.preset)?,
        shared::enum_name(&header.difficulty.information)?,
        shared::enum_name(&header.difficulty.assistance)?,
        shared::enum_name(&header.difficulty.weather)?,
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
        shared::enum_name(&finalization.reason)?,
        shared::enum_name(&finalization.disposition)?,
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
    shared::write_row(&mut output, columns.iter().copied());
    for (index, sample) in document.samples.iter().enumerate() {
        let mut row = metadata.clone();
        row.extend([
            sample.tick_index.to_string(),
            sample.fraction.to_string(),
            time_s(sample.tick_index, sample.fraction, header.physics_hz).to_string(),
        ]);
        let state = &sample.state;
        shared::numbers(&mut row, state.datum_position_ned_m);
        shared::numbers(&mut row, state.datum_velocity_ned_mps);
        shared::numbers(&mut row, state.attitude_body_to_ned);
        shared::numbers(&mut row, state.angular_velocity_body_rad_s);
        shared::numbers(&mut row, [state.pilot_position_m, state.pilot_velocity_mps]);
        let TailFlightRecordControlsDocument::TailIncidence {
            physical_incidence,
            input_from_previous,
        } = &sample.controls;
        shared::numbers(
            &mut row,
            [
                physical_incidence.horizontal_tail_rad,
                physical_incidence.vertical_tail_rad,
            ],
        );
        shared::numbers(&mut row, state.wind_at_cg_ned_mps);
        shared::numbers(&mut row, state.telemetry.composite_cg_position_ned_m);
        shared::numbers(
            &mut row,
            [
                state.telemetry.altitude_m,
                state.telemetry.airspeed_mps,
                state.telemetry.groundspeed_mps,
            ],
        );
        row.extend([
            shared::optional_number(state.telemetry.angle_of_attack_rad),
            shared::optional_number(state.telemetry.sideslip_angle_rad),
        ]);
        shared::numbers(&mut row, state.telemetry.attitude_euler_rad);
        row.push(input_from_previous.is_some().to_string());
        if let Some(input) = input_from_previous {
            shared::numbers(
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
                shared::numbers(
                    &mut row,
                    [incidence.horizontal_tail_rad, incidence.vertical_tail_rad],
                );
            }
        } else {
            row.extend(core::iter::repeat_n(String::new(), INPUT_COLUMNS.len() - 1));
        }
        row.extend(terminal.iter().cloned());
        shared::append_estimate(
            &mut row,
            &document.samples,
            index,
            header.physics_hz,
            motion,
        );
        if row.len() != columns.len() {
            return Err(FlightRecordFormatError::EncodingFailed);
        }
        shared::write_row(&mut output, row.iter().map(String::as_str));
    }
    Ok(output.into_bytes())
}

fn json(value: &impl serde::Serialize) -> Result<String, FlightRecordFormatError> {
    serde_json::to_string(value).map_err(|_| FlightRecordFormatError::EncodingFailed)
}

fn time_s(tick_index: u64, fraction: f64, physics_hz: u32) -> f64 {
    (tick_index as f64 + fraction) / f64::from(physics_hz)
}

fn motion(sample: &TailFlightRecordSampleDocument) -> shared::MotionSample {
    let state = &sample.state;
    let [north, east, down] = state.datum_velocity_ned_mps;
    let [roll, pitch, yaw] = state.angular_velocity_body_rad_s;
    shared::MotionSample {
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
