use birdman_game_core::{FlightRecordPlaybackSample, SessionPhase, SessionSnapshot};
use birdman_game_format::InformationLevel;
use unicode_width::UnicodeWidthStr;

use super::types::*;

#[derive(Clone, Copy)]
struct GameUiHudAttitude {
    roll_degrees: f64,
    pitch_degrees: f64,
}

struct GameUiHudGeometry {
    adi: Option<GameUiHudAdiGeometry>,
    heading_ticks: Vec<GameUiHudHeadingTick>,
    angle_of_attack_ratio: Option<f64>,
}

pub(super) fn game_ui_hud(
    state: &GameUiState,
    context: &GameUiContext<'_, '_>,
) -> Result<Option<GameUiHud>, GameUiError> {
    let snapshot = context.session.snapshot();
    let phase = snapshot.phase();
    if state.credits_open && matches!(phase, SessionPhase::FlightPaused { .. }) {
        return Ok(None);
    }
    let (sample, distance, target) = match snapshot {
        SessionSnapshot::TailFlightRunning { state, .. }
        | SessionSnapshot::TailFlightPaused { state, .. }
            if context.platform.presentation_mode == GameUiPresentationMode::Screen
                || phase == SessionPhase::FlightRunning =>
        {
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
                    || "—".to_owned(),
                    |score| format!("{:.1} m", score.course_parallel_m()),
                );
            (
                FlightRecordPlaybackSample {
                    tick_index: state.tick_index(),
                    fraction: 0.0,
                    flight_state: state.flight_state(),
                    actuators: state.incidence(),
                    telemetry,
                },
                distance,
                Some(state.pilot_position_target().position_m()),
            )
        }
        SessionSnapshot::Replay { .. }
            if context.platform.presentation_mode == GameUiPresentationMode::Screen
                && state.replay_view == GameUiReplayView::Telemetry =>
        {
            let record = context.record.ok_or(GameUiError::ActionUnavailable)?;
            let clock = context
                .session
                .playback_clock()
                .ok_or(GameUiError::ActionUnavailable)?;
            let sample = record
                .sample_at_seconds(clock.time_seconds())
                .map_err(GameUiError::Record)?;
            (sample, "—".to_owned(), None)
        }
        _ => return Ok(None),
    };
    let telemetry = sample.telemetry;
    let information = context.difficulty.information();
    let profile = context.difficulty.hud_profile();
    let full = information == InformationLevel::Full;
    let custom = information == InformationLevel::Custom;
    let minimal = information == InformationLevel::Minimal;
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
    let attitude_text = format!("ピッチ {pitch:.0}°\nロール {roll:.0}°");
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
    let attitude = attitude_visible.then_some(GameUiHudAttitude {
        roll_degrees: roll,
        pitch_degrees: pitch,
    });
    let heading_degrees = attitude_visible.then_some(heading);
    let angle_of_attack_degrees = alpha_visible.then_some(alpha).flatten();
    let menu_layout = super::view::layout(state, context, phase)?;
    let mut layout = hud_layout(context.platform, phase, &menu_layout);
    let geometry = hud_geometry(
        attitude,
        path_angle,
        heading_degrees,
        angle_of_attack_degrees,
    );
    let failure = matches!(
        phase,
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
    )
    .then_some(context.platform.flight_controller_failure.as_deref())
    .flatten();
    let mut primary = Vec::new();
    if let Some(cause) = failure {
        primary.push(hud_node(
            "hud-controller-failure",
            "",
            GameUiHudNodeContent::Text {
                value: format!("停止理由: {cause}"),
                tone: GameUiHudTone::Warning,
            },
        ));
    }
    if failure.is_some() {
        primary.push(text_node(
            "hud-last-valid-state",
            "最後の有効状態",
            format!("{time:.2} s"),
        ));
    }
    if telemetry_visible || failure.is_some() {
        primary.push(text_node(
            "hud-primary-readouts",
            "対気",
            format!("{:.1} m/s", telemetry.airspeed_mps),
        ));
    }
    if telemetry_visible || minimal || failure.is_some() {
        primary.push(text_node(
            if minimal && failure.is_none() {
                "hud-primary-readouts"
            } else {
                "hud-altitude"
            },
            "高度",
            format!("{:.1} m", telemetry.altitude_m),
        ));
        if telemetry_visible && failure.is_none() {
            primary.push(text_node(
                "hud-groundspeed",
                "対地",
                format!("{:.1} m/s", telemetry.groundspeed_mps),
            ));
        }
        primary.push(text_node("hud-distance", "距離", distance));
        primary.push(text_node("hud-elapsed", "時間", format!("{time:.1} s")));
    }
    if full || custom {
        if attitude_visible {
            let rates = sample.flight_state.angular_velocity_body().components();
            for ((id, label), rate) in [
                ("hud-roll-rate", "ロール"),
                ("hud-pitch-rate", "ピッチ"),
                ("hud-yaw-rate", "ヨー"),
            ]
            .into_iter()
            .zip(rates)
            {
                primary.push(text_node(
                    id,
                    label,
                    format!("{:.1} °/s", rate.to_degrees()),
                ));
            }
        }
        if telemetry_visible {
            primary.push(text_node(
                "hud-elevator-incidence",
                "水平尾翼",
                format!("{:.1}°", sample.actuators.elevator_rad().to_degrees()),
            ));
            primary.push(text_node(
                "hud-rudder-incidence",
                "垂直尾翼",
                format!("{:.1}°", sample.actuators.rudder_rad().to_degrees()),
            ));
            primary.push(text_node(
                "hud-pilot-position",
                "パイロット位置（目標）",
                format!(
                    "{:+.2} m（{}）",
                    sample.flight_state.pilot_position_m(),
                    target.map_or_else(|| "—".to_owned(), |target| format!("{target:+.2} m")),
                ),
            ));
        }
    }
    let mut instruments = Vec::new();
    if let Some(adi) = geometry.adi {
        instruments.push(hud_node(
            "hud-attitude",
            if adi.horizon().is_some() {
                "姿勢"
            } else {
                "飛行経路"
            },
            GameUiHudNodeContent::Attitude {
                value: if adi.horizon().is_some() {
                    attitude_text
                } else {
                    path_angle.map_or_else(String::new, |angle| format!("{angle:.1}°"))
                },
                geometry: adi,
            },
        ));
    }
    if attitude_visible {
        instruments.push(hud_node(
            "hud-heading",
            "方位",
            GameUiHudNodeContent::HeadingTape {
                value: format!("{heading:.0}°"),
                ticks: geometry.heading_ticks,
            },
        ));
    }
    if wind_visible {
        instruments.push(hud_node(
            "hud-wind",
            "風",
            GameUiHudNodeContent::Direction {
                value: format!(
                    "水平 {:.1}\n{} {:.1} m/s",
                    wind_north.hypot(wind_east),
                    if wind_down < 0.0 { "上昇" } else { "下降" },
                    wind_down.abs()
                ),
                rotation_degrees: (wind_north != 0.0 || wind_east != 0.0)
                    .then(|| wind_east.atan2(wind_north).to_degrees().rem_euclid(360.0)),
            },
        ));
    }
    if alpha_visible {
        instruments.push(hud_node(
            "hud-angle-of-attack",
            "迎角",
            GameUiHudNodeContent::LinearGauge {
                value: alpha.map_or_else(|| "—".to_owned(), |alpha| format!("{alpha:.1}°")),
                minimum_label: "−10°".to_owned(),
                maximum_label: "+20°".to_owned(),
                position_ratio: geometry.angle_of_attack_ratio,
            },
        ));
    }
    let groups = position_hud(&mut layout, primary, instruments, true);
    Ok(Some(GameUiHud { layout, groups }))
}

fn hud_node(id: &str, label: &str, content: GameUiHudNodeContent) -> GameUiHudNode {
    GameUiHudNode {
        id: id.to_owned(),
        label: label.to_owned(),
        bounds: GameUiRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        instrument_height_em: 0.0,
        content,
    }
}

fn text_node(id: &str, label: &str, value: String) -> GameUiHudNode {
    hud_node(
        id,
        label,
        GameUiHudNodeContent::Text {
            value,
            tone: GameUiHudTone::Normal,
        },
    )
}

fn hud_geometry(
    attitude: Option<GameUiHudAttitude>,
    flight_path: Option<f64>,
    heading: Option<f64>,
    angle_of_attack: Option<f64>,
) -> GameUiHudGeometry {
    let horizon = attitude.map(|attitude| GameUiHudAttitudeGeometry {
        rotation_degrees: -attitude.roll_degrees,
        pitch_offset_ratio: (attitude.pitch_degrees * 2.2).clamp(-55.0, 55.0) / 180.0,
        pitch_ticks: [-30_i32, -20, -10, 0, 10, 20, 30].map(|pitch| GameUiHudPitchTick {
            offset_ratio: -f64::from(pitch) * 2.2 / 180.0,
            half_width_ratio: if pitch == 0 {
                94.0
            } else if pitch.abs() % 20 == 0 {
                27.0
            } else {
                17.0
            } / 240.0,
            label_degrees: (pitch.abs() == 20).then_some(pitch.unsigned_abs()),
        }),
    });
    let flight_path_y_ratio = flight_path.map(|angle| 0.5 - angle.clamp(-30.0, 30.0) * 2.2 / 180.0);
    GameUiHudGeometry {
        adi: match (horizon, flight_path_y_ratio) {
            (Some(horizon), flight_path_y_ratio) => Some(GameUiHudAdiGeometry::Attitude {
                horizon,
                flight_path_y_ratio,
            }),
            (None, Some(y_ratio)) => Some(GameUiHudAdiGeometry::FlightPathOnly { y_ratio }),
            (None, None) => None,
        },
        heading_ticks: heading.map_or_else(Vec::new, |heading| {
            (-6..=6)
                .map(|index| {
                    let offset = index * 10;
                    let major = offset % 30 == 0;
                    let bearing = rounded_bearing(heading + f64::from(offset));
                    GameUiHudHeadingTick {
                        position_ratio: (110.0 + f64::from(offset) * 1.2) / 220.0,
                        major,
                        label: if offset == 0 {
                            match bearing {
                                0 => "N".to_owned(),
                                90 => "E".to_owned(),
                                180 => "S".to_owned(),
                                270 => "W".to_owned(),
                                _ => format!("{bearing:03}"),
                            }
                        } else {
                            String::new()
                        },
                    }
                })
                .collect()
        }),
        angle_of_attack_ratio: angle_of_attack
            .map(|angle| (angle.clamp(-10.0, 20.0) + 10.0) / 30.0),
    }
}

fn rounded_bearing(value: f64) -> u32 {
    let floor = value.floor();
    let rounded = if value - floor < 0.5 {
        floor
    } else {
        floor + 1.0
    };
    rounded.rem_euclid(360.0) as u32
}

pub(super) fn minimum_hud_width(font: f64) -> f64 {
    19.0 * font
}

fn hud_layout(
    platform: &GameUiPlatformInput,
    phase: SessionPhase,
    menu: &GameUiLayout,
) -> GameUiHudLayout {
    let font = platform.viewport.root_font_size;
    let viewport = platform.viewport;
    let compact = viewport.width <= 600.0 || viewport.width < minimum_hud_width(font) + 17.5 * font;
    let in_menu = matches!(
        phase,
        SessionPhase::FlightPaused { .. } | SessionPhase::Replay
    );
    let (left, bottom, width) = if in_menu && menu.shell == GameUiShell::FlightBar {
        (
            if compact { 0.6 } else { 0.75 } * font,
            menu.maximum_height_px + 0.75 * font,
            (viewport.width - if compact { 1.2 } else { 1.5 } * font)
                .max(1.0)
                .min(54.0 * font),
        )
    } else if in_menu
        && matches!(
            menu.shell,
            GameUiShell::PauseSettings | GameUiShell::Cinematic
        )
    {
        (
            0.75 * font,
            0.75 * font,
            (viewport.width - menu.width_px - 2.25 * font)
                .max(1.0)
                .min(54.0 * font),
        )
    } else if compact {
        (
            0.6 * font,
            4.25 * font,
            (viewport.width - 1.2 * font).max(1.0),
        )
    } else {
        (
            0.75 * font,
            0.75 * font,
            (viewport.width - 17.5 * font).max(1.0).min(54.0 * font),
        )
    };
    GameUiHudLayout {
        offset_px: [left, bottom],
        width_px: width,
        height_px: 0.0,
        padding_px: if compact {
            [0.45 * font, 0.3 * font]
        } else {
            [0.6 * font, 0.4 * font]
        },
        gap_px: [0.5 * font, 0.25 * font],
        attitude_size_px: if compact {
            [4.8 * font, 3.2 * font]
        } else {
            [5.6 * font, 3.6 * font]
        },
        font_size_px: (0.875 * font).max(14.0),
        opacity: if platform.presentation_mode == GameUiPresentationMode::Screen {
            0.48
        } else {
            0.28
        },
    }
}

pub(super) fn position_hud(
    layout: &mut GameUiHudLayout,
    readouts: Vec<GameUiHudNode>,
    instruments: Vec<GameUiHudNode>,
    allow_side_by_side: bool,
) -> Vec<GameUiHudGroup> {
    let [padding_x, padding_y] = layout.padding_px;
    let content_width = (layout.width_px - 2.0 * padding_x).max(1.0);
    let side_by_side = allow_side_by_side
        && content_width >= 48.0 * layout.font_size_px
        && !readouts.is_empty()
        && !instruments.is_empty();
    let readout_width = if side_by_side {
        content_width * 0.46
    } else {
        content_width
    };
    let mut groups = hud_rows(
        "hud-readouts",
        GameUiHudPlacement::Readouts,
        readouts,
        readout_width,
        layout,
    );
    let readout_height = position_rows(&mut groups, padding_x, padding_y, readout_width, layout);
    let instrument_width = if side_by_side {
        content_width - readout_width - layout.gap_px[0]
    } else {
        content_width
    };
    let mut instrument_rows = hud_rows(
        "hud-instruments",
        GameUiHudPlacement::Instruments,
        instruments,
        instrument_width,
        layout,
    );
    let instrument_height = position_rows(
        &mut instrument_rows,
        if side_by_side {
            padding_x + readout_width + layout.gap_px[0]
        } else {
            padding_x
        },
        if side_by_side || groups.is_empty() {
            padding_y
        } else {
            padding_y + readout_height + layout.gap_px[1]
        },
        instrument_width,
        layout,
    );
    layout.height_px = 2.0 * padding_y
        + if side_by_side {
            readout_height.max(instrument_height)
        } else {
            readout_height
                + instrument_height
                + if groups.is_empty() || instrument_rows.is_empty() {
                    0.0
                } else {
                    layout.gap_px[1]
                }
        };
    groups.extend(instrument_rows);
    for node in groups.iter_mut().flat_map(|group| &mut group.nodes) {
        node.bounds.x /= layout.width_px;
        node.bounds.y /= layout.height_px;
        node.bounds.width /= layout.width_px;
        node.bounds.height /= layout.height_px;
    }
    groups
}

fn hud_rows(
    id: &str,
    placement: GameUiHudPlacement,
    nodes: Vec<GameUiHudNode>,
    width: f64,
    layout: &GameUiHudLayout,
) -> Vec<GameUiHudGroup> {
    let mut rows: Vec<GameUiHudGroup> = Vec::new();
    let mut used_width = 0.0;
    for node in nodes {
        let minimum_width = hud_node_width(&node, layout).min(width);
        let gap = if used_width > 0.0 {
            layout.gap_px[0]
        } else {
            0.0
        };
        if rows.last().is_none_or(|row| row.nodes.len() == 5)
            || used_width + gap + minimum_width > width + 1.0e-9
        {
            let row_index = rows.len();
            rows.push(GameUiHudGroup {
                id: if row_index == 0 {
                    id.to_owned()
                } else {
                    format!("{id}-{row_index}")
                },
                placement,
                nodes: Vec::new(),
            });
            used_width = 0.0;
        }
        used_width += if used_width > 0.0 {
            layout.gap_px[0]
        } else {
            0.0
        } + minimum_width;
        rows.last_mut().unwrap().nodes.push(node);
    }
    rows
}

fn position_rows(
    groups: &mut [GameUiHudGroup],
    left: f64,
    top: f64,
    width: f64,
    layout: &GameUiHudLayout,
) -> f64 {
    let mut y = top;
    for group in groups.iter_mut() {
        let widths: Vec<_> = group
            .nodes
            .iter()
            .map(|node| hud_node_width(node, layout).min(width))
            .collect();
        let free_width =
            (width - widths.iter().sum::<f64>() - (widths.len() - 1) as f64 * layout.gap_px[0])
                .max(0.0);
        let mut x = left;
        let mut row_height = 0.0_f64;
        let node_count = group.nodes.len() as f64;
        for (node, minimum_width) in group.nodes.iter_mut().zip(widths) {
            let node_width = minimum_width + free_width / node_count;
            let node_height = hud_node_height(node, node_width, layout);
            node.bounds = GameUiRect {
                x,
                y,
                width: node_width,
                height: node_height,
            };
            row_height = row_height.max(node_height);
            x += node_width + layout.gap_px[0];
        }
        for node in &mut group.nodes {
            node.bounds.height = row_height;
        }
        y += row_height + layout.gap_px[1];
    }
    if groups.is_empty() {
        0.0
    } else {
        y - top - layout.gap_px[1]
    }
}

fn hud_node_width(node: &GameUiHudNode, layout: &GameUiHudLayout) -> f64 {
    let font = layout.font_size_px;
    match &node.content {
        GameUiHudNodeContent::Text { value, tone } => {
            if *tone == GameUiHudTone::Warning {
                return layout.width_px;
            }
            let columns = UnicodeWidthStr::width(node.label.as_str())
                + 1
                + value.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
            (columns as f64 * font * 0.55).max(4.0 * font)
        }
        _ => 4.0 * font + 2.0 * layout.padding_px[0],
    }
}

fn hud_node_height(node: &mut GameUiHudNode, width: f64, layout: &GameUiHudLayout) -> f64 {
    let font = layout.font_size_px;
    let is_text = matches!(node.content, GameUiHudNodeContent::Text { .. });
    let content_width = (width
        - if is_text {
            0.0
        } else {
            2.0 * layout.padding_px[0]
        })
    .max(font);
    let lines = |text: &str| -> f64 {
        text.lines()
            .map(|line| {
                (UnicodeWidthStr::width(line) as f64 * font * 0.55 / content_width)
                    .ceil()
                    .max(1.0)
            })
            .sum::<f64>()
            .max(1.0)
    };
    let text_height = if is_text {
        lines(&format!("{} {}", node.label, node.content.value())) * font * 1.5
    } else {
        (lines(&node.label) + lines(node.content.value())) * font * 1.5
    };
    node.instrument_height_em = match node.content {
        GameUiHudNodeContent::Text { .. } => 0.0,
        GameUiHudNodeContent::Attitude { .. } => {
            content_width.min(layout.attitude_size_px[0]) * layout.attitude_size_px[1]
                / layout.attitude_size_px[0]
                / font
        }
        GameUiHudNodeContent::HeadingTape { .. } | GameUiHudNodeContent::LinearGauge { .. } => 2.5,
        GameUiHudNodeContent::Direction { .. } => 3.0,
    };
    text_height
        + node.instrument_height_em * font
        + if is_text {
            0.0
        } else {
            2.0 * layout.padding_px[1]
        }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn instrument_projection_preserves_web_baseline_and_clamping() {
        let geometry = hud_geometry(
            Some(GameUiHudAttitude {
                roll_degrees: 11.0,
                pitch_degrees: 2.0,
            }),
            Some(-3.0),
            Some(0.0),
            Some(5.0),
        );
        let adi = geometry.adi.unwrap();
        assert_eq!(adi.horizon().unwrap().rotation_degrees, -11.0);
        assert_eq!(adi.horizon().unwrap().pitch_offset_ratio, 4.4 / 180.0);
        assert!((adi.flight_path_y_ratio().unwrap() - (90.0 + 6.6) / 180.0).abs() < 1.0e-12);
        assert_eq!(geometry.angle_of_attack_ratio, Some(0.5));
        assert_eq!(geometry.heading_ticks.len(), 13);
        assert_eq!(geometry.heading_ticks[0].label, "300");
        assert_eq!(geometry.heading_ticks[6].label, "N");
        assert_eq!(geometry.heading_ticks[12].label, "060");
        for (angle, path, ratio) in [
            (-100.0, (90.0 + 66.0) / 180.0, 0.0),
            (100.0, (90.0 - 66.0) / 180.0, 1.0),
        ] {
            let projected = hud_geometry(
                Some(GameUiHudAttitude {
                    roll_degrees: 0.0,
                    pitch_degrees: angle,
                }),
                Some(angle),
                None,
                Some(angle),
            );
            assert_eq!(
                projected.adi.unwrap().horizon().unwrap().pitch_offset_ratio,
                angle.signum() * 55.0 / 180.0
            );
            assert!((projected.adi.unwrap().flight_path_y_ratio().unwrap() - path).abs() < 1.0e-12);
            assert_eq!(projected.angle_of_attack_ratio, Some(ratio));
        }
    }

    #[test]
    fn heading_labels_preserve_wrapping_cardinals_and_web_rounding() {
        for (heading, label) in [
            (0.0, "N"),
            (90.0, "E"),
            (180.0, "S"),
            (270.0, "W"),
            (359.5, "N"),
            (17.0, "017"),
            (0.49999999999999994, "N"),
            (0.5, "001"),
            (0.5000000000000001, "001"),
            (-0.5000000000000001, "359"),
            (-0.5, "N"),
            (-0.49999999999999994, "N"),
            (-3.5, "357"),
        ] {
            assert_eq!(
                hud_geometry(None, None, Some(heading), None).heading_ticks[6].label,
                label
            );
        }
        assert_eq!(
            hud_geometry(None, None, Some(0.5), None).heading_ticks[0].label,
            "301"
        );
        let hidden = hud_geometry(None, None, None, None);
        assert!(
            hidden.adi.is_none()
                && hidden.heading_ticks.is_empty()
                && hidden.angle_of_attack_ratio.is_none()
        );
    }

    #[test]
    fn flight_path_only_retains_the_adi_region_without_an_attitude_layer() {
        let geometry = hud_geometry(None, Some(-3.0), None, None);
        let Some(GameUiHudAdiGeometry::FlightPathOnly { y_ratio }) = geometry.adi else {
            panic!("flight path cue requires its visible ADI region");
        };
        assert!((y_ratio - (90.0 + 6.6) / 180.0).abs() < 1.0e-12);
        assert_eq!(geometry.adi.unwrap().horizon(), None);
        assert_eq!(geometry.adi.unwrap().flight_path_y_ratio(), Some(y_ratio));
    }
}
