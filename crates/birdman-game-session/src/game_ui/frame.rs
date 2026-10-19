use birdman_game_core::{FlightRecordPlaybackSample, SessionPhase, SessionScenarioIdentity};

use super::{types::*, view};

/// Observes only frame-varying values using a retained structural view.
pub fn game_ui_frame(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
    retained: &GameUiView,
    current_revision: u32,
) -> Result<GameUiFrame, GameUiError> {
    let phase = context.session.snapshot().phase();
    if retained.view_revision != current_revision {
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
    let controls: Vec<_> = retained
        .panels
        .iter()
        .flat_map(|panel| &panel.controls)
        .collect();
    if let SessionPhase::Countdown { remaining_seconds } = phase {
        for control in &controls {
            if matches!(
                control.content,
                GameUiControlContent::Status {
                    source: GameUiReadoutSource::Countdown,
                    ..
                }
            ) {
                frame
                    .readouts
                    .push(readout(&control.id, format!("{remaining_seconds} 秒")));
            }
        }
    }
    if !matches!(
        phase,
        SessionPhase::Result | SessionPhase::Replay | SessionPhase::Attract
    ) {
        return Ok(frame);
    }
    let Some(record) = context.record else {
        return Ok(frame);
    };
    let duration = record.duration_seconds().map_err(GameUiError::Record)?;
    let clock = context.session.playback_clock();
    let time = match phase {
        SessionPhase::Replay | SessionPhase::Attract => {
            clock.ok_or(GameUiError::ActionUnavailable)?.time_seconds()
        }
        SessionPhase::Result => state.analysis_time_seconds.min(duration),
        _ => return Ok(frame),
    };
    if !time.is_finite() || time < 0.0 || time > duration {
        return Err(GameUiError::InvalidTime);
    }
    let chart = controls.iter().find_map(|control| match &control.content {
        GameUiControlContent::Chart { chart, plot } => Some((&control.id, *chart, plot)),
        _ => None,
    });
    let sample = chart
        .map(|_| record.sample_at_seconds(time).map_err(GameUiError::Record))
        .transpose()?;
    if let (Some((id, chart, plot)), Some(sample)) = (chart, sample) {
        frame.chart_cursor = Some(super::chart::chart_cursor(id, chart, plot, sample, time));
    }
    for control in controls {
        match &control.content {
            GameUiControlContent::Range { .. } => frame.ranges.push(GameUiRangeValue {
                id: control.id.clone(),
                value: time,
                display_value: range_display_value(time),
            }),
            GameUiControlContent::Status { source, .. } => {
                let value = match source {
                    GameUiReadoutSource::Static | GameUiReadoutSource::Countdown => continue,
                    GameUiReadoutSource::PlaybackTime => format!("{time:.2} / {duration:.2} s"),
                    GameUiReadoutSource::PlaybackSummary => {
                        let distance = record
                            .finalization()
                            .and_then(|terminal| terminal.score)
                            .map_or_else(
                                || "距離未記録".to_owned(),
                                |score| format!("{:.1} m", score.course_parallel_m()),
                            );
                        format!("{distance} · t {time:.2} s")
                    }
                    GameUiReadoutSource::AnalysisCursor => analysis_cursor_readout(
                        sample.ok_or(GameUiError::StaleView)?,
                        time,
                        record.header().scenario,
                        frame
                            .chart_cursor
                            .as_ref()
                            .and_then(|cursor| cursor.selected_wind_vector.as_ref())
                            .map(|vector| vector.label.as_str()),
                    ),
                };
                frame.readouts.push(readout(&control.id, value));
            }
            GameUiControlContent::Button { action, .. } if phase == SessionPhase::Replay => {
                let label = match action {
                    GameUiAction::ToggleReplayPlaying {} => {
                        if clock.ok_or(GameUiError::ActionUnavailable)?.is_playing() {
                            "Pause"
                        } else {
                            "Play"
                        }
                        .to_owned()
                    }
                    GameUiAction::SetReplayRate { code } => {
                        let label = match code {
                            0 => "0.5×",
                            1 => "1×",
                            2 => "2×",
                            _ => return Err(GameUiError::StaleView),
                        };
                        view::replay_rate_label(
                            *code,
                            clock.ok_or(GameUiError::ActionUnavailable)?.rate().code(),
                            label,
                        )
                    }
                    _ => continue,
                };
                frame.button_labels.push(GameUiButtonLabel {
                    id: control.id.clone(),
                    label,
                });
            }
            GameUiControlContent::Button { .. }
            | GameUiControlContent::Toggle { .. }
            | GameUiControlContent::Chart { .. } => {}
        }
    }
    Ok(frame)
}

fn readout(id: &str, value: String) -> GameUiReadoutValue {
    GameUiReadoutValue {
        id: id.to_owned(),
        value,
    }
}

pub(super) fn range_display_value(value: f64) -> String {
    ryu_js::Buffer::new().format_to_fixed(value, 1).to_owned()
}

pub(super) fn analysis_cursor_readout(
    sample: FlightRecordPlaybackSample,
    time: f64,
    identity: SessionScenarioIdentity,
    selected_wind_label: Option<&str>,
) -> String {
    let [north, east, _] = sample.telemetry.composite_cg_position_ned_m.components();
    let [wind_north, wind_east, wind_down] = sample.telemetry.wind_velocity_ned_mps.components();
    let mut readout = format!(
        "t {time:.2} s · N {north:.1} m · E {east:.1} m · h {:.1} m\nVair {:.1} · Vground {:.1} · WN {wind_north:.1} · WE {wind_east:.1} · WD {wind_down:.1} m/s",
        sample.telemetry.altitude_m,
        sample.telemetry.airspeed_mps,
        sample.telemetry.groundspeed_mps
    );
    if !super::environment::registered(identity) {
        readout.push_str(" · 風場 unavailable (unregistered_environment_identity)");
    }
    if let Some(label) = selected_wind_label {
        readout.push('\n');
        readout.push_str(label);
    }
    readout
}
