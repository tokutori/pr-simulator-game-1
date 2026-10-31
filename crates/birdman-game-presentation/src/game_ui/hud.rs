use birdman_game_core::{
    FlightRecordPlaybackSample, GameSession, SessionPhase, SessionSnapshot, TailFlightTickState,
};
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
    angle_of_attack_marker: Option<[GameUiChartPoint; 3]>,
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
    let (sample, distance, target, incidence_rates, pilot_protection) = match snapshot {
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
                    input: None,
                    wing_bending_factor: state.wing_bending_factor(),
                    tail_wrenches: context
                        .session
                        .flight_record()
                        .and_then(|record| record.samples().last())
                        .filter(|sample| {
                            sample.tick_index == state.tick_index() && sample.fraction == 0.0
                        })
                        .and_then(|sample| sample.tail_wrenches),
                    telemetry,
                },
                distance,
                Some(state.pilot_position_demand().position_m()),
                tail_incidence_rates(context.session, state),
                state.pilot_motion_limited().then_some(
                    if state.flight_state().pilot_velocity_mps() == 0.0 {
                        "保護停止"
                    } else {
                        "保護制動"
                    },
                ),
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
            let target = sample
                .input
                .map(|input| input.resolved_pilot_position_target_m());
            (sample, "—".to_owned(), target, None, None)
        }
        _ => return Ok(None),
    };
    let telemetry = sample.telemetry;
    let information = context.difficulty.information();
    let profile = context.difficulty.effective_hud_profile();
    let minimal = information == InformationLevel::Minimal;
    let attitude_visible = profile.attitude();
    let telemetry_visible = profile.telemetry();
    let roll = telemetry.roll_rad.to_degrees();
    let pitch = telemetry.pitch_rad.to_degrees();
    let heading = telemetry.heading_rad.to_degrees().rem_euclid(360.0);
    let time =
        (sample.tick_index as f64 + sample.fraction) / f64::from(birdman_game_core::PHYSICS_HZ);
    let attitude_text = format!("ピッチ{pitch:.0}°\nロール{roll:.0}°");
    let alpha = telemetry.angle_of_attack_rad.map(f64::to_degrees);
    let [wind_north, wind_east, wind_down] = telemetry.wind_velocity_ned_mps.components();
    let velocity = sample.flight_state.datum_velocity_ned().components();
    let horizontal_speed = velocity[0].hypot(velocity[1]);
    let path_angle = if profile.flight_path() {
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
    let angle_of_attack_degrees = profile.angle_of_attack().then_some(alpha).flatten();
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
    .then_some(context.platform.renderer_failure.as_deref())
    .flatten();
    let mut primary = Vec::new();
    if matches!(
        phase,
        SessionPhase::FlightRunning | SessionPhase::FlightPaused { .. }
    ) {
        let labels = [
            "↑", "↓", "←", "→", "J", "L", "C", "P", "Esc", "W", "S", "A", "D",
        ];
        let value = labels
            .into_iter()
            .zip(context.accepted_flight_keys)
            .map(|(label, pressed)| format!("{label}{}", if pressed { "●" } else { "○" }))
            .collect::<Vec<_>>()
            .join(" ");
        primary.push(text_node("hud-accepted-keys", "キー", value));
    }
    if let Some(cause) = failure {
        primary.push(hud_node(
            "hud-renderer-failure",
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
    if context.difficulty.hud_basic_readouts() || failure.is_some() {
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
    if context.difficulty.hud_auxiliary_readouts() {
        if attitude_visible {
            let [roll_rate, pitch_rate, yaw_rate] =
                sample.flight_state.angular_velocity_body().components();
            primary.push(text_node(
                "hud-angular-rates",
                "ロール/ピッチ/ヨー",
                format!(
                    "{:.1}/{:.1}/{:.1} °/s",
                    roll_rate.to_degrees(),
                    pitch_rate.to_degrees(),
                    yaw_rate.to_degrees()
                ),
            ));
        }
        if telemetry_visible {
            primary.push(text_node(
                "hud-tail-incidence",
                "水平/垂直尾翼",
                format!(
                    "{:.1}/{:.1}°",
                    sample.actuators.elevator_rad().to_degrees(),
                    sample.actuators.rudder_rad().to_degrees()
                ),
            ));
            let loads = sample.tail_wrenches.unwrap_or([None; 2]).map(|wrench| {
                wrench
                    .and_then(|wrench| wrench.force_body_newtons().norm().ok())
                    .map_or_else(|| "—".to_owned(), |magnitude| format!("{magnitude:.1}"))
            });
            primary.push(text_node(
                "hud-tail-force",
                "水平/垂直尾翼合力",
                format!("{} / {} N", loads[0], loads[1]),
            ));
            primary.push(text_node(
                "hud-tail-incidence-rate",
                "水平/垂直尾翼角速度",
                incidence_rates.map_or_else(
                    || "— rad/s".to_owned(),
                    |[elevator, rudder]| format!("{elevator:+.2}/{rudder:+.2} rad/s"),
                ),
            ));
            primary.push(text_node(
                "hud-pilot-position",
                if phase == SessionPhase::Replay {
                    "パイロット位置（適用目標）"
                } else {
                    "パイロット位置（目標）"
                },
                format!(
                    "{:+.2} ({}) m{}",
                    sample.flight_state.pilot_position_m(),
                    target.map_or_else(|| "—".to_owned(), |target| format!("{target:+.2}")),
                    pilot_protection.map_or_else(String::new, |status| format!(" · {status}")),
                ),
            ));
            primary.push(text_node(
                "hud-pilot-velocity",
                "パイロット移動速度",
                format!("{:+.2} m/s", sample.flight_state.pilot_velocity_mps()),
            ));
        }
    }
    let mut instruments = Vec::new();
    if let Some(adi) = geometry.adi {
        instruments.push(hud_node(
            "hud-attitude",
            if adi.horizon.is_some() {
                ""
            } else {
                "飛行経路"
            },
            GameUiHudNodeContent::Attitude {
                value: if adi.horizon.is_some() {
                    attitude_text
                } else {
                    path_angle.map_or_else(String::new, |angle| format!("{angle:.1}°"))
                },
                geometry: Box::new(adi),
            },
        ));
    }
    if attitude_visible {
        instruments.push(hud_node(
            "hud-heading",
            "方位",
            GameUiHudNodeContent::HeadingTape {
                value: format!("{heading:.0}°"),
                canvas_size: [220.0, 50.0],
                ticks: geometry.heading_ticks,
                pointer: [
                    GameUiChartPoint { x: 110.0, y: 28.0 },
                    GameUiChartPoint { x: 104.0, y: 38.0 },
                    GameUiChartPoint { x: 116.0, y: 38.0 },
                ],
            },
        ));
    }
    if profile.wind() {
        instruments.push(hud_node(
            "hud-wind",
            "風 m/s",
            GameUiHudNodeContent::Direction {
                value: format!(
                    "\n水平{:.1}\n{}{:.1}",
                    wind_north.hypot(wind_east),
                    if wind_down < 0.0 { "上昇" } else { "下降" },
                    wind_down.abs()
                ),
                canvas_size: [72.0, 72.0],
                circle_bounds: GameUiRect {
                    x: 9.0,
                    y: 9.0,
                    width: 54.0,
                    height: 54.0,
                },
                circle_radius: 27.0,
                circle_stroke_width: 1.5,
                cross_segments: [
                    [
                        GameUiChartPoint { x: 36.0, y: 9.0 },
                        GameUiChartPoint { x: 36.0, y: 63.0 },
                    ],
                    [
                        GameUiChartPoint { x: 9.0, y: 36.0 },
                        GameUiChartPoint { x: 63.0, y: 36.0 },
                    ],
                ],
                cross_stroke_width: 1.0,
                north_label: "N".to_owned(),
                north_anchor: GameUiChartPoint { x: 36.0, y: 24.0 },
                arrow: direction_arrow(wind_north, wind_east),
            },
        ));
    }
    if profile.angle_of_attack() {
        instruments.push(hud_node(
            "hud-angle-of-attack",
            "迎角",
            GameUiHudNodeContent::LinearGauge {
                value: alpha.map_or_else(|| "—".to_owned(), |alpha| format!("{alpha:.1}°")),
                minimum_label: "−10°".to_owned(),
                maximum_label: "+20°".to_owned(),
                canvas_size: [200.0, 44.0],
                track: [
                    GameUiChartPoint { x: 10.0, y: 16.0 },
                    GameUiChartPoint { x: 190.0, y: 16.0 },
                ],
                ticks: Box::new(core::array::from_fn(|index| {
                    let position = 10.0 + index as f64 * 30.0;
                    [
                        GameUiChartPoint {
                            x: position,
                            y: 12.0,
                        },
                        GameUiChartPoint {
                            x: position,
                            y: 21.0,
                        },
                    ]
                })),
                label_anchors: [
                    GameUiChartPoint { x: 10.0, y: 39.0 },
                    GameUiChartPoint { x: 190.0, y: 39.0 },
                ],
                marker: geometry.angle_of_attack_marker,
            },
        ));
    }
    let groups = position_hud(&mut layout, primary, instruments, true, 0.55);
    Ok(Some(GameUiHud { layout, groups }))
}

fn tail_incidence_rates(session: &GameSession<'_>, state: TailFlightTickState) -> Option<[f64; 2]> {
    let [previous, current] = session.flight_record()?.samples().last_chunk::<2>()?;
    if current.tick_index != state.tick_index()
        || current.fraction != 0.0
        || current.flight_state != state.flight_state()
        || current.controls.incidence != state.incidence()
        || !previous.fraction.is_finite()
        || !current.fraction.is_finite()
    {
        return None;
    }
    let elapsed_ticks = current.tick_index.checked_sub(previous.tick_index)? as f64
        + (current.fraction - previous.fraction);
    let seconds = elapsed_ticks / f64::from(birdman_game_core::PHYSICS_HZ);
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    let rates = [
        (current.controls.incidence.elevator_rad() - previous.controls.incidence.elevator_rad())
            / seconds,
        (current.controls.incidence.rudder_rad() - previous.controls.incidence.rudder_rad())
            / seconds,
    ];
    rates.iter().all(|rate| rate.is_finite()).then_some(rates)
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

fn direction_arrow(north: f64, east: f64) -> Option<[GameUiChartPoint; 4]> {
    if north == 0.0 && east == 0.0 {
        return None;
    }
    let (sin, cos) = east.atan2(north).sin_cos();
    Some(
        [(0.0, -21.0), (-5.0, 2.0), (0.0, -2.0), (5.0, 2.0)].map(|(x, y)| GameUiChartPoint {
            x: 36.0 + x * cos - y * sin,
            y: 36.0 + x * sin + y * cos,
        }),
    )
}

fn hud_geometry(
    attitude: Option<GameUiHudAttitude>,
    flight_path: Option<f64>,
    heading: Option<f64>,
    angle_of_attack: Option<f64>,
) -> GameUiHudGeometry {
    let clip = GameUiRect {
        x: 26.0,
        y: 18.0,
        width: 188.0,
        height: 144.0,
    };
    let clip_radius = clip.height / 2.0;
    let horizon = attitude.map(|attitude| {
        let rotation = -attitude.roll_degrees.to_radians();
        let (rotation_sin, rotation_cos) = rotation.sin_cos();
        let pitch_offset = (attitude.pitch_degrees * 2.2).clamp(-55.0, 55.0);
        let rotate = |across: f64, down: f64| GameUiChartPoint {
            x: 120.0 + rotation_cos * across - rotation_sin * down,
            y: 90.0 + rotation_sin * across + rotation_cos * down,
        };
        GameUiHudAttitudeGeometry {
            label_rotation_degrees: -attitude.roll_degrees,
            pitch_ticks: [-30_i32, -20, -10, 0, 10, 20, 30].map(|pitch| {
                let offset = pitch_offset - f64::from(pitch) * 2.2;
                let half_width = if pitch == 0 {
                    94.0
                } else if pitch.abs() % 20 == 0 {
                    27.0
                } else {
                    17.0
                };
                GameUiHudPitchTick {
                    segment: clip_adi_segment(
                        [rotate(-half_width, offset), rotate(half_width, offset)],
                        clip,
                    ),
                    label_anchor: (pitch.abs() == 20)
                        .then(|| rotate(half_width + 18.0, offset + 4.0)),
                    label_degrees: (pitch.abs() == 20).then_some(pitch.unsigned_abs()),
                }
            }),
        }
    });
    let flight_path_center = flight_path.map(|angle| GameUiChartPoint {
        x: 120.0,
        y: 90.0 - angle.clamp(-30.0, 30.0) * 2.2,
    });
    GameUiHudGeometry {
        adi: (horizon.is_some() || flight_path_center.is_some()).then_some(GameUiHudAdiGeometry {
            canvas_size: [240.0, 180.0],
            clip,
            clip_radius,
            horizon,
            aircraft_segments: [
                [[72.0, 91.0], [108.0, 91.0]],
                [[108.0, 91.0], [113.0, 82.0]],
                [[113.0, 82.0], [127.0, 82.0]],
                [[127.0, 82.0], [132.0, 91.0]],
                [[132.0, 91.0], [168.0, 91.0]],
                [[120.0, 86.0], [120.0, 96.0]],
            ]
            .map(|segment| {
                segment.map(|[horizontal, vertical]| GameUiChartPoint {
                    x: horizontal,
                    y: vertical,
                })
            }),
            flight_path_center,
            flight_path_radius: 5.0,
        }),
        heading_ticks: heading.map_or_else(Vec::new, |heading| {
            (-6..=6)
                .map(|index| {
                    let offset = index * 10;
                    let major = offset % 30 == 0;
                    let bearing = rounded_bearing(heading + f64::from(offset));
                    let position = 110.0 + f64::from(offset) * 1.2;
                    GameUiHudHeadingTick {
                        segment: [
                            GameUiChartPoint {
                                x: position,
                                y: if major { 28.0 } else { 33.0 },
                            },
                            GameUiChartPoint {
                                x: position,
                                y: 40.0,
                            },
                        ],
                        stroke_width: if major { 1.5 } else { 1.0 },
                        label_anchor: (offset == 0).then_some(GameUiChartPoint {
                            x: position,
                            y: 22.0,
                        }),
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
        angle_of_attack_marker: angle_of_attack.map(|angle| {
            let position = 10.0 + (angle.clamp(-10.0, 20.0) + 10.0) * 6.0;
            [
                GameUiChartPoint {
                    x: position,
                    y: 5.0,
                },
                GameUiChartPoint {
                    x: position - 6.0,
                    y: 1.0,
                },
                GameUiChartPoint {
                    x: position + 6.0,
                    y: 1.0,
                },
            ]
        }),
    }
}

fn clip_adi_segment(
    segment: [GameUiChartPoint; 2],
    clip: GameUiRect,
) -> Option<[GameUiChartPoint; 2]> {
    let [start, end] = segment.map(|point| [point.x, point.y]);
    let delta = [end[0] - start[0], end[1] - start[1]];
    let radius = clip.height / 2.0;
    let length_squared = delta[0] * delta[0] + delta[1] * delta[1];
    if length_squared == 0.0 {
        let nearest_x = start[0].clamp(clip.x + radius, clip.x + clip.width - radius);
        return ((start[0] - nearest_x).hypot(start[1] - clip.y - radius) <= radius)
            .then_some(segment);
    }
    let mut visible: Option<[f64; 2]> = None;
    let mut include = |range: [f64; 2]| {
        let entry = range[0].max(0.0);
        let exit = range[1].min(1.0);
        if entry <= exit {
            visible = Some(visible.map_or([entry, exit], |previous| {
                [previous[0].min(entry), previous[1].max(exit)]
            }));
        }
    };
    let mut rectangle = [0.0_f64, 1.0_f64];
    let mut intersects_rectangle = true;
    for (axis, limits) in [
        (0, [clip.x + radius, clip.x + clip.width - radius]),
        (1, [clip.y, clip.y + clip.height]),
    ] {
        if delta[axis] == 0.0 {
            intersects_rectangle &= (limits[0]..=limits[1]).contains(&start[axis]);
        } else {
            let boundaries = limits.map(|edge| (edge - start[axis]) / delta[axis]);
            rectangle[0] = rectangle[0].max(boundaries[0].min(boundaries[1]));
            rectangle[1] = rectangle[1].min(boundaries[0].max(boundaries[1]));
        }
    }
    if intersects_rectangle {
        include(rectangle);
    }
    for center_x in [clip.x + radius, clip.x + clip.width - radius] {
        let offset = [start[0] - center_x, start[1] - clip.y - radius];
        let linear = offset[0] * delta[0] + offset[1] * delta[1];
        let constant = offset[0] * offset[0] + offset[1] * offset[1] - radius * radius;
        let discriminant = linear * linear - length_squared * constant;
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            include([
                (-linear - root) / length_squared,
                (-linear + root) / length_squared,
            ]);
        }
    }
    Some(visible?.map(|progress| GameUiChartPoint {
        x: start[0] + delta[0] * progress,
        y: start[1] + delta[1] * progress,
    }))
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
    32.0 * (0.875 * font).max(14.0)
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
        gap_px: [0.45 * font, 0.25 * font],
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
    text_width_ratio: f64,
) -> Vec<GameUiHudGroup> {
    let [padding_x, padding_y] = layout.padding_px;
    let content_width = (layout.width_px - 2.0 * padding_x).max(1.0);
    let minimum_readout_width = 14.0 * layout.font_size_px;
    let minimum_instrument_width = instruments
        .iter()
        .map(|node| hud_node_width(node, layout, text_width_ratio))
        .sum::<f64>()
        + instruments.len().saturating_sub(1) as f64 * layout.gap_px[0];
    let spare_width =
        content_width - minimum_readout_width - minimum_instrument_width - layout.gap_px[0];
    let side_by_side =
        allow_side_by_side && spare_width >= 0.0 && !readouts.is_empty() && !instruments.is_empty();
    let readout_width = if side_by_side {
        minimum_readout_width + spare_width * 0.45
    } else {
        content_width
    };
    let mut groups = hud_rows(
        "hud-readouts",
        GameUiHudPlacement::Readouts,
        readouts,
        readout_width,
        layout,
        text_width_ratio,
    );
    let readout_height = position_rows(
        &mut groups,
        padding_x,
        padding_y,
        readout_width,
        layout,
        text_width_ratio,
    );
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
        text_width_ratio,
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
        text_width_ratio,
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
    text_width_ratio: f64,
) -> Vec<GameUiHudGroup> {
    let mut rows: Vec<GameUiHudGroup> = Vec::new();
    let mut used_width = 0.0;
    for node in nodes {
        let minimum_width = hud_node_width(&node, layout, text_width_ratio).min(width);
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
    text_width_ratio: f64,
) -> f64 {
    let mut y = top;
    for group in groups.iter_mut() {
        let widths: Vec<_> = group
            .nodes
            .iter()
            .map(|node| hud_node_width(node, layout, text_width_ratio).min(width))
            .collect();
        let free_width =
            (width - widths.iter().sum::<f64>() - (widths.len() - 1) as f64 * layout.gap_px[0])
                .max(0.0);
        let mut x = left;
        let mut row_height = 0.0_f64;
        let node_count = group.nodes.len() as f64;
        for (node, minimum_width) in group.nodes.iter_mut().zip(widths) {
            let node_width = minimum_width + free_width / node_count;
            let node_height = hud_node_height(node, node_width, layout, text_width_ratio);
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

fn hud_node_width(node: &GameUiHudNode, layout: &GameUiHudLayout, text_width_ratio: f64) -> f64 {
    let font = layout.font_size_px;
    let mut lines = node.content.value().split('\n').map(UnicodeWidthStr::width);
    let caption = if node.label.is_empty() {
        0
    } else {
        UnicodeWidthStr::width(node.label.as_str()) + 1
    };
    let columns = (caption + lines.next().unwrap_or(0)).max(lines.max().unwrap_or(0));
    let text_width = columns as f64 * font * text_width_ratio;
    match &node.content {
        GameUiHudNodeContent::Text { tone, .. } => {
            if *tone == GameUiHudTone::Warning {
                return layout.width_px;
            }
            text_width.max(4.0 * font)
        }
        _ => text_width.max(4.0 * font) + 2.0 * layout.padding_px[0],
    }
}

fn hud_node_height(
    node: &mut GameUiHudNode,
    width: f64,
    layout: &GameUiHudLayout,
    text_width_ratio: f64,
) -> f64 {
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
                (UnicodeWidthStr::width(line) as f64 * font * text_width_ratio / content_width)
                    .ceil()
                    .max(1.0)
            })
            .sum::<f64>()
            .max(1.0)
    };
    let text_height = lines(&format!(
        "{}{}{}",
        node.label,
        if node.label.is_empty() { "" } else { " " },
        node.content.value()
    )) * font
        * 1.5;
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
        assert_eq!(direction_arrow(0.0, 0.0), None);
        assert_eq!(
            direction_arrow(1.0, 0.0).unwrap()[0],
            GameUiChartPoint { x: 36.0, y: 15.0 }
        );
        let east = direction_arrow(0.0, 1.0).unwrap()[0];
        assert!((east.x - 57.0).abs() < 1.0e-12 && (east.y - 36.0).abs() < 1.0e-12);
        let diagonal = direction_arrow(3.0, 4.0).unwrap()[0];
        assert!((diagonal.x - 52.8).abs() < 1.0e-12 && (diagonal.y - 23.4).abs() < 1.0e-12);
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
        assert_eq!(adi.horizon.unwrap().label_rotation_degrees, -11.0);
        assert_eq!(adi.canvas_size, [240.0, 180.0]);
        assert!((adi.flight_path_center.unwrap().y - 96.6).abs() < 1.0e-12);
        assert_eq!(
            geometry.angle_of_attack_marker.unwrap()[0],
            GameUiChartPoint { x: 100.0, y: 5.0 }
        );
        assert_eq!(geometry.heading_ticks.len(), 13);
        assert_eq!(
            geometry.heading_ticks[0].segment,
            [
                GameUiChartPoint { x: 38.0, y: 28.0 },
                GameUiChartPoint { x: 38.0, y: 40.0 }
            ]
        );
        assert!(geometry.heading_ticks[0].label.is_empty());
        assert_eq!(geometry.heading_ticks[6].label, "N");
        assert_eq!(
            geometry.heading_ticks[6].label_anchor,
            Some(GameUiChartPoint { x: 110.0, y: 22.0 })
        );
        assert!(geometry.heading_ticks[12].label.is_empty());
        assert_eq!(
            adi.horizon
                .unwrap()
                .pitch_ticks
                .iter()
                .filter_map(|tick| tick.label_degrees)
                .collect::<Vec<_>>(),
            vec![20, 20]
        );
        for (angle, path, position) in [
            (-100.0, (90.0 + 66.0) / 180.0, 10.0),
            (100.0, (90.0 - 66.0) / 180.0, 190.0),
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
                projected.adi.unwrap().horizon.unwrap().pitch_ticks[1]
                    .label_anchor
                    .unwrap()
                    .y,
                138.0 + angle.signum() * 55.0
            );
            assert!(
                (projected.adi.unwrap().flight_path_center.unwrap().y / 180.0 - path).abs()
                    < 1.0e-12
            );
            assert_eq!(projected.angle_of_attack_marker.unwrap()[0].x, position);
        }
        let attitude = |roll_degrees, pitch_degrees| {
            hud_geometry(
                Some(GameUiHudAttitude {
                    roll_degrees,
                    pitch_degrees,
                }),
                None,
                None,
                None,
            )
            .adi
            .unwrap()
        };
        let level = attitude(0.0, 0.0);
        assert_eq!(
            level.horizon.unwrap().pitch_ticks[3].segment,
            Some([
                GameUiChartPoint { x: 26.0, y: 90.0 },
                GameUiChartPoint { x: 214.0, y: 90.0 }
            ])
        );
        let pitched = attitude(0.0, 10.0);
        let segment = pitched.horizon.unwrap().pitch_ticks[3].segment.unwrap();
        assert_eq!(segment[0].y, 112.0);
        assert_eq!(segment[1].y, 112.0);
        let rolled = attitude(90.0, 0.0);
        let segment = rolled.horizon.unwrap().pitch_ticks[3].segment.unwrap();
        for (actual, expected) in segment.into_iter().zip([
            GameUiChartPoint { x: 120.0, y: 162.0 },
            GameUiChartPoint { x: 120.0, y: 18.0 },
        ]) {
            assert!((actual.x - expected.x).abs() < 1.0e-12);
            assert!((actual.y - expected.y).abs() < 1.0e-12);
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
        assert!(
            hud_geometry(None, None, Some(0.5), None).heading_ticks[0]
                .label
                .is_empty()
        );
        let hidden = hud_geometry(None, None, None, None);
        assert!(
            hidden.adi.is_none()
                && hidden.heading_ticks.is_empty()
                && hidden.angle_of_attack_marker.is_none()
        );
    }

    #[test]
    fn flight_path_only_retains_the_adi_region_without_an_attitude_layer() {
        let geometry = hud_geometry(None, Some(-3.0), None, None);
        let adi = geometry.adi.unwrap();
        assert!((adi.flight_path_center.unwrap().y - 96.6).abs() < 1.0e-12);
        assert_eq!(geometry.adi.unwrap().horizon, None);
    }
}
