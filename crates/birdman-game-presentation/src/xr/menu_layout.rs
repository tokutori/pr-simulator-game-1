use crate::game_ui::{
    GameUiButtonPresentation, GameUiChartLegend, GameUiChartText, GameUiSection,
    GameUiSectionLayout,
};
use std::collections::{BTreeMap, BTreeSet};
mod types;
pub use types::*;
mod preparation;
mod text;
pub use text::*;
mod viewport;

/// Resolves a complete layout phase using engine-independent metric observations.
pub fn menu_layout(query: MenuLayoutQuery) -> MenuLayoutReply {
    match query {
        MenuLayoutQuery::Prepare {
            source,
            viewer,
            menu_from_panel,
            observation,
        } => preparation::prepare(*source, &viewer, menu_from_panel, observation.as_ref()),
        MenuLayoutQuery::Viewport {
            source,
            rectangles,
            panel_points,
            document_points,
        } => match viewport::project(source, rectangles, panel_points, document_points) {
            Ok(projection) => MenuLayoutReply::Viewport { projection },
            Err(reason) => MenuLayoutReply::Unavailable { reason },
        },
    }
}

fn plan(source: MenuLayoutSource) -> Result<MenuLayoutPlan, MenuLayoutFailure> {
    use MenuLayoutFailure::InvalidLayout;
    let size = source.surface_size;
    let font = source.font_meters;
    if ![size.width, size.height, font]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
        || source.controls.len() > 256
        || source.font.family.trim().is_empty()
        || source.locale.trim().is_empty()
        || !(1..=1000).contains(&source.font.weight)
        || source.font.generation > 9_007_199_254_740_991
        || source.previous_label.trim().is_empty()
        || source.next_label.trim().is_empty()
        || source
            .controls
            .iter()
            .map(|control| &control.id)
            .collect::<BTreeSet<_>>()
            .len()
            != source.controls.len()
    {
        return Err(InvalidLayout);
    }
    let padding = font * 0.5;
    let gap = font * 0.35;
    let width = size.width - padding * 2.0;
    let page_width = (width - gap) / 2.0 - padding * 2.0;
    if width <= padding * 2.0 || page_width <= 0.0 {
        return Err(InvalidLayout);
    }
    let fallback = vec![GameUiSection {
        id: String::new(),
        label: String::new(),
        layout: GameUiSectionLayout::Column,
    }];
    let sections = source.sections.as_ref().unwrap_or(&fallback);
    if sections.len() > 256
        || sections
            .iter()
            .map(|section| &section.id)
            .collect::<BTreeSet<_>>()
            .len()
            != sections.len()
        || source.sections.is_some()
            && source.controls.iter().any(|control| {
                !sections
                    .iter()
                    .any(|section| Some(&section.id) == control.section.as_ref())
            })
    {
        return Err(InvalidLayout);
    }
    let mut rows = Vec::new();
    for section in sections {
        let controls = source
            .controls
            .iter()
            .enumerate()
            .filter(|(_, control)| {
                source.sections.is_none() || control.section.as_ref() == Some(&section.id)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let limit = if size.width < size.height {
            1
        } else {
            ((width / (font * 8.0)).floor() as usize).max(1)
        };
        let columns = match section.layout {
            GameUiSectionLayout::Column => 1,
            GameUiSectionLayout::Row => limit.min(controls.len().max(1)),
            GameUiSectionLayout::ChoiceGrid => limit.min(3),
        };
        for (row, controls) in controls.chunks(columns).enumerate() {
            rows.push(MenuRow {
                heading: (row == 0).then(|| MenuHeading {
                    id: section.id.clone(),
                    label: section.label.clone(),
                }),
                controls: controls
                    .iter()
                    .enumerate()
                    .map(|(column, index)| MenuRowItem {
                        index: *index,
                        left: column as f64 * width / columns as f64,
                        width: width / columns as f64,
                    })
                    .collect(),
            });
        }
    }
    let mut output = MenuLayoutPlan {
        source,
        line_height_meters: font * 1.35,
        padding_meters: padding,
        gap_meters: gap,
        rows,
        texts: Vec::new(),
    };
    for (role, value, width) in [
        (MenuFixedRole::Title, output.source.title.clone(), width),
        (MenuFixedRole::Caption, output.source.caption.clone(), width),
        (
            MenuFixedRole::Previous,
            output.source.previous_label.clone(),
            page_width,
        ),
        (
            MenuFixedRole::Next,
            output.source.next_label.clone(),
            page_width,
        ),
    ] {
        output.add_text(MenuTextSlot::Fixed { role }, value, width, None, None, None)?;
    }
    for row in output.rows.clone() {
        if let Some(heading) = row.heading.filter(|heading| !heading.label.is_empty()) {
            output.add_text(
                MenuTextSlot::Section {
                    role: MenuHeadingRole::Heading,
                    section_id: heading.id,
                },
                heading.label,
                width,
                None,
                None,
                None,
            )?;
        }
        for item in row.controls {
            let control = output.source.controls[item.index].clone();
            let text_width = item.width - padding * 2.0;
            let label = if let MenuWidget::Button { presentation } = &control.widget {
                match presentation {
                    GameUiButtonPresentation::Choice {
                        selected,
                        description,
                        ..
                    } => format!(
                        "{} {}{}",
                        if *selected { "●" } else { "○" },
                        control.label,
                        if description.is_empty() {
                            String::new()
                        } else {
                            format!("\n{description}")
                        }
                    ),
                    GameUiButtonPresentation::Disclosure { expanded } => {
                        format!("{} {}", if *expanded { "▾" } else { "▸" }, control.label)
                    }
                    _ => control.label.clone(),
                }
            } else {
                control.label.clone()
            };
            let slot = |role, index| MenuTextSlot::Control {
                control_id: control.id.clone(),
                role,
                index,
            };
            output.add_text(
                slot(MenuTextRole::Label, 0),
                label,
                text_width,
                None,
                None,
                None,
            )?;
            match control.widget {
                MenuWidget::Button { .. } => {}
                MenuWidget::Toggle { value } => output.add_text(
                    slot(MenuTextRole::Value, 0),
                    if value { "On" } else { "Off" }.into(),
                    text_width,
                    None,
                    None,
                    None,
                )?,
                MenuWidget::Range {
                    display_value: value,
                }
                | MenuWidget::Status { value } => output.add_text(
                    slot(MenuTextRole::Value, 0),
                    value,
                    text_width,
                    None,
                    None,
                    None,
                )?,
                MenuWidget::Chart { layout } => {
                    if ![layout.size[0], layout.size[1], layout.tick_font_size]
                        .into_iter()
                        .all(|value| value.is_finite() && value > 0.0)
                        || !layout.plot.into_iter().all(f64::is_finite)
                    {
                        return Err(InvalidLayout);
                    }
                    for label in &layout.labels {
                        let divisor =
                            if matches!(label.role, MenuTextRole::XTick | MenuTextRole::YTick) {
                                layout
                                    .labels
                                    .iter()
                                    .filter(|candidate| candidate.role == label.role)
                                    .count() as f64
                            } else {
                                1.0
                            };
                        output.add_text(
                            slot(label.role, label.index),
                            label.text.label.clone(),
                            text_width / divisor,
                            Some(label.text.clone()),
                            None,
                            Some(layout.tick_font_size),
                        )?;
                    }
                    for (index, legend) in layout.legend.into_iter().enumerate() {
                        output.add_text(
                            slot(MenuTextRole::Legend, index),
                            legend.label.clone(),
                            text_width - font * 2.0,
                            None,
                            Some(legend),
                            None,
                        )?;
                    }
                }
            }
        }
    }
    if output
        .texts
        .iter()
        .map(|text| &text.identity)
        .collect::<BTreeSet<_>>()
        .len()
        != output.texts.len()
    {
        return Err(InvalidLayout);
    }
    Ok(output)
}

impl MenuLayoutPlan {
    fn add_text(
        &mut self,
        slot: MenuTextSlot,
        value: String,
        width: f64,
        chart_text: Option<GameUiChartText>,
        legend: Option<GameUiChartLegend>,
        chart_font: Option<f64>,
    ) -> Result<(), MenuLayoutFailure> {
        if !width.is_finite() || width <= 0.0 || value.len() > 65_536 || self.texts.len() >= 4096 {
            return Err(MenuLayoutFailure::InvalidLayout);
        }
        let mut font = self.source.font.clone();
        let mut font_meters = self.source.font_meters;
        if let Some(text) = &chart_text {
            if ![
                text.font_size,
                text.position.x,
                text.position.y,
                text.rotation_degrees,
            ]
            .into_iter()
            .all(f64::is_finite)
                || text.font_size <= 0.0
                || !(1..=1000).contains(&text.font_weight)
            {
                return Err(MenuLayoutFailure::InvalidLayout);
            }
            font.weight = text.font_weight;
            font_meters *= text.font_size / chart_font.ok_or(MenuLayoutFailure::InvalidLayout)?;
        }
        if !font_meters.is_finite() || font_meters <= 0.0 {
            return Err(MenuLayoutFailure::InvalidLayout);
        }
        let identity = serde_json::to_string(&(
            &self.source.panel_id,
            self.source.surface_size,
            &self.source.locale,
            self.source.font_meters,
            font_meters,
            &font,
            &slot,
            &value,
            width,
            &chart_text,
            &legend,
        ))
        .map_err(|_| MenuLayoutFailure::InvalidLayout)?;
        self.texts.push(MenuTextRequest {
            identity,
            slot,
            value,
            locale: self.source.locale.clone(),
            font_meters,
            font,
            width_meters: width,
            chart_text,
            legend,
        });
        Ok(())
    }
}

fn arrange(
    plan: &MenuLayoutPlan,
    measurements: &[MenuTextMeasurement],
) -> Result<MenuDocumentGeometry, MenuLayoutFailure> {
    use MenuLayoutFailure::*;
    let expected = plan
        .texts
        .iter()
        .enumerate()
        .map(|(index, text)| (text.identity.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut measured = BTreeMap::new();
    for measurement in measurements {
        let Some(index) = expected.get(measurement.identity.as_str()) else {
            return Err(StaleTextMetrics);
        };
        if measured.contains_key(index) {
            return Err(StaleTextMetrics);
        }
        if !valid_measurement(&plan.texts[*index], measurement, plan.line_height_meters) {
            return Err(InvalidTextMetrics);
        }
        measured.insert(*index, measurement);
    }
    if measured.len() != expected.len() {
        return Err(MissingTextMetrics);
    }
    let fixed = |role| {
        plan.texts
            .iter()
            .position(|text| text.slot == MenuTextSlot::Fixed { role })
            .ok_or(InvalidLayout)
    };
    let place = |index: usize, coordinate_x: f64, coordinate_y: f64| {
        let text = &plan.texts[index];
        MenuTextPlacement {
            text_index: index,
            bounds: MenuRect {
                x: coordinate_x,
                y: coordinate_y,
                width: text.width_meters,
                height: if text.value.is_empty() {
                    0.0
                } else {
                    measured[&index].lines.len() as f64 * plan.line_height_meters
                },
            },
            chart_projection: None,
            line_geometry: Vec::new(),
        }
    };
    let title = place(fixed(MenuFixedRole::Title)?, 0.0, 0.0);
    let mut top = title.bounds.height + plan.gap_meters;
    let mut controls = Vec::new();
    let mut section_headings = Vec::new();
    for row in &plan.rows {
        if let Some(heading) = &row.heading
            && !heading.label.is_empty()
        {
            let index = plan.texts.iter().position(|text| matches!(&text.slot, MenuTextSlot::Section { section_id, .. } if section_id == &heading.id)).ok_or(InvalidLayout)?;
            let placed = place(index, 0.0, top);
            top += placed.bounds.height + plan.gap_meters;
            section_headings.push(placed);
        }
        let mut height = 0.0_f64;
        for item in &row.controls {
            let control = plan.source.controls.get(item.index).ok_or(InvalidLayout)?;
            let specifications = plan.texts.iter().enumerate().filter(|(_, text)| matches!(&text.slot, MenuTextSlot::Control { control_id, .. } if control_id == &control.id)).collect::<Vec<_>>();
            let mut texts = Vec::new();
            let mut text_top = top + plan.padding_meters;
            for (index, _) in specifications.iter().filter(|(_, text)| {
                matches!(
                    text.slot,
                    MenuTextSlot::Control {
                        role: MenuTextRole::Label | MenuTextRole::Value,
                        ..
                    }
                )
            }) {
                let text = place(*index, item.left + plan.padding_meters, text_top);
                text_top += text.bounds.height;
                texts.push(text);
            }
            let mut plot = None;
            if let MenuWidget::Chart { layout } = &control.widget {
                let scale = (item.width - plan.padding_meters * 2.0) / layout.size[0];
                let [left, chart_top, right, bottom] = layout.plot;
                plot = Some(MenuRect {
                    x: item.left + plan.padding_meters + left * scale,
                    y: text_top + chart_top * scale,
                    width: (right - left) * scale,
                    height: (bottom - chart_top) * scale,
                });
                for (index, specification) in &specifications {
                    if let Some(source) = &specification.chart_text {
                        let projection = MenuChartProjection {
                            baseline: MenuPoint {
                                x: item.left + plan.padding_meters + source.position.x * scale,
                                y: text_top + source.position.y * scale,
                            },
                            rotation_degrees: source.rotation_degrees,
                            anchor: source.anchor,
                        };
                        let line_geometry = measured[index]
                            .lines
                            .iter()
                            .enumerate()
                            .map(|(line_index, line)| {
                                chart_line(&projection, line, line_index, plan.line_height_meters)
                            })
                            .collect::<Vec<_>>();
                        let bounds = union(line_geometry.iter().map(|geometry| geometry.bounds))
                            .ok_or(InvalidLayout)?;
                        texts.push(MenuTextPlacement {
                            text_index: *index,
                            bounds,
                            chart_projection: Some(projection),
                            line_geometry,
                        });
                    }
                }
                text_top += layout.size[1] * scale + plan.gap_meters;
                for (index, _) in specifications
                    .iter()
                    .filter(|(_, text)| text.legend.is_some())
                {
                    let text = place(
                        *index,
                        item.left + plan.padding_meters + plan.source.font_meters * 2.0,
                        text_top,
                    );
                    text_top += text.bounds.height;
                    texts.push(text);
                }
            }
            let bounds = MenuRect {
                x: item.left,
                y: top,
                width: item.width,
                height: text_top - top + plan.padding_meters,
            };
            if !finite_rect(bounds)
                || plot.is_some_and(|plot| !finite_rect(plot))
                || texts.iter().any(|text| {
                    ![
                        text.bounds.x,
                        text.bounds.y,
                        text.bounds.width,
                        text.bounds.height,
                    ]
                    .into_iter()
                    .all(f64::is_finite)
                })
            {
                return Err(InvalidLayout);
            }
            height = height.max(bounds.height);
            controls.push(MenuControlPlacement {
                control_index: item.index,
                bounds,
                texts,
                plot,
            });
        }
        top += height + plan.gap_meters;
    }
    let caption = place(fixed(MenuFixedRole::Caption)?, 0.0, top);
    let previous_index = fixed(MenuFixedRole::Previous)?;
    let next_index = fixed(MenuFixedRole::Next)?;
    let footer_height_meters = measured[&previous_index]
        .lines
        .len()
        .max(measured[&next_index].lines.len()) as f64
        * plan.line_height_meters
        + plan.padding_meters * 2.0;
    let content_height_meters = top + caption.bounds.height;
    if !content_height_meters.is_finite()
        || !footer_height_meters.is_finite()
        || plan.source.surface_size.height
            - footer_height_meters
            - plan.padding_meters * 2.0
            - plan.gap_meters
            <= 0.0
    {
        return Err(InvalidLayout);
    }
    Ok(MenuDocumentGeometry {
        title,
        caption,
        section_headings,
        controls,
        content_height_meters,
        footer_height_meters,
        previous_index,
        next_index,
    })
}

fn valid_measurement(
    request: &MenuTextRequest,
    measurement: &MenuTextMeasurement,
    line_height: f64,
) -> bool {
    let paragraphs = request.value.split('\n').collect::<Vec<_>>();
    let mut reconstructed = Vec::<String>::new();
    if measurement.lines.len() > 4096 {
        return false;
    }
    for line in &measurement.lines {
        if line.paragraph_index >= paragraphs.len()
            || line.paragraph_index > reconstructed.len()
            || line.paragraph_index + 1 < reconstructed.len()
            || line.value.contains('\n')
            || line.value.is_empty()
                && (!paragraphs[line.paragraph_index].is_empty()
                    || line.paragraph_index < reconstructed.len())
        {
            return false;
        }
        if line.paragraph_index == reconstructed.len() {
            reconstructed.push(String::new());
        }
        reconstructed[line.paragraph_index].push_str(&line.value);
        if ![
            line.advance_meters,
            line.left_meters,
            line.right_meters,
            line.ascent_meters,
            line.descent_meters,
        ]
        .into_iter()
        .all(f64::is_finite)
            || line.advance_meters < 0.0
            || line.left_meters + line.right_meters < 0.0
            || line
                .advance_meters
                .max(line.left_meters.max(0.0) + line.right_meters)
                > request.width_meters + 1.0e-10
            || line.ascent_meters + line.descent_meters > line_height
            || if line.value.trim().is_empty() {
                line.ascent_meters + line.descent_meters < 0.0
            } else {
                line.ascent_meters + line.descent_meters <= 0.0
            }
        {
            return false;
        }
    }
    reconstructed.len() == paragraphs.len()
        && reconstructed
            .iter()
            .zip(paragraphs)
            .all(|(actual, expected)| actual == expected)
}

fn chart_line(
    projection: &MenuChartProjection,
    line: &MenuMeasuredLine,
    index: usize,
    height: f64,
) -> MenuLineGeometry {
    use crate::game_ui::GameUiChartTextAnchor;
    let shift = match projection.anchor {
        GameUiChartTextAnchor::Start => 0.0,
        GameUiChartTextAnchor::Middle => -line.advance_meters / 2.0,
        GameUiChartTextAnchor::End => -line.advance_meters,
    };
    let rotation = projection.rotation_degrees.to_radians();
    let project = |coordinate_x: f64, coordinate_y: f64| MenuPoint {
        x: projection.baseline.x + coordinate_x * rotation.cos() - coordinate_y * rotation.sin(),
        y: projection.baseline.y + coordinate_x * rotation.sin() + coordinate_y * rotation.cos(),
    };
    let corners = [shift - line.left_meters, shift + line.right_meters]
        .into_iter()
        .flat_map(|coordinate_x| {
            [
                index as f64 * height - line.ascent_meters,
                index as f64 * height + line.descent_meters,
            ]
            .into_iter()
            .map(move |coordinate_y| project(coordinate_x, coordinate_y))
        })
        .collect::<Vec<_>>();
    let left = corners
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let top = corners
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    MenuLineGeometry {
        anchor: project(shift, index as f64 * height),
        bounds: MenuRect {
            x: left,
            y: top,
            width: corners
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max)
                - left,
            height: corners
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max)
                - top,
        },
    }
}

fn union(rectangles: impl Iterator<Item = MenuRect>) -> Option<MenuRect> {
    let rectangles = rectangles.collect::<Vec<_>>();
    if rectangles.is_empty() {
        return None;
    }
    let left = rectangles
        .iter()
        .map(|rect| rect.x)
        .fold(f64::INFINITY, f64::min);
    let top = rectangles
        .iter()
        .map(|rect| rect.y)
        .fold(f64::INFINITY, f64::min);
    Some(MenuRect {
        x: left,
        y: top,
        width: rectangles
            .iter()
            .map(|rect| rect.x + rect.width)
            .fold(f64::NEG_INFINITY, f64::max)
            - left,
        height: rectangles
            .iter()
            .map(|rect| rect.y + rect.height)
            .fold(f64::NEG_INFINITY, f64::max)
            - top,
    })
}

fn finite_rect(rect: MenuRect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f64::is_finite)
        && rect.width > 0.0
        && rect.height > 0.0
}

#[cfg(test)]
mod tests;
