use super::*;
use crate::game_ui::{
    GameUiChart, GameUiControlContent, GameUiEmphasis, GameUiFrame, GameUiPanel, GameUiTheme,
    GameUiView,
};
use crate::xr::{
    XrPointerControl, XrPointerRect, XrPointerRegion, XrPointerSurface, XrPointerTarget,
};
use serde::{Deserialize, Serialize};

/// Prepared painting and input geometry from one published UI and font observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuProjection {
    /// Fitted surface dimensions in metres.
    pub size: MenuSize,
    /// Published panel title.
    pub title: String,
    /// Full-surface background colour.
    pub background: String,
    /// Background opacity.
    pub opacity: f64,
    /// Clipped document painting.
    pub content: MenuPaintLayer,
    /// Fixed page-control painting.
    pub footer: Vec<SurfacePaintItem>,
    /// Enabled targets in the same surface coordinates as the painting.
    pub surface: XrPointerSurface,
    /// Normalized increment for one visible page.
    pub page_progress: f64,
    /// Current scroll offset in metres.
    pub offset_meters: f64,
    /// Maximum scroll offset in metres.
    pub maximum_offset_meters: f64,
    /// Unscrolled layout identity for input-generation invalidation.
    pub geometry_key: MenuGeometryIdentity,
}

/// Unscrolled input geometry used to invalidate a changed menu layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuGeometryIdentity {
    /// Fitted surface size.
    pub size: MenuSize,
    /// Content and previous/next page clipping rectangles.
    pub clips: [MenuRect; 3],
    /// Maximum scroll offset.
    pub maximum_offset_meters: f64,
    /// Heading and caption bounds.
    pub headings: [MenuRect; 2],
    /// Published controls and unscrolled hit geometry.
    pub controls: Vec<MenuControlGeometry>,
}

/// Geometry and scale of one published control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuControlGeometry {
    /// Published control identity and widget kind.
    pub identity: [String; 2],
    /// Complete document rectangle.
    pub bounds: MenuRect,
    /// Optional chart plot rectangle.
    pub plot: Option<MenuRect>,
    /// Optional range minimum, maximum and step.
    pub range: Option<[f64; 3]>,
}

/// Clipped spatial UI paint layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MenuPaintLayer {
    /// Surface-space clipping bounds in metres.
    pub clip: MenuRect,
    /// Ordered paint operations.
    pub items: Vec<SurfacePaintItem>,
}

/// Engine-independent paint operation in surface coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SurfacePaintItem {
    /// Filled rectangle or horizontal colour gradient.
    Rectangle {
        /// Surface bounds in metres.
        bounds: MenuRect,
        /// Fill colour.
        color: String,
        /// Fill opacity.
        opacity: f64,
        /// Optional left-to-right gradient.
        gradient: Option<[String; 2]>,
    },
    /// Measured line with its resolved alphabetic baseline.
    Text {
        /// Visible line content.
        value: String,
        /// Observed font resource.
        font: MenuFont,
        /// Font size in metres.
        font_meters: f64,
        /// Text-shaping locale.
        locale: String,
        /// Alphabetic baseline in surface metres.
        anchor: MenuPoint,
        /// Rotation about the baseline.
        rotation_degrees: f64,
        /// Text colour.
        color: String,
        /// Text opacity.
        opacity: f64,
    },
    /// Chart or guide segment.
    Line {
        /// Segment start in metres.
        start: MenuPoint,
        /// Segment end in metres.
        end: MenuPoint,
        /// Stroke colour.
        color: String,
        /// Stroke width in texture pixels.
        width_px: f64,
        /// Stroke opacity.
        opacity: f64,
    },
    /// Screen-sized sample marker.
    Marker {
        /// Marker centre in surface metres.
        point: MenuPoint,
        /// Square side in texture pixels.
        size_px: f64,
        /// Fill colour.
        color: String,
    },
    /// Nested clipping for plot primitives.
    Plot {
        /// Plot clipping rectangle in surface metres.
        clip: MenuRect,
        /// Ordered plot operations.
        items: Vec<SurfacePaintItem>,
    },
}

pub(super) fn project(
    view: &GameUiView,
    frame: &GameUiFrame,
    panel: &GameUiPanel,
    input: &MenuPreparationInput,
    plan: &MenuLayoutPlan,
    measurements: &[MenuTextMeasurement],
    geometry: &MenuDocumentGeometry,
) -> Result<MenuProjection, MenuLayoutFailure> {
    let viewport = viewport::project(
        MenuViewportSource {
            surface_size: plan.source.surface_size,
            padding_meters: plan.padding_meters,
            gap_meters: plan.gap_meters,
            content_height_meters: geometry.content_height_meters,
            footer_height_meters: geometry.footer_height_meters,
            progress: input.progress,
        },
        geometry
            .controls
            .iter()
            .map(|control| control.bounds)
            .collect(),
    )?;
    let translate = |point: MenuPoint| MenuPoint {
        x: point.x + viewport.content_clip.x,
        y: point.y + viewport.content_clip.y - viewport.offset_meters,
    };
    let mut content = Vec::new();
    let mut regions = Vec::new();
    let mut footer = Vec::new();
    let theme = &view.theme;
    for (text, color) in [
        (&geometry.title, &theme.text),
        (&geometry.caption, &theme.muted_text),
    ]
    .into_iter()
    .chain(
        geometry
            .section_headings
            .iter()
            .map(|text| (text, &theme.text)),
    ) {
        paint_text(&mut content, text, plan, measurements, &translate, color)?;
    }
    for (placement, projected) in geometry.controls.iter().zip(&viewport.rectangles) {
        let control = panel
            .controls
            .get(placement.control_index)
            .ok_or(MenuLayoutFailure::StaleView)?;
        if let Some(visible) = projected.visible {
            content.push(SurfacePaintItem::Rectangle {
                opacity: 1.0,
                bounds: visible,
                color: control_background(&control.content, theme).into(),
                gradient: None,
            });
            for card in &placement.cards {
                let origin = translate(MenuPoint {
                    x: card.x,
                    y: card.y,
                });
                content.push(SurfacePaintItem::Rectangle {
                    opacity: 1.0,
                    bounds: MenuRect {
                        x: origin.x,
                        y: origin.y,
                        width: card.width,
                        height: card.height,
                    },
                    color: theme.normal_background.clone(),
                    gradient: None,
                });
            }
            if let (GameUiControlContent::Chart { plot, .. }, Some(bounds)) =
                (&control.content, placement.plot)
            {
                let mut chart = (**plot).clone();
                if let Some(cursor) = frame
                    .chart_cursor
                    .as_ref()
                    .filter(|cursor| cursor.id == control.id)
                {
                    chart.cursor_x = cursor.cursor_x;
                    chart.cursor_points = cursor.cursor_points.clone();
                    if let Some(vector) = &cursor.selected_wind_vector
                        && let Some(first) = chart.vectors.first_mut()
                    {
                        *first = vector.clone();
                    }
                }
                let mut clip = bounds;
                let origin = translate(MenuPoint {
                    x: clip.x,
                    y: clip.y,
                });
                clip.x = origin.x;
                clip.y = origin.y;
                content.push(SurfacePaintItem::Plot {
                    clip,
                    items: paint_chart(&chart, bounds, &translate),
                });
            }
            for text in &placement.texts {
                let request = &plan.texts[text.text_index];
                if let Some(legend) = &request.legend {
                    let origin = translate(MenuPoint {
                        x: text.bounds.x - plan.source.font_meters * 2.0,
                        y: text.bounds.y + plan.line_height_meters * 0.35,
                    });
                    content.push(SurfacePaintItem::Rectangle {
                        opacity: 1.0,
                        bounds: MenuRect {
                            x: origin.x,
                            y: origin.y,
                            width: plan.source.font_meters * 1.5,
                            height: plan.line_height_meters * 0.3,
                        },
                        color: legend.color.clone(),
                        gradient: legend.segment_gradient.clone(),
                    });
                }
                let color = if control.enabled
                    || matches!(
                        control.content,
                        GameUiControlContent::Status { .. }
                            | GameUiControlContent::Fields { .. }
                            | GameUiControlContent::Chart { .. }
                    ) {
                    &theme.text
                } else {
                    &theme.muted_text
                };
                paint_text(&mut content, text, plan, measurements, &translate, color)?;
            }
            if control.enabled {
                let descriptor = match control.content {
                    GameUiControlContent::Toggle { value, .. } => {
                        Some(XrPointerControl::Toggle { value })
                    }
                    GameUiControlContent::Range {
                        minimum,
                        maximum,
                        step,
                        ..
                    } => Some(XrPointerControl::Range {
                        minimum,
                        maximum,
                        step,
                        track: rect(projected.full),
                    }),
                    _ => None,
                };
                regions.push(XrPointerRegion {
                    target: XrPointerTarget::Control {
                        id: control.id.clone(),
                    },
                    bounds: rect(visible),
                    control: descriptor,
                });
            }
        }
    }
    if viewport.maximum_offset_meters > 0.0 {
        let mut pages = Vec::new();
        for (next, bounds, index, enabled) in [
            (
                false,
                viewport.previous_bounds,
                geometry.previous_index,
                viewport.offset_meters > 0.0,
            ),
            (
                true,
                viewport.next_bounds,
                geometry.next_index,
                viewport.offset_meters < viewport.maximum_offset_meters,
            ),
        ] {
            footer.push(SurfacePaintItem::Rectangle {
                opacity: 1.0,
                bounds,
                color: theme.normal_background.clone(),
                gradient: None,
            });
            let measurement = &measurements[index];
            let request = &plan.texts[index];
            let text = MenuTextPlacement {
                text_index: index,
                bounds: MenuRect {
                    x: bounds.x + plan.padding_meters,
                    y: bounds.y + plan.padding_meters,
                    width: request.width_meters,
                    height: measurement.lines.len() as f64 * plan.line_height_meters,
                },
                chart_projection: None,
                line_geometry: vec![],
            };
            paint_text(
                &mut footer,
                &text,
                plan,
                measurements,
                &|point| point,
                if enabled {
                    &theme.text
                } else {
                    &theme.muted_text
                },
            )?;
            if enabled {
                pages.push(XrPointerRegion {
                    target: XrPointerTarget::Page { next },
                    bounds: rect(bounds),
                    control: None,
                });
            }
        }
        pages.append(&mut regions);
        regions = pages;
    }
    let geometry_key = MenuGeometryIdentity {
        size: plan.source.surface_size,
        clips: [
            viewport.content_clip,
            viewport.previous_bounds,
            viewport.next_bounds,
        ],
        maximum_offset_meters: viewport.maximum_offset_meters,
        headings: [geometry.title.bounds, geometry.caption.bounds],
        controls: geometry
            .controls
            .iter()
            .map(|placed| {
                let control = &panel.controls[placed.control_index];
                let (kind, range) = match control.content {
                    GameUiControlContent::Button { .. } => ("button", None),
                    GameUiControlContent::Toggle { .. } => ("toggle", None),
                    GameUiControlContent::Range {
                        minimum,
                        maximum,
                        step,
                        ..
                    } => ("range", Some([minimum, maximum, step])),
                    GameUiControlContent::Status { .. } => ("status", None),
                    GameUiControlContent::Fields { .. } => ("fields", None),
                    GameUiControlContent::Chart { .. } => ("chart", None),
                };
                MenuControlGeometry {
                    identity: [control.id.clone(), kind.into()],
                    bounds: placed.bounds,
                    plot: placed.plot,
                    range,
                }
            })
            .collect(),
    };
    Ok(MenuProjection {
        size: plan.source.surface_size,
        title: plan.source.title.clone(),
        background: theme.panel_background.clone(),
        opacity: theme.panel_opacity,
        content: MenuPaintLayer {
            clip: viewport.content_clip,
            items: content,
        },
        footer,
        surface: XrPointerSurface {
            scope: serde_json::to_string(&(&view.interaction_scope, &panel.id))
                .map_err(|_| MenuLayoutFailure::InvalidLayout)?,
            revision: input.generation,
            size_m: [
                plan.source.surface_size.width,
                plan.source.surface_size.height,
            ],
            regions,
        },
        page_progress: viewport.page_progress,
        offset_meters: viewport.offset_meters,
        maximum_offset_meters: viewport.maximum_offset_meters,
        geometry_key,
    })
}

fn rect(bounds: MenuRect) -> XrPointerRect {
    XrPointerRect([bounds.x, bounds.y, bounds.width, bounds.height])
}

fn control_background<'theme>(
    content: &GameUiControlContent,
    theme: &'theme GameUiTheme,
) -> &'theme str {
    match content {
        GameUiControlContent::Button { presentation, .. } => match presentation {
            GameUiButtonPresentation::Action {
                emphasis: GameUiEmphasis::Primary,
            }
            | GameUiButtonPresentation::Choice { selected: true, .. } => &theme.primary_background,
            GameUiButtonPresentation::Action {
                emphasis: GameUiEmphasis::Secondary,
            }
            | GameUiButtonPresentation::Disclosure { .. } => &theme.panel_background,
            GameUiButtonPresentation::Choice { .. } => &theme.choice_background,
            _ => &theme.normal_background,
        },
        _ => &theme.choice_background,
    }
}

fn paint_text(
    output: &mut Vec<SurfacePaintItem>,
    text: &MenuTextPlacement,
    plan: &MenuLayoutPlan,
    measurements: &[MenuTextMeasurement],
    translate: &impl Fn(MenuPoint) -> MenuPoint,
    color: &str,
) -> Result<(), MenuLayoutFailure> {
    let request = &plan.texts[text.text_index];
    let measured = measurements
        .get(text.text_index)
        .filter(|measurement| measurement.identity == request.identity)
        .ok_or(MenuLayoutFailure::MissingTextMetrics)?;
    for (index, line) in measured.lines.iter().enumerate() {
        let anchor = if text.chart_projection.is_some() {
            text.line_geometry
                .get(index)
                .ok_or(MenuLayoutFailure::MissingTextMetrics)?
                .anchor
        } else {
            MenuPoint {
                x: text.bounds.x + line.left_meters.max(0.0),
                y: text.bounds.y
                    + index as f64 * plan.line_height_meters
                    + plan.line_height_meters / 2.0
                    + (line.ascent_meters - line.descent_meters) / 2.0,
            }
        };
        output.push(SurfacePaintItem::Text {
            opacity: 1.0,
            value: line.value.clone(),
            font: request.font.clone(),
            font_meters: request.font_meters,
            locale: request.locale.clone(),
            anchor: translate(anchor),
            rotation_degrees: text
                .chart_projection
                .as_ref()
                .map_or(0.0, |projection| projection.rotation_degrees),
            color: request
                .chart_text
                .as_ref()
                .map_or_else(|| color.into(), |text| text.color.clone()),
        });
    }
    Ok(())
}

fn paint_chart(
    chart: &GameUiChart,
    plot: MenuRect,
    translate: &impl Fn(MenuPoint) -> MenuPoint,
) -> Vec<SurfacePaintItem> {
    use crate::game_ui::GameUiChartPoint as Point;
    let [left, top, right, bottom] = chart.presentation.plot;
    let project = |point: Point| {
        translate(MenuPoint {
            x: plot.x + (point.x - left) / (right - left) * plot.width,
            y: plot.y + (point.y - top) / (bottom - top) * plot.height,
        })
    };
    let mut items = Vec::new();
    let mut segment = |start, end, color: &str, width_px, opacity| {
        items.push(SurfacePaintItem::Line {
            start: project(start),
            end: project(end),
            color: color.into(),
            width_px,
            opacity,
        })
    };
    for [start, end] in &chart.presentation.grid_lines {
        segment(
            *start,
            *end,
            &chart.presentation.grid_color,
            1.0,
            chart.presentation.grid_opacity,
        );
    }
    segment(
        Point { x: left, y: top },
        Point { x: left, y: bottom },
        &chart.presentation.axes_color,
        1.5,
        1.0,
    );
    segment(
        Point { x: left, y: bottom },
        Point {
            x: right,
            y: bottom,
        },
        &chart.presentation.axes_color,
        1.5,
        1.0,
    );
    for reference in &chart.reference_lines {
        segment(
            Point {
                x: left,
                y: reference.position_y_px,
            },
            Point {
                x: right,
                y: reference.position_y_px,
            },
            &reference.color,
            2.0,
            1.0,
        );
    }
    if let Some(guides) = &chart.presentation.map_guides {
        for [start, end] in guides.scale_segments.iter().chain(&guides.north_segments) {
            segment(*start, *end, "#f3f4e8", 1.5, 1.0);
        }
    }
    for series in &chart.series {
        for (index, pair) in series.points.windows(2).enumerate() {
            segment(
                pair[0],
                pair[1],
                series.segment_colors.get(index).unwrap_or(&series.color),
                3.0,
                1.0,
            );
        }
    }
    for vector in &chart.vectors {
        for [start, end] in &vector.segments {
            segment(*start, *end, &vector.color, 2.0, 1.0);
        }
    }
    for marker in chart.markers.iter().chain(&chart.time_markers) {
        items.push(SurfacePaintItem::Marker {
            point: project(marker.point),
            size_px: 6.0,
            color: marker.color.clone(),
        });
    }
    if let Some(cursor) = chart.cursor_x {
        items.push(SurfacePaintItem::Line {
            start: project(Point {
                x: cursor,
                y: bottom,
            }),
            end: project(Point { x: cursor, y: top }),
            color: "#ffd45c".into(),
            width_px: 2.0,
            opacity: 1.0,
        });
    }
    for cursor in &chart.cursor_points {
        items.push(SurfacePaintItem::Marker {
            point: project(cursor.point),
            size_px: 8.0,
            color: cursor.color.clone(),
        });
    }
    items
}
