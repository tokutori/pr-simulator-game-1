use super::{
    MenuFont, MenuGlyphMetrics, MenuLayoutFailure, MenuMeasuredLine, MenuPoint, MenuRect, MenuSize,
    MenuTextRequest, MenuTextRole, MenuTextSlot, SurfaceFontCache, SurfacePaintItem, XrFitReason,
    XrHeadHudProfile, XrHeadHudProjection, XrInkFit, XrInkSegment, XrViewerFrame, fit_xr_text_ink,
    project_xr_head_hud,
};
use crate::camera::{Pose, Vector3};
use crate::game_ui::{
    GameUiChartPoint, GameUiChartTextAnchor, GameUiFrame, GameUiHudNode, GameUiHudNodeContent,
    GameUiHudTone, GameUiRect, GameUiTheme, GameUiView,
};
use serde::{Deserialize, Serialize};

/// Same-frame observations for the retained head-fixed UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadHudPreparationInput {
    /// Structural UI revision supplied by the owner.
    pub view_revision: u32,
    /// Current stereo observation.
    pub viewer: Box<XrViewerFrame>,
    /// Optical HUD limits.
    pub profile: XrHeadHudProfile,
    /// Platform font resource.
    pub font: MenuFont,
    /// Text-shaping locale.
    pub locale: String,
    /// Optional observed frame-rate label.
    pub frame_rate_label: Option<String>,
}

/// Complete head-fixed painting or its rejection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum HeadHudPaintReply {
    /// No requested instruments or frame-rate label.
    Absent,
    /// Failed optical or font observation.
    Unavailable {
        /// Rejection reason.
        reason: MenuLayoutFailure,
    },
    /// Final engine-independent drawing.
    Ready {
        /// Head anchor and surface-space commands.
        projection: Box<HeadHudPaintProjection>,
    },
}

/// Head-fixed surface whose text ink has passed the binocular readability check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HeadHudPaintProjection {
    /// Head-to-surface pose.
    pub local_pose: Pose,
    /// Physical drawing extent in metres.
    pub size: MenuSize,
    /// Texture allocation required by the drawing observations.
    pub canvas_size: [u32; 2],
    /// Normalized forward-view reservation.
    pub clear_region: GameUiRect,
    /// Ordered drawing commands in surface metres.
    pub items: Vec<SurfacePaintItem>,
}

/// Resolves optics, measured text, instrument geometry and ink in one owner call.
pub fn prepare_game_ui_head_hud(
    view: &GameUiView,
    frame: &GameUiFrame,
    input: HeadHudPreparationInput,
    cache: &mut SurfaceFontCache,
    mut measure: impl FnMut(
        &MenuTextRequest,
        MenuSize,
        &str,
    ) -> Result<MenuGlyphMetrics, MenuLayoutFailure>,
) -> HeadHudPaintReply {
    let result = (|| {
        if input.view_revision != view.view_revision || frame.view_revision != view.view_revision {
            return Err(MenuLayoutFailure::StaleView);
        }
        if input.locale.trim().is_empty()
            || input.font.family.trim().is_empty()
            || !(1..=1000).contains(&input.font.weight)
            || input.font.generation > 9_007_199_254_740_991
            || input
                .frame_rate_label
                .as_ref()
                .is_some_and(|value| value.len() > 256)
        {
            return Err(MenuLayoutFailure::InvalidLayout);
        }
        let hud = frame.hud.as_ref();
        if hud.is_none_or(|hud| hud.groups.iter().all(|group| group.nodes.is_empty()))
            && input.frame_rate_label.is_none()
        {
            return Ok(HeadHudPaintReply::Absent);
        }
        let projection = project_xr_head_hud(
            hud,
            &input.viewer,
            input.profile,
            input.frame_rate_label.as_deref(),
        )
        .map_err(|_| MenuLayoutFailure::InvalidLayout)?;
        let XrHeadHudProjection::Visible {
            local_pose,
            size_m,
            text_height_meters,
            clear_region,
            frame_rate_bounds,
            nodes,
        } = projection
        else {
            let XrHeadHudProjection::Unavailable { reason } = projection else {
                unreachable!()
            };
            return Err(optical_failure(reason));
        };
        let size = MenuSize {
            width: size_m[0],
            height: size_m[1],
        };
        let scale = 2048.0 / size.width.max(size.height);
        let canvas_size = [
            (size.width * scale).round().max(1.0) as u32,
            (size.height * scale).round().max(1.0) as u32,
        ];
        let mut painter = HudPainter {
            input: &input,
            size,
            font: text_height_meters,
            stroke_pixels: (text_height_meters * f64::from(canvas_size[1]) / size.height * 0.1)
                .max(1.0),
            pixels_per_meter: f64::from(canvas_size[1]) / size.height,
            cache,
            measure: &mut measure,
            theme: &view.theme,
            ink: Vec::new(),
        };
        let mut items = Vec::new();
        if let (Some(value), Some(bounds)) = (&input.frame_rate_label, frame_rate_bounds) {
            let node = GameUiHudNode {
                id: "head-frame-rate".into(),
                label: String::new(),
                bounds,
                instrument_height_em: 0.0,
                content: GameUiHudNodeContent::Text {
                    value: value.clone(),
                    tone: GameUiHudTone::Normal,
                },
            };
            items.push(painter.card(
                &node,
                bounds,
                0.0,
                hud.map_or(view.theme.hud_opacity, |hud| hud.layout.opacity),
            )?);
        }
        for placement in nodes {
            let node = hud
                .and_then(|hud| {
                    hud.groups
                        .iter()
                        .flat_map(|group| &group.nodes)
                        .find(|node| node.id == placement.id)
                })
                .ok_or(MenuLayoutFailure::InvalidLayout)?;
            items.push(painter.card(
                node,
                placement.bounds,
                placement.instrument_height_em,
                hud.map_or(view.theme.hud_opacity, |hud| hud.layout.opacity),
            )?);
        }
        match fit_xr_text_ink(&input.viewer, &painter.ink, local_pose, 0.35) {
            XrInkFit::Ready { .. } | XrInkFit::Empty => {}
            XrInkFit::TooSmall => return Err(MenuLayoutFailure::InsufficientInkAngle),
            XrInkFit::Unavailable { reason } => return Err(optical_failure(reason)),
        }
        Ok(HeadHudPaintReply::Ready {
            projection: Box::new(HeadHudPaintProjection {
                local_pose,
                size,
                canvas_size,
                clear_region,
                items,
            }),
        })
    })();
    result.unwrap_or_else(|reason| HeadHudPaintReply::Unavailable { reason })
}

struct HudPainter<'a, Measure> {
    input: &'a HeadHudPreparationInput,
    size: MenuSize,
    font: f64,
    stroke_pixels: f64,
    pixels_per_meter: f64,
    cache: &'a mut SurfaceFontCache,
    measure: &'a mut Measure,
    theme: &'a GameUiTheme,
    ink: Vec<XrInkSegment>,
}

impl<Measure> HudPainter<'_, Measure>
where
    Measure: FnMut(&MenuTextRequest, MenuSize, &str) -> Result<MenuGlyphMetrics, MenuLayoutFailure>,
{
    fn text(
        &mut self,
        id: &str,
        value: String,
        width: f64,
    ) -> Result<Vec<MenuMeasuredLine>, MenuLayoutFailure> {
        let identity = serde_json::to_string(&(
            "head",
            id,
            &value,
            width,
            self.font,
            self.size,
            &self.input.font,
            &self.input.locale,
        ))
        .map_err(|_| MenuLayoutFailure::InvalidLayout)?;
        let request = MenuTextRequest {
            identity,
            slot: MenuTextSlot::Control {
                control_id: id.into(),
                role: MenuTextRole::Value,
                index: 0,
            },
            value,
            locale: self.input.locale.clone(),
            font_meters: self.font,
            font: self.input.font.clone(),
            width_meters: width,
            chart_text: None,
            legend: None,
        };
        let mut observed = self.cache.measure(&[request], self.size, self.measure)?;
        let lines = observed
            .pop()
            .ok_or(MenuLayoutFailure::MissingTextMetrics)?
            .lines;
        if lines
            .iter()
            .any(|line| line.ascent_meters + line.descent_meters > self.font * 1.5 + 1.0e-10)
        {
            return Err(MenuLayoutFailure::InvalidTextMetrics);
        }
        Ok(lines)
    }

    fn label(&mut self, id: &str, value: String) -> Result<MenuMeasuredLine, MenuLayoutFailure> {
        let mut lines = self.text(id, value, self.size.width)?;
        if lines.len() != 1 {
            return Err(MenuLayoutFailure::InvalidTextMetrics);
        }
        Ok(lines.remove(0))
    }

    fn paint_text(
        &mut self,
        items: &mut Vec<SurfacePaintItem>,
        line: &MenuMeasuredLine,
        anchor_x: f64,
        baseline: f64,
        color: &str,
        rotation_degrees: f64,
    ) {
        let angle = rotation_degrees.to_radians();
        let position = |across: f64, down: f64| Vector3 {
            x: anchor_x + across * angle.cos() - down * angle.sin() - self.size.width / 2.0,
            y: self.size.height / 2.0 - (baseline + across * angle.sin() + down * angle.cos()),
            z: 0.0,
        };
        if !line.value.trim().is_empty() {
            for edge in [-line.left_meters, line.right_meters] {
                self.ink.push(XrInkSegment {
                    start: position(edge, -line.ascent_meters),
                    end: position(edge, line.descent_meters),
                });
            }
        }
        items.push(SurfacePaintItem::Text {
            value: line.value.clone(),
            font: self.input.font.clone(),
            font_meters: self.font,
            locale: self.input.locale.clone(),
            anchor: MenuPoint {
                x: anchor_x,
                y: baseline,
            },
            rotation_degrees,
            color: color.into(),
            opacity: self.input.profile.foreground_alpha,
        });
    }

    fn card(
        &mut self,
        node: &GameUiHudNode,
        normalized: GameUiRect,
        instrument_em: f64,
        opacity: f64,
    ) -> Result<SurfacePaintItem, MenuLayoutFailure> {
        let bounds = MenuRect {
            x: normalized.x * self.size.width,
            y: normalized.y * self.size.height,
            width: normalized.width * self.size.width,
            height: normalized.height * self.size.height,
        };
        let padding = if matches!(node.content, GameUiHudNodeContent::Text { .. }) {
            0.0
        } else {
            self.font * 0.5
        };
        let line_height = self.font * 1.5;
        let value = if node.label.is_empty() {
            node.content.value().to_owned()
        } else {
            format!("{} {}", node.label, node.content.value())
        };
        let lines = self.text(&node.id, value, bounds.width - padding * 2.0)?;
        let instrument_height = instrument_em * self.font;
        if lines.len() as f64 * line_height + instrument_height + padding * 2.0
            > bounds.height + 1.0e-10
        {
            return Err(MenuLayoutFailure::InvalidLayout);
        }
        let mut items = vec![SurfacePaintItem::Rectangle {
            bounds,
            color: self.theme.panel_background.clone(),
            opacity,
            gradient: None,
        }];
        let text_color = match node.content {
            GameUiHudNodeContent::Text {
                tone: GameUiHudTone::Muted,
                ..
            } => self.theme.muted_text.clone(),
            GameUiHudNodeContent::Text {
                tone: GameUiHudTone::Warning,
                ..
            } => self.theme.focus_outline.clone(),
            _ => self.theme.text.clone(),
        };
        for (index, line) in lines.iter().enumerate() {
            let anchor_x = bounds.x + padding + line.left_meters.max(0.0);
            let baseline = bounds.y
                + padding
                + index as f64 * line_height
                + line_height / 2.0
                + (line.ascent_meters - line.descent_meters) / 2.0;
            self.paint_text(&mut items, line, anchor_x, baseline, &text_color, 0.0);
        }
        let instrument = MenuRect {
            x: bounds.x + padding,
            y: bounds.y + padding + lines.len() as f64 * line_height,
            width: bounds.width - padding * 2.0,
            height: instrument_height,
        };
        self.instrument_labels(node, bounds, instrument, padding, &mut items)?;
        self.instrument(&node.content, instrument, &mut items);
        Ok(SurfacePaintItem::Plot {
            clip: bounds,
            items,
        })
    }

    fn instrument_labels(
        &mut self,
        node: &GameUiHudNode,
        card: MenuRect,
        instrument: MenuRect,
        padding: f64,
        items: &mut Vec<SurfacePaintItem>,
    ) -> Result<(), MenuLayoutFailure> {
        match &node.content {
            GameUiHudNodeContent::Attitude { geometry, .. } => {
                if let Some(horizon) = geometry.horizon {
                    let angle = horizon.label_rotation_degrees.to_radians();
                    let (scale, origin) = instrument_fit(instrument, geometry.canvas_size);
                    for tick in &horizon.pitch_ticks {
                        let (Some(value), Some(label_anchor)) =
                            (tick.label_degrees, tick.label_anchor)
                        else {
                            continue;
                        };
                        let line = self.label(&node.id, value.to_string())?;
                        let anchor_x = origin.x + label_anchor.x * scale
                            - angle.cos() * line.advance_meters / 2.0;
                        let baseline = origin.y + label_anchor.y * scale
                            - angle.sin() * line.advance_meters / 2.0;
                        let inside =
                            [-line.left_meters, line.right_meters]
                                .into_iter()
                                .all(|across| {
                                    [-line.ascent_meters, line.descent_meters].into_iter().all(
                                        |down| {
                                            let position_x = anchor_x + across * angle.cos()
                                                - down * angle.sin();
                                            let position_y = baseline
                                                + across * angle.sin()
                                                + down * angle.cos();
                                            position_x >= card.x + padding
                                                && position_x <= card.x + card.width - padding
                                                && position_y >= instrument.y
                                                && position_y <= instrument.y + instrument.height
                                        },
                                    )
                                });
                        if inside {
                            self.paint_text(
                                items,
                                &line,
                                anchor_x,
                                baseline,
                                &self.theme.text.clone(),
                                horizon.label_rotation_degrees,
                            );
                        }
                    }
                }
            }
            GameUiHudNodeContent::HeadingTape {
                canvas_size, ticks, ..
            } => {
                for tick in ticks {
                    let Some(anchor) = tick.label_anchor else {
                        continue;
                    };
                    self.instrument_label(
                        &node.id,
                        tick.label.clone(),
                        (anchor, *canvas_size, GameUiChartTextAnchor::Middle),
                        instrument,
                        items,
                    )?;
                }
            }
            GameUiHudNodeContent::LinearGauge {
                minimum_label,
                maximum_label,
                canvas_size,
                label_anchors,
                ..
            } => {
                for (value, anchor, align) in [
                    (
                        minimum_label,
                        label_anchors[0],
                        GameUiChartTextAnchor::Start,
                    ),
                    (maximum_label, label_anchors[1], GameUiChartTextAnchor::End),
                ] {
                    self.instrument_label(
                        &node.id,
                        value.clone(),
                        (anchor, *canvas_size, align),
                        instrument,
                        items,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn instrument_label(
        &mut self,
        id: &str,
        value: String,
        (anchor, canvas_size, align): (GameUiChartPoint, [f64; 2], GameUiChartTextAnchor),
        bounds: MenuRect,
        items: &mut Vec<SurfacePaintItem>,
    ) -> Result<(), MenuLayoutFailure> {
        let line = self.label(id, value)?;
        let (scale, origin) = instrument_fit(bounds, canvas_size);
        let anchor_x = origin.x + anchor.x * scale
            - line.advance_meters
                * match align {
                    GameUiChartTextAnchor::Start => 0.0,
                    GameUiChartTextAnchor::Middle => 0.5,
                    GameUiChartTextAnchor::End => 1.0,
                };
        let baseline = origin.y + anchor.y * scale;
        if anchor_x - line.left_meters < bounds.x
            || anchor_x + line.right_meters > bounds.x + bounds.width
            || baseline - line.ascent_meters < bounds.y
            || baseline + line.descent_meters > bounds.y + bounds.height
        {
            return Err(MenuLayoutFailure::InvalidTextMetrics);
        }
        self.paint_text(
            items,
            &line,
            anchor_x,
            baseline,
            &self.theme.text.clone(),
            0.0,
        );
        Ok(())
    }

    fn line(&self, items: &mut Vec<SurfacePaintItem>, start: [f64; 2], end: [f64; 2], color: &str) {
        items.push(SurfacePaintItem::Line {
            start: MenuPoint {
                x: start[0],
                y: start[1],
            },
            end: MenuPoint {
                x: end[0],
                y: end[1],
            },
            color: color.into(),
            width_px: self.stroke_pixels,
            opacity: self.input.profile.foreground_alpha,
        });
    }

    fn instrument(
        &self,
        content: &GameUiHudNodeContent,
        bounds: MenuRect,
        items: &mut Vec<SurfacePaintItem>,
    ) {
        let [center_x, center_y] = [
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        ];
        match content {
            GameUiHudNodeContent::Text { .. } => {}
            GameUiHudNodeContent::Attitude { geometry, .. } => {
                let (scale, origin) = instrument_fit(bounds, geometry.canvas_size);
                let point =
                    |source: [f64; 2]| [origin.x + source[0] * scale, origin.y + source[1] * scale];
                if let Some(horizon) = geometry.horizon {
                    for tick in &horizon.pitch_ticks {
                        if let Some([start, end]) = tick.segment {
                            self.line(
                                items,
                                point([start.x, start.y]),
                                point([end.x, end.y]),
                                &self.theme.text,
                            );
                        }
                    }
                }
                for [start, end] in geometry.aircraft_segments {
                    self.line(
                        items,
                        point([start.x, start.y]),
                        point([end.x, end.y]),
                        &self.theme.focus_outline,
                    );
                }
                let clip = geometry.clip;
                items.push(SurfacePaintItem::RoundedOutline {
                    bounds: MenuRect {
                        x: origin.x + clip.x * scale,
                        y: origin.y + clip.y * scale,
                        width: clip.width * scale,
                        height: clip.height * scale,
                    },
                    radius: geometry.clip_radius * scale,
                    color: self.theme.text.clone(),
                    width_px: self.stroke_pixels,
                    opacity: self.input.profile.foreground_alpha,
                });
                if let Some(center) = geometry.flight_path_center {
                    let radius = geometry.flight_path_radius * scale;
                    let center = point([center.x, center.y]);
                    items.push(SurfacePaintItem::RoundedOutline {
                        bounds: MenuRect {
                            x: center[0] - radius,
                            y: center[1] - radius,
                            width: radius * 2.0,
                            height: radius * 2.0,
                        },
                        radius,
                        color: "#7df4c5".into(),
                        width_px: self.stroke_pixels,
                        opacity: self.input.profile.foreground_alpha,
                    });
                }
            }
            GameUiHudNodeContent::HeadingTape { canvas_size, .. }
            | GameUiHudNodeContent::LinearGauge { canvas_size, .. } => {
                let (scale, origin) = instrument_fit(bounds, *canvas_size);
                let point = |source: GameUiChartPoint| MenuPoint {
                    x: origin.x + source.x * scale,
                    y: origin.y + source.y * scale,
                };
                let line = |items: &mut Vec<SurfacePaintItem>,
                            segment: [GameUiChartPoint; 2],
                            color: &str,
                            width: f64| {
                    items.push(SurfacePaintItem::Line {
                        start: point(segment[0]),
                        end: point(segment[1]),
                        color: color.into(),
                        width_px: width * scale * self.pixels_per_meter,
                        opacity: self.input.profile.foreground_alpha,
                    });
                };
                let marker = match content {
                    GameUiHudNodeContent::HeadingTape { ticks, pointer, .. } => {
                        for tick in ticks.iter() {
                            line(items, tick.segment, &self.theme.text, tick.stroke_width);
                        }
                        Some(*pointer)
                    }
                    GameUiHudNodeContent::LinearGauge {
                        track,
                        ticks,
                        marker,
                        ..
                    } => {
                        line(items, *track, &self.theme.muted_text, 3.0);
                        for tick in ticks.iter() {
                            line(items, *tick, &self.theme.muted_text, 1.0);
                        }
                        *marker
                    }
                    _ => unreachable!(),
                };
                if let Some(marker) = marker {
                    items.push(SurfacePaintItem::Triangle {
                        points: marker.map(point),
                        color: self.theme.focus_outline.clone(),
                        opacity: self.input.profile.foreground_alpha,
                    });
                }
            }
            GameUiHudNodeContent::Direction {
                rotation_degrees, ..
            } => {
                if let Some(angle) = rotation_degrees.map(f64::to_radians) {
                    let radius = (bounds.width * 0.3).min(bounds.height * 0.4);
                    let end = [
                        center_x + angle.sin() * radius,
                        center_y - angle.cos() * radius,
                    ];
                    self.line(items, [center_x, center_y], end, &self.theme.focus_outline);
                    for delta in [-0.5, 0.5] {
                        self.line(
                            items,
                            end,
                            [
                                end[0] - (angle + delta).sin() * radius * 0.35,
                                end[1] + (angle + delta).cos() * radius * 0.35,
                            ],
                            &self.theme.focus_outline,
                        );
                    }
                }
            }
        }
    }
}

fn instrument_fit(bounds: MenuRect, canvas: [f64; 2]) -> (f64, MenuPoint) {
    let scale = (bounds.width / canvas[0]).min(bounds.height / canvas[1]);
    (
        scale,
        MenuPoint {
            x: bounds.x + (bounds.width - canvas[0] * scale) / 2.0,
            y: bounds.y + (bounds.height - canvas[1] * scale) / 2.0,
        },
    )
}

fn optical_failure(reason: XrFitReason) -> MenuLayoutFailure {
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
