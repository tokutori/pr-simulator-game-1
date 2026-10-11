use birdman_game_core::{
    FlightRecordPlaybackSample, SessionEndReason, SessionPhase, SessionSnapshot,
};
use birdman_game_format::InformationLevel;

use super::types::*;

pub(super) fn game_ui_hud(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
) -> Result<Option<GameUiHud>, GameUiError> {
    let snapshot = context.session.snapshot();
    let phase = snapshot.phase();
    let (sample, status, distance, target, warning) = match snapshot {
        SessionSnapshot::TailFlightRunning { state, .. }
        | SessionSnapshot::TailFlightPaused { state, .. } => {
            let telemetry = context
                .session
                .telemetry()
                .map_err(|error| {
                    GameUiError::Session(birdman_game_core::GameSessionError::Telemetry(error))
                })?
                .ok_or(GameUiError::ActionUnavailable)?;
            let distance = context
                .session
                .flight_progress()
                .map_err(|error| {
                    GameUiError::Session(birdman_game_core::GameSessionError::Score(error))
                })?
                .map_or_else(
                    || "距離 unavailable".to_owned(),
                    |score| format!("距離 {:.1} m", score.course_parallel_m()),
                );
            (
                FlightRecordPlaybackSample {
                    tick_index: state.tick_index(),
                    fraction: 0.0,
                    flight_state: state.flight_state(),
                    actuators: state.incidence(),
                    telemetry,
                },
                if matches!(phase, SessionPhase::FlightPaused { .. }) {
                    "一時停止"
                } else {
                    "滑空中"
                },
                distance,
                Some(state.pilot_position_target().position_m()),
                None,
            )
        }
        SessionSnapshot::Result(result) => {
            let record = context.record.ok_or(GameUiError::ActionUnavailable)?;
            let duration = record.duration_seconds().map_err(GameUiError::Record)?;
            let sample = record
                .sample_at_seconds(duration)
                .map_err(GameUiError::Record)?;
            let label = match result.reason {
                SessionEndReason::WaterContact => "着水",
                SessionEndReason::TimeLimit => "時間制限",
                SessionEndReason::ManualAbort => "手動終了",
                SessionEndReason::OutOfValidEnvelope => "空力モデルの適用範囲外",
                SessionEndReason::FatalSimulationError => "シミュレーションエラー",
            };
            let warning = match result.reason {
                SessionEndReason::OutOfValidEnvelope => Some("AERODYNAMIC ENVELOPE"),
                SessionEndReason::FatalSimulationError => Some("SIMULATION FAILURE"),
                _ => None,
            };
            let target = match result.state {
                birdman_game_core::SessionTerminalState::TailTick(state) => {
                    state.pilot_position_target().position_m()
                }
                birdman_game_core::SessionTerminalState::TailWaterContact(contact) => {
                    contact.pilot_position_target().position_m()
                }
            };
            (
                sample,
                label,
                result.score.map_or_else(
                    || "距離 unavailable".to_owned(),
                    |score| format!("確定距離 {:.1} m", score.course_parallel_m()),
                ),
                Some(target),
                warning,
            )
        }
        SessionSnapshot::Replay { .. } | SessionSnapshot::Attract { .. } => {
            let record = context.record.ok_or(GameUiError::ActionUnavailable)?;
            let clock = context
                .session
                .playback_clock()
                .ok_or(GameUiError::ActionUnavailable)?;
            let sample = record
                .sample_at_seconds(clock.time_seconds())
                .map_err(GameUiError::Record)?;
            if phase == SessionPhase::Replay && state.replay_view != GameUiReplayView::Telemetry {
                return Ok(None);
            }
            (sample, "記録再生", "保存標本".to_owned(), None, None)
        }
        _ => return Ok(None),
    };
    let telemetry = sample.telemetry;
    let information = context.difficulty.information();
    let profile = context.difficulty.hud_profile();
    let full = information == InformationLevel::Full;
    let custom = information == InformationLevel::Custom;
    let minimal = information == InformationLevel::Minimal;
    let realistic = information == InformationLevel::Realistic;
    let attitude_visible = if custom { profile.attitude() } else { !minimal };
    let telemetry_visible = if custom {
        profile.telemetry()
    } else {
        !minimal
    };
    let wind_visible = full || custom && profile.wind();
    let alpha_visible = full || custom && profile.angle_of_attack();
    let roll = telemetry.roll_rad.to_degrees();
    let pitch = telemetry.pitch_rad.to_degrees();
    let heading = telemetry.heading_rad.to_degrees().rem_euclid(360.0);
    let time =
        (sample.tick_index as f64 + sample.fraction) / f64::from(birdman_game_core::PHYSICS_HZ);
    let airspeed = format!("IAS {:.1} m/s", telemetry.airspeed_mps);
    let altitude = format!("ALT {:.1} m", telemetry.altitude_m);
    let attitude_text = format!("PITCH {pitch:.0}° · ROLL {roll:.0}°");
    let duration = format!("時間 {time:.1} s");
    let mut readouts = Vec::new();
    if minimal {
        readouts.push(format!("{altitude} · {distance} · {duration}"));
    } else {
        if telemetry_visible {
            readouts.push(format!("{airspeed} · {altitude}"));
        }
        if attitude_visible {
            readouts.push(attitude_text);
        }
    }
    let alpha = telemetry.angle_of_attack_rad.map(f64::to_degrees);
    let [wind_north, wind_east, wind_down] = telemetry.wind_velocity_ned_mps.components();
    let velocity = sample.flight_state.datum_velocity_ned().components();
    let horizontal_speed = velocity[0].hypot(velocity[1]);
    let path_angle = if full || custom && profile.flight_path() {
        if horizontal_speed == 0.0 {
            None
        } else {
            Some((-velocity[2]).atan2(horizontal_speed).to_degrees())
        }
    } else {
        None
    };
    let position = sample.flight_state.datum_position_ned().components();
    let origin = crate::launch_venue()
        .map_err(|_| GameUiError::InvalidVenueMap)?
        .origin_wgs84;
    let latitude = origin.latitude_degrees + position[0] / 111_132.0;
    let longitude = origin.longitude_degrees
        + position[1] / (111_320.0 * origin.latitude_degrees.to_radians().cos());
    let mut supplementary = Vec::new();
    if full || custom {
        if attitude_visible {
            let [roll_rate, pitch_rate, yaw_rate] =
                sample.flight_state.angular_velocity_body().components();
            supplementary.push(format!(
                "p {:.1}  q {:.1}  r {:.1} °/s",
                roll_rate.to_degrees(),
                pitch_rate.to_degrees(),
                yaw_rate.to_degrees()
            ));
        }
        if telemetry_visible {
            let incidence = sample.actuators;
            supplementary.push(format!(
                "水平尾翼 {:.1}°  垂直尾翼 {:.1}°",
                incidence.elevator_rad().to_degrees(),
                incidence.rudder_rad().to_degrees()
            ));
            supplementary.push(target.map_or_else(
                || "PILOT TARGET unavailable".to_owned(),
                |target| format!("PILOT TARGET {target:.2} m"),
            ));
        }
    }
    let mut hud = GameUiHud { layout: hud_layout(context.platform), heading_label: "HEADING".to_owned(), pilot_position_label: "PILOT CG".to_owned(), wind_label: "WIND N/E/D".to_owned(), angle_of_attack_label: "AoA".to_owned(), status: status.to_owned(), attitude: attitude_visible.then_some(GameUiHudAttitude { roll_degrees: roll, pitch_degrees: pitch }),
        flight_path_angle_degrees: path_angle, warning: if full || custom && profile.warnings() { warning.map(str::to_owned) } else { None },
        heading_degrees: attitude_visible.then_some(heading), wind_direction_degrees: if wind_visible && (wind_north != 0.0 || wind_east != 0.0) { Some(wind_east.atan2(wind_north).to_degrees().rem_euclid(360.0)) } else { None },
        angle_of_attack_degrees: alpha_visible.then_some(alpha).flatten(), readouts: readouts.join("\n"), heading: attitude_visible.then(|| format!("{heading:.0}°")),
        pilot_position: (full || custom && profile.telemetry()).then(|| format!("{:+.2} m", sample.flight_state.pilot_position_m())),
        wind: wind_visible.then(|| format!("N {wind_north:.1} · E {wind_east:.1} · D {wind_down:.1} m/s")), angle_of_attack: alpha_visible.then(|| alpha.map_or_else(|| "—".to_owned(), |alpha| format!("{alpha:.1}°"))),
        telemetry: if realistic { "代表計器表示 · 対象機の実機構成は未確認".to_owned() } else if telemetry_visible { format!("対地速度 {:.1} m/s · {distance} · {duration}", telemetry.groundspeed_mps) } else { String::new() },
        location: format!("緯度 {latitude:.6}° · 経度 {longitude:.6}°"), map_attribution: "湖岸 © OpenStreetMap contributors · ODbL 1.0".to_owned(),
        supplementary_readouts: supplementary, controls_description: "↑/↓ nose-up/down intent · ←/→ left/right intent · J/L pilot Set · キー解放 pilot Hold · Gamepad pilot Set".to_owned() };
    if matches!(
        phase,
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
    ) && let Some(cause) = &context.platform.flight_controller_failure
    {
        hud.status = "飛行処理停止".to_owned();
        hud.warning = Some(format!("停止理由: {cause}"));
        hud.telemetry = format!("Last valid · tick {} · {time:.2} s", sample.tick_index);
    }
    Ok(Some(hud))
}

fn hud_layout(platform: &GameUiPlatformInput) -> GameUiHudLayout {
    let font = platform.viewport.root_font_size;
    let compact = platform.viewport.width <= 600.0;
    GameUiHudLayout {
        offset_px: [0.75 * font; 2],
        width_px: ((if compact { 19.0 } else { 25.0 }) * font)
            .min((platform.viewport.width - if compact { 1.2 } else { 1.5 } * font).max(1.0)),
        maximum_height_px: (platform.viewport.height - 5.0 * font).max(1.0),
        padding_px: if compact {
            [0.5 * font, 0.4 * font]
        } else {
            [0.65 * font, 0.55 * font]
        },
        gap_px: [0.65 * font, 0.25 * font],
        attitude_size_px: if compact {
            [5.5 * font, 4.2 * font]
        } else {
            [7.5 * font, 5.65 * font]
        },
        font_size_px: if compact { 0.65 * font } else { 0.72 * font },
        opacity: if platform.presentation_mode == GameUiPresentationMode::Screen {
            0.64
        } else {
            0.28
        },
        controls_visible: !compact,
        instrument_columns: 2,
    }
}
