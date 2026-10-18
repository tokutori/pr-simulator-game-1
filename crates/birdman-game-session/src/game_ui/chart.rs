use std::sync::OnceLock;

use birdman_game_core::{
    FlightRecord, FlightRecordPlaybackSample, FlightRecordQueryError, FlightRecordSample, NedPoint,
    SessionEndReason, WindError,
};
use serde::Deserialize;

use super::{environment, types::*};

pub(super) fn record_chart(
    record: &FlightRecord,
    kind: GameUiAnalysisChart,
    time: f64,
    extent: Option<GameUiChartExtent>,
) -> Result<GameUiChart, GameUiError> {
    let samples = record.samples();
    let first = samples
        .first()
        .ok_or(GameUiError::Record(FlightRecordQueryError::EmptyRecord))?;
    let cursor = record
        .sample_at_seconds(time)
        .map_err(GameUiError::Record)?;
    let duration = record
        .duration_seconds()
        .map_err(GameUiError::Record)?
        .max(0.01);
    let mut chart = GameUiChart {
        presentation: initial_presentation(extent)?,
        x_axis_label: "Time (s)".to_owned(),
        y_axis_label: String::new(),
        x_minimum: 0.0,
        x_maximum: duration,
        y_minimum: 0.0,
        y_maximum: 1.0,
        equal_axis_scale: false,
        series: Vec::new(),
        vectors: Vec::new(),
        markers: Vec::new(),
        time_markers: Vec::new(),
        reference_lines: Vec::new(),
        cursor_x: Some(time),
        cursor_points: Vec::new(),
    };
    match kind {
        GameUiAnalysisChart::Map => map_chart(&mut chart, record)?,
        GameUiAnalysisChart::Altitude => {
            chart.y_axis_label = "Altitude (m)".to_owned();
            let range = padded_range(
                samples
                    .iter()
                    .map(|sample| sample.telemetry.altitude_m)
                    .chain([0.0, first.telemetry.altitude_m]),
            );
            chart.y_minimum = range[0];
            chart.y_maximum = range[1];
            chart.series.push(series(
                "高度",
                "#70d6c8",
                samples
                    .iter()
                    .map(|sample| point(sample_time(record, sample), sample.telemetry.altitude_m))
                    .collect(),
            ));
            chart.reference_lines = vec![
                GameUiChartReferenceLine {
                    value: 0.0,
                    label: "静水面".to_owned(),
                    color: "#85c7e8".to_owned(),
                },
                GameUiChartReferenceLine {
                    value: first.telemetry.altitude_m,
                    label: format!("発進高度 {:.1} m", first.telemetry.altitude_m),
                    color: "#f2b35e".to_owned(),
                },
            ];
        }
        GameUiAnalysisChart::Speed => {
            chart.y_axis_label = "Speed (m/s)".to_owned();
            let range = padded_range(samples.iter().flat_map(|sample| {
                [
                    sample.telemetry.airspeed_mps,
                    sample.telemetry.groundspeed_mps,
                ]
            }));
            chart.y_minimum = range[0];
            chart.y_maximum = range[1];
            for (label, color, air_relative) in [
                ("対気速度", "#70d6c8", true),
                ("対地速度", "#f2b35e", false),
            ] {
                let speed = |sample: &FlightRecordSample| {
                    if air_relative {
                        sample.telemetry.airspeed_mps
                    } else {
                        sample.telemetry.groundspeed_mps
                    }
                };
                chart.series.push(series(
                    label,
                    color,
                    samples
                        .iter()
                        .map(|sample| point(sample_time(record, sample), speed(sample)))
                        .collect(),
                ));
            }
        }
    }
    let cursor = chart_cursor(kind, &chart, cursor, time);
    chart.cursor_x = cursor.cursor_x;
    chart.cursor_points = cursor.cursor_points;
    if let Some(selected) = cursor.selected_wind_vector {
        chart.vectors.insert(0, selected);
    }
    complete_presentation(&mut chart);
    Ok(chart)
}

fn initial_presentation(
    extent: Option<GameUiChartExtent>,
) -> Result<GameUiChartPresentation, GameUiError> {
    let measurement_minimum = GameUiChartExtent {
        width: 140.0,
        height: 100.0,
    };
    let viewport = extent.unwrap_or(GameUiChartExtent {
        width: 1000.0,
        height: 520.0,
    });
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width < measurement_minimum.width
        || viewport.height < measurement_minimum.height
    {
        return Err(GameUiError::InvalidViewport);
    }
    let axis = chart_text(
        String::new(),
        point(0.0, 0.0),
        16.0,
        400,
        GameUiChartTextAnchor::Middle,
        "#d5e0dc",
    );
    Ok(GameUiChartPresentation {
        viewport,
        measurement_minimum,
        scroll_overflow: true,
        plot: [64.0, 26.0, viewport.width - 20.0, viewport.height - 48.0],
        tick_font_size: 14.0,
        axis_font_size: 16.0,
        x_ticks: Vec::new(),
        y_ticks: Vec::new(),
        grid_lines: Vec::new(),
        axes_color: "#9db8b8".to_owned(),
        grid_color: "#456168".to_owned(),
        grid_opacity: 1.0,
        x_axis: axis.clone(),
        y_axis: axis,
        marker_labels: Vec::new(),
        time_marker_labels: Vec::new(),
        reference_labels: Vec::new(),
        legend: Vec::new(),
        map_guides: None,
        viewport_minimum_rem: [14.0, 10.0],
        legend_font_rem: 0.85,
        legend_gap_rem: 0.75,
        legend_swatch_rem: [0.75, 0.75],
        legend_swatch_gap_rem: 0.35,
        legend_gradient_rem: [2.0, 0.35],
        legend_gradient_radius_rem: 0.2,
    })
}

fn complete_presentation(chart: &mut GameUiChart) {
    let presentation = &mut chart.presentation;
    if chart.equal_axis_scale {
        let [left, top, right, bottom] = presentation.plot;
        let side = (right - left).min(bottom - top);
        let centre_x = (left + right) / 2.0;
        let centre_y = (top + bottom) / 2.0;
        presentation.plot = [
            centre_x - side / 2.0,
            centre_y - side / 2.0,
            centre_x + side / 2.0,
            centre_y + side / 2.0,
        ];
    }
    let [left, top, right, bottom] = presentation.plot;
    let width = right - left;
    let height = bottom - top;
    let project = |position: GameUiChartPoint| {
        point(
            left + (position.x - chart.x_minimum) / (chart.x_maximum - chart.x_minimum) * width,
            bottom - (position.y - chart.y_minimum) / (chart.y_maximum - chart.y_minimum) * height,
        )
    };
    let tick_count_x = (width / 80.0).floor().clamp(1.0, 4.0) as u32;
    let tick_count_y = (height / 32.0).floor().clamp(1.0, 4.0) as u32;
    presentation.x_ticks = (0..=tick_count_x)
        .map(|index| {
            let ratio = f64::from(index) / f64::from(tick_count_x);
            let value = chart.x_minimum + ratio * (chart.x_maximum - chart.x_minimum);
            GameUiChartTick {
                value,
                ratio,
                text: chart_text(
                    format_chart_tick(value),
                    point(left + ratio * width, bottom + 20.0),
                    14.0,
                    400,
                    GameUiChartTextAnchor::Middle,
                    "#d5e0dc",
                ),
            }
        })
        .collect();
    presentation.y_ticks = (0..=tick_count_y)
        .map(|index| {
            let ratio = f64::from(index) / f64::from(tick_count_y);
            let value = chart.y_minimum + ratio * (chart.y_maximum - chart.y_minimum);
            GameUiChartTick {
                value,
                ratio,
                text: chart_text(
                    format_chart_tick(value),
                    point(left - 10.0, bottom - ratio * height + 5.0),
                    14.0,
                    400,
                    GameUiChartTextAnchor::End,
                    "#d5e0dc",
                ),
            }
        })
        .collect();
    presentation.grid_lines = (1..=3)
        .map(|index| {
            let y = top + height * f64::from(index) / 4.0;
            [point(left, y), point(right, y)]
        })
        .collect();
    presentation.x_axis = chart_text(
        chart.x_axis_label.clone(),
        point((left + right) / 2.0, presentation.viewport.height - 8.0),
        16.0,
        400,
        GameUiChartTextAnchor::Middle,
        "#d5e0dc",
    );
    presentation.y_axis = chart_text(
        chart.y_axis_label.clone(),
        point(16.0, (top + bottom) / 2.0),
        16.0,
        400,
        GameUiChartTextAnchor::Middle,
        "#d5e0dc",
    );
    presentation.y_axis.rotation_degrees = -90.0;
    let marker_text = |marker: &GameUiChartMarker, offset: [f64; 2], weight| {
        let position = project(marker.point);
        let estimated_width = marker.label.encode_utf16().count() as f64 * 14.0 * 0.65;
        let position = point(
            (position.x + offset[0])
                .min(presentation.viewport.width - estimated_width - 4.0)
                .max(4.0),
            (position.y + offset[1])
                .min(presentation.viewport.height - 4.0)
                .max(16.0),
        );
        chart_text(
            marker.label.clone(),
            position,
            14.0,
            weight,
            GameUiChartTextAnchor::Start,
            "#f3f4e8",
        )
    };
    presentation.marker_labels = chart
        .markers
        .iter()
        .map(|marker| marker_text(marker, [10.0, -8.0], 600))
        .collect();
    presentation.time_marker_labels = chart
        .time_markers
        .iter()
        .map(|marker| marker_text(marker, [7.0, 18.0], 500))
        .collect();
    presentation.reference_labels = chart
        .reference_lines
        .iter()
        .map(|line| {
            chart_text(
                line.label.clone(),
                point(
                    left + 8.0,
                    project(point(chart.x_minimum, line.value)).y - 7.0,
                ),
                14.0,
                500,
                GameUiChartTextAnchor::Start,
                "#f3f4e8",
            )
        })
        .collect();
    presentation.legend = chart
        .series
        .iter()
        .map(|series| GameUiChartLegend {
            label: series.label.clone(),
            color: series.color.clone(),
            segment_gradient: series
                .segment_colors
                .first()
                .zip(series.segment_colors.last())
                .map(|(first, last)| [first.clone(), last.clone()]),
        })
        .chain(chart.markers.iter().map(|marker| GameUiChartLegend {
            label: marker.label.clone(),
            color: marker.color.clone(),
            segment_gradient: None,
        }))
        .chain(
            chart
                .vectors
                .iter()
                .enumerate()
                .filter(|(_, vector)| !vector.label.is_empty())
                .map(|(index, vector)| GameUiChartLegend {
                    label: if chart.equal_axis_scale && index == 0 {
                        "選択sampleの風（矢印は空気の移動先）".to_owned()
                    } else {
                        vector.label.clone()
                    },
                    color: vector.color.clone(),
                    segment_gradient: None,
                }),
        )
        .collect();
    if chart.equal_axis_scale {
        let distance = scale_bar_distance(chart.x_maximum - chart.x_minimum);
        let scale_width = distance / (chart.x_maximum - chart.x_minimum) * width;
        let scale_x = right - scale_width - 14.0;
        let scale_y = bottom - 18.0;
        let north_x = right - 22.0;
        let north_top = (top + 18.0).min(bottom);
        let north_length = (bottom - north_top).min(34.0);
        let north_head_height = north_length.min(10.0);
        let north_head_width = 6.0_f64.min(north_x - left).min(right - north_x);
        presentation.map_guides = Some(GameUiChartMapGuides {
            scale_distance_m: distance,
            scale_segments: [
                [
                    point(scale_x, scale_y),
                    point(scale_x + scale_width, scale_y),
                ],
                [point(scale_x, scale_y - 6.0), point(scale_x, scale_y + 3.0)],
                [
                    point(scale_x + scale_width, scale_y - 6.0),
                    point(scale_x + scale_width, scale_y + 3.0),
                ],
            ],
            scale_label: chart_text(
                format!("{} m", format_chart_tick(distance)),
                point(scale_x + scale_width / 2.0, scale_y - 8.0),
                14.0,
                600,
                GameUiChartTextAnchor::Middle,
                "#f3f4e8",
            ),
            north_segments: [
                [
                    point(north_x, north_top + north_length),
                    point(north_x, north_top),
                ],
                [
                    point(north_x, north_top),
                    point(north_x - north_head_width, north_top + north_head_height),
                ],
                [
                    point(north_x, north_top),
                    point(north_x + north_head_width, north_top + north_head_height),
                ],
            ],
            north_label: chart_text(
                "N".to_owned(),
                point(north_x, north_top - 6.0),
                14.0,
                600,
                GameUiChartTextAnchor::Middle,
                "#f3f4e8",
            ),
        });
    }
}

fn chart_text(
    label: String,
    position: GameUiChartPoint,
    font_size: f64,
    font_weight: u16,
    anchor: GameUiChartTextAnchor,
    color: &str,
) -> GameUiChartText {
    GameUiChartText {
        label,
        position,
        font_size,
        font_weight,
        rotation_degrees: 0.0,
        anchor,
        color: color.to_owned(),
    }
}

fn format_chart_tick(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let magnitude = value.abs();
    let digits = if magnitude >= 100.0 {
        0
    } else if magnitude >= 10.0 {
        1
    } else {
        2
    };
    let mut buffer = ryu_js::Buffer::new();
    let raw = buffer.format_to_fixed(value, digits);
    let trimmed = if raw.contains('.') && !raw.contains('e') {
        raw.trim_end_matches('0').trim_end_matches('.')
    } else {
        raw
    };
    if trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn scale_bar_distance(span: f64) -> f64 {
    let desired = span / 4.0;
    let magnitude = 10.0_f64.powf(desired.log10().floor());
    let normalized = desired / magnitude;
    let factor = if normalized >= 5.0 {
        5.0
    } else if normalized >= 2.0 {
        2.0
    } else {
        1.0
    };
    factor * magnitude
}

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn tick_labels_preserve_ecmascript_fixed_rounding_and_negative_zero() {
        for (value, expected) in [
            (0.0, "0"),
            (-0.0, "0"),
            (-0.001, "0"),
            (0.125, "0.13"),
            (-0.125, "-0.13"),
            (2.675, "2.67"),
            (10.25, "10.3"),
            (-10.25, "-10.3"),
            (100.5, "101"),
            (1.0e21, "1e+21"),
            (1.2e30, "1.2e+30"),
        ] {
            assert_eq!(format_chart_tick(value), expected);
        }
    }

    #[test]
    fn map_scale_uses_the_web_one_two_five_policy() {
        for (span, expected) in [
            (4.0, 1.0),
            (8.0, 2.0),
            (20.0, 5.0),
            (100.0, 20.0),
            (1000.0, 200.0),
        ] {
            assert_eq!(scale_bar_distance(span), expected);
        }
    }
}

pub(super) fn chart_cursor(
    kind: GameUiAnalysisChart,
    plot: &GameUiChart,
    sample: FlightRecordPlaybackSample,
    time: f64,
) -> GameUiChartCursor {
    let mut cursor = GameUiChartCursor {
        id: "game-analysis-plot".to_owned(),
        cursor_x: Some(time),
        cursor_points: Vec::new(),
        selected_wind_vector: None,
    };
    match kind {
        GameUiAnalysisChart::Map => {
            cursor.cursor_x = None;
            let position = sample.telemetry.composite_cg_position_ned_m.components();
            let start = point(position[1], position[0]);
            cursor.cursor_points.push(GameUiChartCursorPoint {
                point: start,
                color: plot.series[0].color.clone(),
            });
            let [north, east, _] = sample.telemetry.wind_velocity_ned_mps.components();
            let speed = north.hypot(east);
            let half_range = (plot.x_maximum - plot.x_minimum) * 0.75 / 2.0;
            let scale = if speed > 0.0 {
                half_range * 0.2 / speed
            } else {
                0.0
            };
            let end = point(start.x + east * scale, start.y + north * scale);
            cursor.selected_wind_vector = Some(GameUiChartVector {
                label: format!(
                    "選択sampleの風 · h {:.1} m · 矢印は空気の移動先 · {scale:.1} m/(m/s)",
                    sample.telemetry.altitude_m
                ),
                color: "#f2b35e".to_owned(),
                segment: if speed > 0.0 {
                    clip_segment(
                        start,
                        end,
                        [
                            plot.x_minimum,
                            plot.x_maximum,
                            plot.y_minimum,
                            plot.y_maximum,
                        ],
                    )
                } else {
                    None
                },
            });
        }
        GameUiAnalysisChart::Altitude => cursor.cursor_points.push(GameUiChartCursorPoint {
            point: point(time, sample.telemetry.altitude_m),
            color: plot.series[0].color.clone(),
        }),
        GameUiAnalysisChart::Speed => {
            cursor.cursor_points.push(GameUiChartCursorPoint {
                point: point(time, sample.telemetry.airspeed_mps),
                color: plot.series[0].color.clone(),
            });
            cursor.cursor_points.push(GameUiChartCursorPoint {
                point: point(time, sample.telemetry.groundspeed_mps),
                color: plot.series[1].color.clone(),
            });
        }
    }
    cursor
}

fn map_chart(chart: &mut GameUiChart, record: &FlightRecord) -> Result<(), GameUiError> {
    let samples = record.samples();
    let points: Vec<_> = samples.iter().map(map_point).collect();
    let east = range(points.iter().map(|position| position.x));
    let north = range(points.iter().map(|position| position.y));
    let center_east = (east[0] + east[1]) / 2.0;
    let center_north = (north[0] + north[1]) / 2.0;
    let half_range = (east[1] - east[0]).max(north[1] - north[0]).max(2.0) / 2.0;
    let bounds = [
        center_east - half_range / 0.75,
        center_east + half_range / 0.75,
        center_north - half_range / 0.75,
        center_north + half_range / 0.75,
    ];
    chart.x_axis_label = "東 E (m)".to_owned();
    chart.y_axis_label = "北 N (m)".to_owned();
    chart.x_minimum = bounds[0];
    chart.x_maximum = bounds[1];
    chart.y_minimum = bounds[2];
    chart.y_maximum = bounds[3];
    chart.equal_axis_scale = true;
    chart.cursor_x = None;
    let colors = (0..samples.len().saturating_sub(1))
        .map(|index| {
            progress_color(if samples.len() <= 2 {
                0.5
            } else {
                index as f64 / (samples.len() - 2) as f64
            })
        })
        .collect();
    chart.series.push(GameUiChartSeries {
        label: "飛行軌跡（紫=開始、黄=終端）".to_owned(),
        color: progress_color(0.0),
        points: points.clone(),
        segment_colors: colors,
    });
    let terminal = record
        .finalization()
        .map_or("End", |value| match value.reason {
            SessionEndReason::WaterContact => "Splash",
            SessionEndReason::TimeLimit => "TimeLimit",
            SessionEndReason::ManualAbort => "ManualAbort",
            SessionEndReason::OutOfValidEnvelope => "OutsideEnvelope",
            SessionEndReason::FatalSimulationError => "SimulationError",
        });
    if let (Some(first), Some(last)) = (points.first(), points.last()) {
        chart.markers.push(marker("Start", "#f3f4e8", *first));
        chart.markers.push(marker(terminal, "#f29c78", *last));
    }
    let duration = record.duration_seconds().map_err(GameUiError::Record)?;
    let interval = if duration <= 12.0 { 1.0 } else { 5.0 };
    let mut marker_time = interval;
    while marker_time < duration {
        if let Some(sample) = samples
            .iter()
            .find(|sample| sample_time(record, sample) >= marker_time)
        {
            chart.time_markers.push(marker(
                &format!("{marker_time} s"),
                "#d5e0dc",
                map_point(sample),
            ));
        }
        marker_time += interval;
    }
    wind_grid(chart, record, center_north, center_east, half_range, bounds)?;
    if environment::registered(record.header().scenario)
        && record.header().scenario.environment_version == 6
    {
        venue(chart, bounds)?;
    }
    Ok(())
}

fn wind_grid(
    chart: &mut GameUiChart,
    record: &FlightRecord,
    north: f64,
    east: f64,
    half_range: f64,
    bounds: [f64; 4],
) -> Result<(), GameUiError> {
    let Some(wind) =
        environment::registered_wind(record.header().scenario).map_err(GameUiError::Environment)?
    else {
        return Ok(());
    };
    let altitude = record
        .samples()
        .first()
        .map_or(0.0, |sample| sample.telemetry.altitude_m.max(0.0));
    let mut samples = Vec::with_capacity(25);
    for row in 0..5 {
        for column in 0..5 {
            let position = NedPoint::try_new(
                north - half_range + f64::from(row) * half_range / 2.0,
                east - half_range + f64::from(column) * half_range / 2.0,
                -altitude,
            )
            .map_err(|_| GameUiError::InvalidTime)?;
            let velocity = match wind.velocity_at(position) {
                Ok(velocity) => velocity.components(),
                Err(WindError::OutsideGrid) => return Ok(()),
                Err(error) => {
                    return Err(GameUiError::Environment(
                        birdman_game_format::EnvironmentFormatError::Wind(error),
                    ));
                }
            };
            samples.push((position.components(), velocity));
        }
    }
    let maximum_speed = samples
        .iter()
        .map(|(_, velocity)| velocity[0].hypot(velocity[1]))
        .fold(0.0, f64::max);
    let scale = if maximum_speed > 0.0 {
        half_range * 0.12 / maximum_speed
    } else {
        0.0
    };
    let vertical = range(samples.iter().map(|(_, velocity)| velocity[2]));
    for (index, (position, velocity)) in samples.into_iter().enumerate() {
        let start = point(position[1], position[0]);
        let end = point(start.x + velocity[1] * scale, start.y + velocity[0] * scale);
        chart.vectors.push(GameUiChartVector { label: if index == 0 { format!("風断面 h {altitude:.1} m · W_D {:.1}…{:.1} m/s · 矢印は空気の移動先 · 1 m/s = {scale:.1} m", vertical[0], vertical[1]) } else { String::new() },
            color: "#c7eb72".to_owned(), segment: if scale > 0.0 && velocity[0].hypot(velocity[1]) > 0.0 { clip_segment(start, end, bounds) } else { None } });
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VenueMapAsset {
    shorelines_north_east_meters: Vec<Vec<[f64; 2]>>,
}
struct VenueGeometry {
    lines: Vec<GameUiChartSeries>,
    markers: Vec<GameUiChartMarker>,
}
static VENUE: OnceLock<Result<VenueGeometry, GameUiError>> = OnceLock::new();

fn venue(chart: &mut GameUiChart, bounds: [f64; 4]) -> Result<(), GameUiError> {
    let geometry = VENUE
        .get_or_init(build_venue)
        .as_ref()
        .map_err(|error| *error)?;
    for line in &geometry.lines {
        let mut fragments = Vec::new();
        let mut current = Vec::new();
        for pair in line.points.windows(2) {
            let Some([start, end]) = clip_segment(pair[0], pair[1], bounds) else {
                if current.len() > 1 {
                    fragments.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                continue;
            };
            if current.last().is_some_and(|last: &GameUiChartPoint| {
                (last.x - start.x).hypot(last.y - start.y) > 1e-8
            }) {
                if current.len() > 1 {
                    fragments.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            if current.last() != Some(&start) {
                current.push(start);
            }
            current.push(end);
            if (end.x - pair[1].x).hypot(end.y - pair[1].y) > 1e-8 && current.len() > 1 {
                fragments.push(std::mem::take(&mut current));
            }
        }
        if current.len() > 1 {
            fragments.push(current);
        }
        for (index, points) in fragments.into_iter().enumerate() {
            chart.series.push(series(
                if index == 0 { &line.label } else { "" },
                &line.color,
                points,
            ));
        }
    }
    chart.markers.extend(
        geometry
            .markers
            .iter()
            .filter(|marker| inside(marker.point, bounds))
            .cloned(),
    );
    Ok(())
}

fn build_venue() -> Result<VenueGeometry, GameUiError> {
    let map: VenueMapAsset =
        serde_json::from_str(include_str!("../../../../assets/biwa-analysis-map.json"))
            .map_err(|_| GameUiError::InvalidVenueMap)?;
    let features =
        crate::venue_structures::registered_features().map_err(|_| GameUiError::InvalidVenueMap)?;
    let to_points = |positions: &[[f64; 2]]| {
        positions
            .iter()
            .map(|position| point(position[1], position[0]))
            .collect()
    };
    let mut lines: Vec<_> = map
        .shorelines_north_east_meters
        .iter()
        .enumerate()
        .map(|(index, positions)| {
            series(
                if index == 0 {
                    "彦根 湖岸線（OSM）"
                } else {
                    "彦根 湖岸線（OSM・続き）"
                },
                "#77b6c8",
                to_points(positions),
            )
        })
        .collect();
    let mut markers = Vec::new();
    for feature in &features.features {
        let beach = feature.kind == "beach";
        if matches!(feature.kind.as_str(), "pier" | "quay" | "breakwater")
            || (beach && feature.name.as_deref() == Some("松原水泳場"))
        {
            let fallback = match feature.kind.as_str() {
                "pier" => "桟橋（OSM）",
                "quay" => "岸壁（OSM）",
                "breakwater" => "防波堤（OSM）",
                _ => "砂浜（OSM）",
            };
            lines.push(series(
                feature.name.as_deref().unwrap_or(fallback),
                if beach { "#d8c89b" } else { "#a08f70" },
                to_points(&feature.north_east_meters),
            ));
        }
        if beach
            && let Some(name) = &feature.name
            && !feature.north_east_meters.is_empty()
        {
            let count = feature.north_east_meters.len() as f64;
            markers.push(marker(
                name,
                "#d8c89b",
                point(
                    feature
                        .north_east_meters
                        .iter()
                        .map(|position| position[1])
                        .sum::<f64>()
                        / count,
                    feature
                        .north_east_meters
                        .iter()
                        .map(|position| position[0])
                        .sum::<f64>()
                        / count,
                ),
            ));
        }
    }
    let platform = crate::launch_venue()
        .map_err(|_| GameUiError::InvalidVenueMap)?
        .platform;
    let [forward_north, forward_east] = platform.horizontal_direction_ned();
    let positions = [
        (0.0, -1.0),
        (0.0, 1.0),
        (-platform.length_meters, 1.0),
        (-platform.length_meters, -1.0),
        (0.0, -1.0),
    ]
    .into_iter()
    .map(|(along, across)| {
        point(
            along * forward_east + across * platform.width_meters / 2.0 * forward_north,
            along * forward_north - across * platform.width_meters / 2.0 * forward_east,
        )
    })
    .collect();
    lines.push(series("発進台", "#e7c27b", positions));
    if lines
        .iter()
        .flat_map(|line| &line.points)
        .chain(markers.iter().map(|marker| &marker.point))
        .any(|position| !position.x.is_finite() || !position.y.is_finite())
    {
        return Err(GameUiError::InvalidVenueMap);
    }
    Ok(VenueGeometry { lines, markers })
}

fn clip_segment(
    start: GameUiChartPoint,
    end: GameUiChartPoint,
    bounds: [f64; 4],
) -> Option<[GameUiChartPoint; 2]> {
    let delta_x = end.x - start.x;
    let delta_y = end.y - start.y;
    let mut lower: f64 = 0.0;
    let mut upper: f64 = 1.0;
    for (direction, distance) in [
        (-delta_x, start.x - bounds[0]),
        (delta_x, bounds[1] - start.x),
        (-delta_y, start.y - bounds[2]),
        (delta_y, bounds[3] - start.y),
    ] {
        if direction == 0.0 {
            if distance < 0.0 {
                return None;
            }
            continue;
        }
        let ratio = distance / direction;
        if direction < 0.0 {
            lower = lower.max(ratio);
        } else {
            upper = upper.min(ratio);
        }
        if lower > upper {
            return None;
        }
    }
    Some([
        point(start.x + lower * delta_x, start.y + lower * delta_y),
        point(start.x + upper * delta_x, start.y + upper * delta_y),
    ])
}

fn inside(position: GameUiChartPoint, bounds: [f64; 4]) -> bool {
    position.x >= bounds[0]
        && position.x <= bounds[1]
        && position.y >= bounds[2]
        && position.y <= bounds[3]
}
fn map_point(sample: &FlightRecordSample) -> GameUiChartPoint {
    let position = sample.telemetry.composite_cg_position_ned_m.components();
    point(position[1], position[0])
}
fn sample_time(record: &FlightRecord, sample: &FlightRecordSample) -> f64 {
    (sample.tick_index as f64 + sample.fraction) / f64::from(record.header().physics_hz)
}
fn point(x: f64, y: f64) -> GameUiChartPoint {
    GameUiChartPoint { x, y }
}
fn series(label: &str, color: &str, points: Vec<GameUiChartPoint>) -> GameUiChartSeries {
    GameUiChartSeries {
        label: label.to_owned(),
        color: color.to_owned(),
        points,
        segment_colors: Vec::new(),
    }
}
fn marker(label: &str, color: &str, point: GameUiChartPoint) -> GameUiChartMarker {
    GameUiChartMarker {
        label: label.to_owned(),
        color: color.to_owned(),
        point,
    }
}
fn range(values: impl Iterator<Item = f64>) -> [f64; 2] {
    values.fold([f64::INFINITY, f64::NEG_INFINITY], |range, value| {
        [range[0].min(value), range[1].max(value)]
    })
}
fn padded_range(values: impl Iterator<Item = f64>) -> [f64; 2] {
    let range = range(values);
    let padding = ((range[1] - range[0]) * 0.08)
        .max(range[1].abs() * 0.02)
        .max(0.5);
    [range[0] - padding, range[1] + padding]
}
fn progress_color(progress: f64) -> String {
    let progress = progress.clamp(0.0, 1.0);
    let lower = if progress <= 0.5 {
        [68.0, 1.0, 84.0]
    } else {
        [33.0, 145.0, 140.0]
    };
    let upper = if progress <= 0.5 {
        [33.0, 145.0, 140.0]
    } else {
        [253.0, 231.0, 37.0]
    };
    let ratio = if progress <= 0.5 {
        progress * 2.0
    } else {
        (progress - 0.5) * 2.0
    };
    let channels: [u8; 3] = core::array::from_fn(|index| {
        (lower[index] + (upper[index] - lower[index]) * ratio).round() as u8
    });
    format!("#{:02x}{:02x}{:02x}", channels[0], channels[1], channels[2])
}
