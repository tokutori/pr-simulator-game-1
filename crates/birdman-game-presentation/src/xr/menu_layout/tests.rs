use super::*;
use crate::game_ui::GameUiEmphasis;

fn source() -> MenuLayoutSource {
    MenuLayoutSource {
        panel_id: "menu".into(),
        title: String::new(),
        caption: String::new(),
        sections: Some(vec![GameUiSection {
            id: "actions".into(),
            label: String::new(),
            layout: GameUiSectionLayout::Row,
        }]),
        controls: ["start", "back"]
            .into_iter()
            .map(|id| MenuLayoutControl {
                id: id.into(),
                section: Some("actions".into()),
                label: id.into(),
                widget: MenuWidget::Button {
                    presentation: GameUiButtonPresentation::Action {
                        emphasis: GameUiEmphasis::Normal,
                    },
                },
            })
            .collect(),
        surface_size: MenuSize {
            width: 2.4,
            height: 1.8,
        },
        font_meters: 0.04,
        font: MenuFont {
            family: "fixture".into(),
            weight: 600,
            style: MenuFontStyle::Normal,
            generation: 0,
        },
        locale: "ja".into(),
        previous_label: "Prev".into(),
        next_label: "Next".into(),
    }
}

fn measurements(plan: &MenuLayoutPlan) -> Vec<MenuTextMeasurement> {
    plan.texts
        .iter()
        .map(|text| MenuTextMeasurement {
            identity: text.identity.clone(),
            lines: vec![MenuMeasuredLine {
                paragraph_index: 0,
                value: text.value.clone(),
                advance_meters: text.value.chars().count() as f64 * 0.01,
                left_meters: 0.0,
                right_meters: text.value.chars().count() as f64 * 0.01,
                ascent_meters: if text.value.is_empty() { 0.0 } else { 0.02 },
                descent_meters: 0.0,
            }],
        })
        .collect()
}

#[test]
fn responsive_rows_and_measured_positions_share_one_metric_contract() {
    let landscape = plan(source()).unwrap();
    assert_eq!(landscape.rows.len(), 1);
    assert_eq!(landscape.rows[0].controls.len(), 2);
    let geometry = arrange(&landscape, &measurements(&landscape)).unwrap();
    assert!((geometry.controls[0].bounds.y - 0.014).abs() < 1.0e-12);
    assert!((geometry.controls[0].bounds.height - 0.094).abs() < 1.0e-12);
    assert!((geometry.controls[1].bounds.x - 1.18).abs() < 1.0e-12);
    assert!((geometry.content_height_meters - 0.122).abs() < 1.0e-12);
    let mut portrait_source = source();
    portrait_source.surface_size.width = 0.5;
    let portrait = plan(portrait_source).unwrap();
    assert_eq!(portrait.rows.len(), 2);
    let geometry = arrange(&portrait, &measurements(&portrait)).unwrap();
    assert!((geometry.content_height_meters - 0.23).abs() < 1.0e-12);
}

#[test]
fn mismatched_and_invalid_measurements_are_rejected_before_arrangement() {
    let prepared = plan(source()).unwrap();
    let mut measured = measurements(&prepared);
    assert_eq!(
        arrange(&prepared, &measured[1..]),
        Err(MenuLayoutFailure::MissingTextMetrics)
    );
    measured.push(measured[0].clone());
    assert_eq!(
        arrange(&prepared, &measured),
        Err(MenuLayoutFailure::StaleTextMetrics)
    );
    measured.pop();
    let mut changed = source();
    changed.font.generation = 1;
    assert_eq!(
        arrange(&plan(changed).unwrap(), &measured),
        Err(MenuLayoutFailure::StaleTextMetrics)
    );
    measured[2].lines[0].advance_meters = f64::NAN;
    assert_eq!(
        arrange(&prepared, &measured),
        Err(MenuLayoutFailure::InvalidTextMetrics)
    );
    let mut invalid = source();
    invalid.controls[1].id = invalid.controls[0].id.clone();
    assert_eq!(plan(invalid), Err(MenuLayoutFailure::InvalidLayout));
    let mut chart_source = source();
    chart_source.controls[0].widget = MenuWidget::Chart {
        layout: MenuChartLayout {
            size: [400.0, 240.0],
            plot: [40.0, 20.0, 380.0, 200.0],
            tick_font_size: 12.0,
            legend: Vec::new(),
            labels: vec![MenuChartLabel {
                role: MenuTextRole::XAxis,
                index: 0,
                text: GameUiChartText {
                    label: "Axis".into(),
                    position: crate::game_ui::GameUiChartPoint { x: 200.0, y: 220.0 },
                    font_size: 12.0,
                    font_weight: 600,
                    rotation_degrees: 0.0,
                    anchor: crate::game_ui::GameUiChartTextAnchor::Middle,
                    color: "#fff".into(),
                },
            }],
        },
    };
    let original = plan(chart_source.clone()).unwrap();
    let MenuWidget::Chart { layout } = &mut chart_source.controls[0].widget else {
        panic!("Expected chart");
    };
    layout.tick_font_size = 24.0;
    let changed = plan(chart_source).unwrap();
    let chart_identity = |plan: &MenuLayoutPlan| {
        plan.texts
            .iter()
            .find(|text| text.chart_text.is_some())
            .unwrap()
            .identity
            .clone()
    };
    assert_ne!(chart_identity(&original), chart_identity(&changed));
    assert_eq!(
        arrange(&changed, &measurements(&original)),
        Err(MenuLayoutFailure::StaleTextMetrics)
    );
}

#[test]
fn chart_ink_uses_signed_bearings_and_rotated_baseline_coordinates() {
    let projection = MenuChartProjection {
        baseline: MenuPoint { x: 2.0, y: 3.0 },
        rotation_degrees: 90.0,
        anchor: crate::game_ui::GameUiChartTextAnchor::Start,
    };
    let line = MenuMeasuredLine {
        paragraph_index: 0,
        value: "Axis".into(),
        advance_meters: 0.4,
        left_meters: -0.1,
        right_meters: 0.5,
        ascent_meters: 0.2,
        descent_meters: 0.1,
    };
    let geometry = chart_line(&projection, &line, 1, 0.3);
    assert!((geometry.anchor.x - 1.7).abs() < 1.0e-12);
    assert!((geometry.anchor.y - 3.0).abs() < 1.0e-12);
    assert!((geometry.bounds.x - 1.6).abs() < 1.0e-12);
    assert!((geometry.bounds.y - 3.1).abs() < 1.0e-12);
    assert!((geometry.bounds.width - 0.3).abs() < 1.0e-12);
    assert!((geometry.bounds.height - 0.4).abs() < 1.0e-12);
}

#[test]
fn viewport_projects_unclipped_tracks_clipped_ink_and_reversible_points() {
    let source = MenuViewportSource {
        surface_size: MenuSize {
            width: 2.0,
            height: 1.0,
        },
        padding_meters: 0.1,
        gap_meters: 0.05,
        content_height_meters: 2.0,
        footer_height_meters: 0.2,
        progress: 0.5,
    };
    let projection = viewport::project(
        source.clone(),
        vec![
            MenuRect {
                x: 0.0,
                y: 0.675,
                width: 0.4,
                height: 0.2,
            },
            MenuRect {
                x: 0.0,
                y: 0.0,
                width: 0.4,
                height: 0.2,
            },
            MenuRect {
                x: 0.0,
                y: 0.8,
                width: 0.0,
                height: 0.0,
            },
        ],
        vec![
            MenuPoint { x: -0.7, y: 0.325 },
            MenuPoint { x: 1.0, y: 0.5 },
        ],
        vec![MenuPoint { x: 0.2, y: 0.8 }],
    )
    .unwrap();
    assert!((projection.content_clip.height - 0.55).abs() < 1.0e-12);
    assert!((projection.offset_meters - 0.725).abs() < 1.0e-12);
    assert!((projection.page_progress - 0.55 / 1.45).abs() < 1.0e-12);
    assert!((projection.rectangles[0].full.y - 0.05).abs() < 1.0e-12);
    let clipped = projection.rectangles[0].visible.unwrap();
    assert!((clipped.y - 0.1).abs() < 1.0e-12);
    assert!((clipped.height - 0.15).abs() < 1.0e-12);
    assert!(projection.rectangles[1].visible.is_none());
    assert!(projection.rectangles[2].visible.is_none());
    let restored = projection.panel_points[0].unwrap();
    assert!((restored.x - 0.2).abs() < 1.0e-12);
    assert!((restored.y - 0.8).abs() < 1.0e-12);
    assert!(projection.panel_points[1].is_none());
    assert!((projection.document_points[0].x + 0.7).abs() < 1.0e-12);
    assert!((projection.document_points[0].y - 0.325).abs() < 1.0e-12);
    let fitting = viewport::project(
        MenuViewportSource {
            content_height_meters: 0.5,
            ..source.clone()
        },
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    assert_eq!(fitting.maximum_offset_meters, 0.0);
    assert_eq!(fitting.previous_bounds.height, 0.0);
    assert_eq!(fitting.page_progress, 0.0);
    assert_eq!(
        viewport::project(
            MenuViewportSource {
                progress: 1.1,
                ..source
            },
            vec![],
            vec![],
            vec![]
        ),
        Err(MenuLayoutFailure::InvalidLayout)
    );
}
