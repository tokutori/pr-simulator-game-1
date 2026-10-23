use super::super::types::*;

pub(super) struct ChartData {
    pub presentation: GameUiChartPresentation,
    pub x_axis_label: String,
    pub y_axis_label: String,
    pub x_minimum: f64,
    pub x_maximum: f64,
    pub y_minimum: f64,
    pub y_maximum: f64,
    pub equal_axis_scale: bool,
    pub series: Vec<GameUiChartSeries>,
    pub vectors: Vec<ChartWindVector>,
    pub markers: Vec<GameUiChartMarker>,
    pub time_markers: Vec<GameUiChartMarker>,
    pub reference_lines: Vec<ChartReferenceLine>,
    pub cursor_x: Option<f64>,
    pub cursor_points: Vec<GameUiChartCursorPoint>,
}

pub(super) struct ChartDataCursor {
    pub id: String,
    pub cursor_x: Option<f64>,
    pub cursor_points: Vec<GameUiChartCursorPoint>,
    pub selected_wind_vector: Option<ChartWindVector>,
}

pub(super) struct ChartWindVector {
    pub label: String,
    pub color: String,
    pub segment: Option<[GameUiChartPoint; 2]>,
}

pub(super) struct ChartReferenceLine {
    pub value: f64,
    pub label: String,
    pub color: String,
}

#[derive(Clone, Copy)]
pub(super) struct ChartProjection {
    domain: [f64; 4],
    plot: [f64; 4],
}

impl ChartProjection {
    pub(super) fn new(domain: [f64; 4], plot: [f64; 4]) -> Self {
        Self { domain, plot }
    }

    pub(super) fn point(self, point: GameUiChartPoint) -> GameUiChartPoint {
        let [x_minimum, x_maximum, y_minimum, y_maximum] = self.domain;
        let [left, top, right, bottom] = self.plot;
        GameUiChartPoint {
            x: left + (point.x - x_minimum) / (x_maximum - x_minimum) * (right - left),
            y: bottom - (point.y - y_minimum) / (y_maximum - y_minimum) * (bottom - top),
        }
    }

    fn vector(self, vector: ChartWindVector) -> GameUiChartVector {
        let mut segments = Vec::new();
        if let Some([start, end]) = vector
            .segment
            .map(|points| points.map(|point| self.point(point)))
        {
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let length = dx.hypot(dy);
            if length > f64::EPSILON {
                segments.push([start, end]);
                let head_length = 12.0_f64.min(length * 0.45);
                let angle = dy.atan2(dx);
                for sign in [-1.0, 1.0] {
                    let angle = angle + sign * std::f64::consts::FRAC_PI_6;
                    let tip = GameUiChartPoint {
                        x: end.x - head_length * angle.cos(),
                        y: end.y - head_length * angle.sin(),
                    };
                    if let Some(segment) = super::clip_segment(
                        end,
                        tip,
                        [self.plot[0], self.plot[2], self.plot[1], self.plot[3]],
                    ) {
                        segments.push(segment);
                    }
                }
            }
        }
        GameUiChartVector {
            label: vector.label,
            color: vector.color,
            segments,
        }
    }

    pub(super) fn cursor(self, cursor: ChartDataCursor) -> GameUiChartCursor {
        GameUiChartCursor {
            id: cursor.id,
            cursor_x: cursor.cursor_x.map(|x| {
                self.point(GameUiChartPoint {
                    x,
                    y: self.domain[2],
                })
                .x
            }),
            cursor_points: cursor
                .cursor_points
                .into_iter()
                .map(|mut cursor| {
                    cursor.point = self.point(cursor.point);
                    cursor
                })
                .collect(),
            selected_wind_vector: cursor
                .selected_wind_vector
                .map(|vector| self.vector(vector)),
        }
    }
}

impl ChartData {
    pub(super) fn into_projected(self) -> GameUiChart {
        let projection = ChartProjection::new(
            [
                self.x_minimum,
                self.x_maximum,
                self.y_minimum,
                self.y_maximum,
            ],
            self.presentation.plot,
        );
        let markers = |markers: Vec<GameUiChartMarker>| {
            markers
                .into_iter()
                .map(|mut marker| {
                    marker.point = projection.point(marker.point);
                    marker
                })
                .collect()
        };
        GameUiChart {
            seek_target: (!self.equal_axis_scale).then_some(super::GameUiRangeTarget::Analysis),
            presentation: self.presentation,
            x_axis_label: self.x_axis_label,
            y_axis_label: self.y_axis_label,
            x_minimum: self.x_minimum,
            x_maximum: self.x_maximum,
            y_minimum: self.y_minimum,
            y_maximum: self.y_maximum,
            equal_axis_scale: self.equal_axis_scale,
            series: self
                .series
                .into_iter()
                .map(|mut series| {
                    for point in &mut series.points {
                        *point = projection.point(*point);
                    }
                    series
                })
                .collect(),
            vectors: self
                .vectors
                .into_iter()
                .map(|vector| projection.vector(vector))
                .collect(),
            markers: markers(self.markers),
            time_markers: markers(self.time_markers),
            reference_lines: self
                .reference_lines
                .into_iter()
                .map(|line| GameUiChartReferenceLine {
                    position_y_px: projection
                        .point(GameUiChartPoint {
                            x: self.x_minimum,
                            y: line.value,
                        })
                        .y,
                    label: line.label,
                    color: line.color,
                })
                .collect(),
            cursor_x: self.cursor_x.map(|x| {
                projection
                    .point(GameUiChartPoint {
                        x,
                        y: self.y_minimum,
                    })
                    .x
            }),
            cursor_points: self
                .cursor_points
                .into_iter()
                .map(|mut cursor| {
                    cursor.point = projection.point(cursor.point);
                    cursor
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_projection_inverts_height_and_bounds_arrowheads_by_the_visible_segment() {
        let projection = ChartProjection::new([-1.0, 1.0, -2.0, 2.0], [10.0, 20.0, 210.0, 120.0]);
        for (input, expected) in [
            ([-1.0, -2.0], [10.0, 120.0]),
            ([1.0, 2.0], [210.0, 20.0]),
            ([0.0, 0.0], [110.0, 70.0]),
        ] {
            assert_eq!(
                projection.point(GameUiChartPoint {
                    x: input[0],
                    y: input[1]
                }),
                GameUiChartPoint {
                    x: expected[0],
                    y: expected[1]
                }
            );
        }
        let vector = |end| {
            projection.vector(ChartWindVector {
                label: String::new(),
                color: "#ffffff".into(),
                segment: Some([
                    GameUiChartPoint { x: -1.0, y: 0.0 },
                    GameUiChartPoint { x: end, y: 0.0 },
                ]),
            })
        };
        let long = vector(1.0);
        assert_eq!(long.segments.len(), 3);
        for [start, end] in &long.segments[1..] {
            assert_eq!(*start, GameUiChartPoint { x: 210.0, y: 70.0 });
            assert!(((end.x - start.x).hypot(end.y - start.y) - 12.0).abs() < 1e-10);
            assert!(((end.y - 70.0).abs() - 6.0).abs() < 1e-10);
            assert!(end.x < start.x);
        }
        let short = vector(-0.99);
        assert_eq!(short.segments.len(), 3);
        for [start, end] in &short.segments[1..] {
            assert!(((end.x - start.x).hypot(end.y - start.y) - 0.45).abs() < 1e-10);
        }
        assert!(vector(-1.0).segments.is_empty());
        let boundary = projection.vector(ChartWindVector {
            label: String::new(),
            color: String::new(),
            segment: Some([
                GameUiChartPoint { x: 1.0, y: -2.0 },
                GameUiChartPoint { x: 1.0, y: 2.0 },
            ]),
        });
        for point in boundary.segments.into_iter().flatten() {
            assert!((10.0..=210.0).contains(&point.x) && (20.0..=120.0).contains(&point.y));
        }
        assert!(
            projection
                .vector(ChartWindVector {
                    label: String::new(),
                    color: String::new(),
                    segment: None
                })
                .segments
                .is_empty()
        );
    }
}
