use super::*;
use crate::camera::{Pose, Vector3, compose_pose};
use crate::game_ui::GameUiReadoutSource;
use crate::game_ui::{GameUiAnchor, GameUiControlContent, GameUiFrame, GameUiPanel, GameUiView};
use crate::xr::{
    XrFitReason, XrInkFit, XrInkSegment, XrMenuViewFit, XrViewerFrame, fit_xr_menu_view,
    fit_xr_text_ink,
};

/// Bounded font observations owned with the platform UI resource.
#[derive(Debug, Default)]
pub struct SurfaceFontCache {
    entries: Vec<Vec<MenuTextMeasurement>>,
}

impl SurfaceFontCache {
    pub(crate) fn measure(
        &mut self,
        requests: &[MenuTextRequest],
        size: MenuSize,
        measure: &mut impl FnMut(
            &MenuTextRequest,
            MenuSize,
            &str,
        ) -> Result<MenuGlyphMetrics, MenuLayoutFailure>,
    ) -> Result<Vec<MenuTextMeasurement>, MenuLayoutFailure> {
        if let Some(cached) = self.entries.iter().find(|entry| {
            entry.len() == requests.len()
                && entry
                    .iter()
                    .zip(requests)
                    .all(|(metric, text)| metric.identity == text.identity)
        }) {
            return Ok(cached.clone());
        }
        let measured = measure_menu_text(requests, |index, value| {
            measure(&requests[index], size, value)
        })?;
        if self.entries.len() >= 32 {
            self.entries.remove(0);
        }
        self.entries.push(measured.clone());
        Ok(measured)
    }
}

/// Prepares the published shared UI with current frame values and platform font observations.
pub fn prepare_game_ui_menu(
    view: &GameUiView,
    frame: &GameUiFrame,
    input: MenuPreparationInput<'_>,
    cache: &mut SurfaceFontCache,
    measure: impl FnMut(&MenuTextRequest, MenuSize, &str) -> Result<MenuGlyphMetrics, MenuLayoutFailure>,
) -> MenuLayoutReply {
    let result = (|| {
        if input.view_revision != view.view_revision || frame.view_revision != view.view_revision {
            return Err(MenuLayoutFailure::StaleView);
        }
        let panel = view
            .panels
            .iter()
            .find(|panel| panel.id == input.panel_id && panel.anchor == GameUiAnchor::Menu)
            .ok_or(MenuLayoutFailure::StaleView)?;
        let source = source_from_view(panel, &view.description, frame, &input);
        let menu_from_panel = Pose {
            position: Vector3 {
                x: panel.local_position[0],
                y: panel.local_position[1],
                z: panel.local_position[2],
            },
            ..Pose::IDENTITY
        };
        let (plan, measurements, geometry) =
            select(source, input.viewer, menu_from_panel, cache, measure)?;
        Ok(MenuLayoutReply::Ready {
            projection: Box::new(super::projection::project(
                view,
                frame,
                panel,
                &input,
                &plan,
                &measurements,
                &geometry,
            )?),
        })
    })();
    result.unwrap_or_else(|reason| MenuLayoutReply::Unavailable { reason })
}

fn source_from_view(
    panel: &GameUiPanel,
    caption: &str,
    frame: &GameUiFrame,
    input: &MenuPreparationInput<'_>,
) -> MenuLayoutSource {
    let controls = panel
        .controls
        .iter()
        .map(|control| {
            let widget = match &control.content {
                GameUiControlContent::Button { presentation, .. } => MenuWidget::Button {
                    presentation: presentation.clone(),
                },
                GameUiControlContent::Toggle { value, .. } => MenuWidget::Toggle { value: *value },
                GameUiControlContent::Range { display_value, .. } => MenuWidget::Range {
                    display_value: frame
                        .ranges
                        .iter()
                        .find(|value| value.id == control.id)
                        .map_or(display_value, |value| &value.display_value)
                        .clone(),
                },
                GameUiControlContent::Status { value, .. } => MenuWidget::Status {
                    value: frame
                        .readouts
                        .iter()
                        .find(|readout| readout.id == control.id)
                        .map_or(value, |readout| &readout.value)
                        .clone(),
                },
                GameUiControlContent::Fields { fields, .. } => MenuWidget::Fields {
                    fields: frame
                        .fields
                        .iter()
                        .find(|value| value.id == control.id)
                        .map_or(fields, |value| &value.fields)
                        .clone(),
                },
                GameUiControlContent::Chart { plot, .. } => {
                    let chart = &plot.presentation;
                    let mut labels = vec![
                        MenuChartLabel {
                            role: MenuTextRole::XAxis,
                            index: 0,
                            text: chart.x_axis.clone(),
                        },
                        MenuChartLabel {
                            role: MenuTextRole::YAxis,
                            index: 0,
                            text: chart.y_axis.clone(),
                        },
                    ];
                    if let Some(guides) = &chart.map_guides {
                        labels.push(MenuChartLabel {
                            role: MenuTextRole::North,
                            index: 0,
                            text: guides.north_label.clone(),
                        });
                        labels.push(MenuChartLabel {
                            role: MenuTextRole::Scale,
                            index: 0,
                            text: guides.scale_label.clone(),
                        });
                    }
                    labels.extend(chart.x_ticks.iter().enumerate().map(|(index, tick)| {
                        MenuChartLabel {
                            role: MenuTextRole::XTick,
                            index,
                            text: tick.text.clone(),
                        }
                    }));
                    labels.extend(chart.y_ticks.iter().enumerate().map(|(index, tick)| {
                        MenuChartLabel {
                            role: MenuTextRole::YTick,
                            index,
                            text: tick.text.clone(),
                        }
                    }));
                    labels.extend(
                        chart
                            .marker_labels
                            .iter()
                            .chain(&chart.time_marker_labels)
                            .chain(&chart.reference_labels)
                            .enumerate()
                            .map(|(index, text)| MenuChartLabel {
                                role: MenuTextRole::Annotation,
                                index,
                                text: text.clone(),
                            }),
                    );
                    MenuWidget::Chart {
                        layout: MenuChartLayout {
                            size: [chart.viewport.width, chart.viewport.height],
                            plot: chart.plot,
                            tick_font_size: chart.tick_font_size,
                            labels,
                            legend: chart
                                .legend
                                .iter()
                                .cloned()
                                .map(|mut legend| {
                                    if legend.source == GameUiReadoutSource::AnalysisCursor
                                        && let Some(vector) = frame
                                            .chart_cursor
                                            .as_ref()
                                            .and_then(|cursor| cursor.selected_wind_vector.as_ref())
                                    {
                                        legend.label.clone_from(&vector.label);
                                    }
                                    legend
                                })
                                .collect(),
                        },
                    }
                }
            };
            MenuLayoutControl {
                id: control.id.clone(),
                section: Some(control.section.clone()),
                label: frame
                    .button_labels
                    .iter()
                    .find(|button| button.id == control.id)
                    .map_or(&control.label, |button| &button.label)
                    .clone(),
                widget,
            }
        })
        .collect();
    MenuLayoutSource {
        panel_id: panel.id.clone(),
        title: panel.title.clone(),
        sections: Some(panel.sections.clone()),
        controls,
        surface_size: MenuSize {
            width: panel.size[0],
            height: panel.size[1],
        },
        font_meters: 0.035,
        font: input.font.clone(),
        locale: input.locale.to_owned(),
        caption: caption.into(),
        previous_label: "前頁".into(),
        next_label: "次頁".into(),
    }
}

pub(super) fn select(
    mut source: MenuLayoutSource,
    viewer: &XrViewerFrame,
    menu_from_panel: Pose,
    cache: &mut SurfaceFontCache,
    mut measure: impl FnMut(
        &MenuTextRequest,
        MenuSize,
        &str,
    ) -> Result<MenuGlyphMetrics, MenuLayoutFailure>,
) -> Result<
    (
        MenuLayoutPlan,
        Vec<MenuTextMeasurement>,
        MenuDocumentGeometry,
    ),
    MenuLayoutFailure,
> {
    let fit = |size: MenuSize| {
        fit_xr_menu_view(
            viewer,
            [size.width, size.height],
            menu_from_panel,
            2.4,
            0.035,
        )
        .map_err(|_| MenuLayoutFailure::InvalidLayout)
    };
    let mut fitted = fit(source.surface_size)?;
    for _ in 0..12 {
        match fitted {
            XrMenuViewFit::Unavailable { reason } => return Err(fit_reason(reason)),
            XrMenuViewFit::Ready {
                distance_meters, ..
            } if distance_meters > 2.6 && source.surface_size.width > 0.35 => {
                source.surface_size.width = (source.surface_size.width * 0.85).max(0.35);
                fitted = fit(source.surface_size)?;
            }
            _ => break,
        }
    }
    let head_from_menu = match fitted {
        XrMenuViewFit::Ready { head_from_menu, .. } => head_from_menu,
        XrMenuViewFit::Unavailable { reason } => return Err(fit_reason(reason)),
    };
    let head_from_surface = compose_pose(head_from_menu, menu_from_panel)
        .map_err(|_| MenuLayoutFailure::InvalidViewGeometry)?;
    if source.surface_size.height < 0.6 {
        source.title.clear();
        source.caption.clear();
    }
    for attempt in 0..12 {
        let planned = plan(source)?;
        let measurements =
            cache.measure(&planned.texts, planned.source.surface_size, &mut measure)?;
        let geometry = arrange(&planned, &measurements)?;
        match readability(
            viewer,
            planned.source.surface_size,
            &measurements,
            head_from_surface,
        ) {
            XrInkFit::Ready { .. } => return Ok((planned, measurements, geometry)),
            XrInkFit::Unavailable { reason } => return Err(fit_reason(reason)),
            XrInkFit::Empty | XrInkFit::TooSmall if attempt + 1 < 12 => {
                source = planned.source;
                source.font_meters *= 1.15;
            }
            _ => return Err(MenuLayoutFailure::InsufficientInkAngle),
        }
    }
    Err(MenuLayoutFailure::InsufficientInkAngle)
}

fn readability(
    viewer: &XrViewerFrame,
    size: MenuSize,
    measurements: &[MenuTextMeasurement],
    head_from_surface: Pose,
) -> XrInkFit {
    let mut segments = Vec::new();
    for line in measurements
        .iter()
        .flat_map(|measurement| &measurement.lines)
        .filter(|line| !line.value.trim().is_empty())
    {
        let height = line.ascent_meters + line.descent_meters;
        if height <= 0.0 || height > size.height {
            return XrInkFit::TooSmall;
        }
        for horizontal in [-1.0, 1.0] {
            for vertical in [-1.0, 1.0] {
                segments.push(XrInkSegment {
                    start: Vector3 {
                        x: horizontal * size.width / 2.0,
                        y: vertical * size.height / 2.0,
                        z: 0.0,
                    },
                    end: Vector3 {
                        x: horizontal * size.width / 2.0,
                        y: vertical * (size.height / 2.0 - height),
                        z: 0.0,
                    },
                });
            }
        }
    }
    fit_xr_text_ink(viewer, &segments, head_from_surface, 0.35)
}

fn fit_reason(reason: XrFitReason) -> MenuLayoutFailure {
    match reason {
        XrFitReason::NotStereo => MenuLayoutFailure::NotStereo,
        XrFitReason::ViewerUnavailable => MenuLayoutFailure::ViewerUnavailable,
        XrFitReason::UnsupportedViewConfiguration => {
            MenuLayoutFailure::UnsupportedViewConfiguration
        }
        XrFitReason::InvalidViewGeometry => MenuLayoutFailure::InvalidViewGeometry,
        XrFitReason::InsufficientViewArea => MenuLayoutFailure::InsufficientViewArea,
    }
}
