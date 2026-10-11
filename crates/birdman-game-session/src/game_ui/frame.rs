use birdman_game_core::{FlightRecordPlaybackSample, SessionPhase};

use super::{types::*, view};

/// Observes only frame-varying values using a retained structural view.
pub fn game_ui_frame(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
    retained: &GameUiView,
) -> Result<GameUiFrame, GameUiError> {
    let phase = context.session.snapshot().phase();
    let mut structural_state = state.clone();
    structural_state.analysis_time_seconds = retained.selections.analysis_time_seconds;
    let overlay = match phase {
        SessionPhase::FlightPaused { .. } => Some(match state.pause_overlay {
            GameUiPauseOverlay::Menu => GameUiOverlay::Pause,
            GameUiPauseOverlay::Settings => GameUiOverlay::PauseSettings,
            GameUiPauseOverlay::Help => GameUiOverlay::PauseHelp,
        }),
        _ => None,
    };
    if structural_state != retained.selections
        || view::scene(phase) != retained.scene
        || overlay != retained.active_overlay
    {
        return Err(GameUiError::StaleView);
    }
    let mut frame = GameUiFrame {
        view_revision: retained.view_revision,
        selections: state.clone(),
        hud: super::hud::game_ui_hud(state, context)?,
        ranges: Vec::new(),
        readouts: Vec::new(),
        button_labels: Vec::new(),
        chart_cursor: None,
    };
    if let SessionPhase::Countdown { remaining_ticks } = phase {
        frame.readouts.push(readout(
            "game-countdown-status",
            format!("{remaining_ticks} 秒"),
        ));
    }
    let Some(record) = context.record else {
        return Ok(frame);
    };
    let duration = record.duration_seconds().map_err(GameUiError::Record)?;
    let time = match phase {
        SessionPhase::Replay | SessionPhase::Attract => context
            .session
            .playback_clock()
            .ok_or(GameUiError::ActionUnavailable)?
            .time_seconds(),
        SessionPhase::Result => state.analysis_time_seconds.min(duration),
        _ => return Ok(frame),
    };
    if !time.is_finite() || time < 0.0 || time > duration {
        return Err(GameUiError::InvalidTime);
    }
    let controls: Vec<_> = retained
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .collect();
    for control in &controls {
        if let GameUiControlContent::Range { .. } = control.content {
            frame.ranges.push(GameUiRangeValue {
                id: control.id.clone(),
                value: time,
            });
        }
    }
    if phase == SessionPhase::Replay {
        let clock = context
            .session
            .playback_clock()
            .ok_or(GameUiError::ActionUnavailable)?;
        if controls
            .iter()
            .any(|control| control.id == "game-replay-time")
        {
            frame.readouts.push(readout(
                "game-replay-time",
                format!("{time:.2} / {duration:.2} s"),
            ));
        }
        frame.button_labels.push(GameUiButtonLabel {
            id: "game-replay-play-pause".to_owned(),
            label: if clock.is_playing() { "Pause" } else { "Play" }.to_owned(),
        });
    } else if phase == SessionPhase::Attract {
        frame.readouts.push(readout(
            "game-attract-time",
            format!("{time:.2} / {duration:.2} s"),
        ));
        let distance = record
            .finalization()
            .and_then(|terminal| terminal.score)
            .map_or_else(
                || "距離未記録".to_owned(),
                |score| format!("{:.1} m", score.course_parallel_m()),
            );
        frame.readouts.push(readout(
            "game-attract-distance",
            format!("{distance} · t {time:.2} s"),
        ));
    }
    if let Some(control) = controls
        .iter()
        .find(|control| control.id == "game-analysis-plot")
    {
        let GameUiControlContent::Chart { chart, plot } = &control.content else {
            return Err(GameUiError::StaleView);
        };
        let sample = record
            .sample_at_seconds(time)
            .map_err(GameUiError::Record)?;
        frame.readouts.push(readout(
            "game-analysis-cursor-values",
            analysis_cursor_readout(sample, time),
        ));
        frame.chart_cursor = Some(super::chart::chart_cursor(*chart, plot, sample, time));
    }
    Ok(frame)
}

fn readout(id: &str, value: String) -> GameUiReadoutValue {
    GameUiReadoutValue {
        id: id.to_owned(),
        value,
    }
}

pub(super) fn analysis_cursor_readout(sample: FlightRecordPlaybackSample, time: f64) -> String {
    let [north, east, _] = sample.telemetry.composite_cg_position_ned_m.components();
    let [wind_north, wind_east, wind_down] = sample.telemetry.wind_velocity_ned_mps.components();
    format!(
        "t {time:.2} s · N {north:.1} m · E {east:.1} m · h {:.1} m\nVair {:.1} · Vground {:.1} · WN {wind_north:.1} · WE {wind_east:.1} · WD {wind_down:.1} m/s",
        sample.telemetry.altitude_m,
        sample.telemetry.airspeed_mps,
        sample.telemetry.groundspeed_mps
    )
}
