use super::{
    MenuFont, MenuGlyphMetrics, MenuLayoutFailure, MenuMeasuredLine, MenuPoint, MenuRect, MenuSize,
    MenuTextRequest, MenuTextRole, MenuTextSlot, SurfaceFontCache, SurfacePaintItem, XrFitReason,
    XrHeadHudProfile, XrHeadHudProjection, XrInkFit, XrInkSegment, XrViewerFrame, fit_xr_text_ink,
    project_xr_head_hud,
};
use crate::camera::{Pose, Vector3};
use crate::game_ui::{
    GameUiFrame, GameUiHudNode, GameUiHudNodeContent, GameUiHudTone, GameUiRect, GameUiTheme,
    GameUiView,
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
    ) {
        if !line.value.trim().is_empty() {
            for edge in [anchor_x - line.left_meters, anchor_x + line.right_meters] {
                self.ink.push(XrInkSegment {
                    start: Vector3 {
                        x: edge - self.size.width / 2.0,
                        y: self.size.height / 2.0 - (baseline - line.ascent_meters),
                        z: 0.0,
                    },
                    end: Vector3 {
                        x: edge - self.size.width / 2.0,
                        y: self.size.height / 2.0 - (baseline + line.descent_meters),
                        z: 0.0,
                    },
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
            rotation_degrees: 0.0,
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
            self.paint_text(&mut items, line, anchor_x, baseline, &text_color);
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
        let line_height = self.font * 1.5;
        match &node.content {
            GameUiHudNodeContent::Attitude { geometry, .. } => {
                if let Some(horizon) = geometry.horizon() {
                    let angle = horizon.rotation_degrees.to_radians();
                    for tick in &horizon.pitch_ticks {
                        let Some(value) = tick.label_degrees else {
                            continue;
                        };
                        let line = self.label(&node.id, value.to_string())?;
                        let across = tick.half_width_ratio * instrument.width + self.font * 0.7;
                        let down =
                            (horizon.pitch_offset_ratio + tick.offset_ratio) * instrument.height;
                        let anchor_x = card.x + card.width / 2.0 + angle.cos() * across
                            - angle.sin() * down
                            - line.advance_meters / 2.0;
                        let baseline = instrument.y
                            + instrument.height / 2.0
                            + angle.sin() * across
                            + angle.cos() * down
                            + (line.ascent_meters - line.descent_meters) / 2.0;
                        if anchor_x - line.left_meters >= card.x + padding
                            && anchor_x + line.right_meters <= card.x + card.width - padding
                            && baseline - line.ascent_meters >= instrument.y
                            && baseline + line.descent_meters <= instrument.y + instrument.height
                        {
                            self.paint_text(
                                items,
                                &line,
                                anchor_x,
                                baseline,
                                &self.theme.text.clone(),
                            );
                        }
                    }
                }
            }
            GameUiHudNodeContent::HeadingTape { ticks, .. } => {
                for tick in ticks.iter().filter(|tick| !tick.label.is_empty()) {
                    self.instrument_label(
                        &node.id,
                        tick.label.clone(),
                        tick.position_ratio,
                        instrument,
                        line_height,
                        items,
                    )?;
                }
            }
            GameUiHudNodeContent::LinearGauge {
                minimum_label,
                maximum_label,
                ..
            } => {
                for (value, ratio) in [(minimum_label, 0.0), (maximum_label, 1.0)] {
                    self.instrument_label(
                        &node.id,
                        value.clone(),
                        ratio,
                        instrument,
                        line_height,
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
        ratio: f64,
        bounds: MenuRect,
        line_height: f64,
        items: &mut Vec<SurfacePaintItem>,
    ) -> Result<(), MenuLayoutFailure> {
        let line = self.label(id, value)?;
        let anchor_x = (bounds.x + ratio * bounds.width - line.advance_meters / 2.0)
            .min(bounds.x + bounds.width - line.right_meters)
            .max(bounds.x + line.left_meters.max(0.0));
        let baseline =
            bounds.y + line_height / 2.0 + (line.ascent_meters - line.descent_meters) / 2.0;
        if anchor_x - line.left_meters < bounds.x
            || anchor_x + line.right_meters > bounds.x + bounds.width
            || baseline - line.ascent_meters < bounds.y
            || baseline + line.descent_meters > bounds.y + bounds.height
        {
            return Err(MenuLayoutFailure::InvalidTextMetrics);
        }
        self.paint_text(items, &line, anchor_x, baseline, &self.theme.text.clone());
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

    fn flight_path(&self, items: &mut Vec<SurfacePaintItem>, center_x: f64, marker_y: f64) {
        let radius = self.font * 0.35;
        self.line(
            items,
            [center_x - radius, marker_y],
            [center_x, marker_y - radius],
            "#7df4c5",
        );
        self.line(
            items,
            [center_x, marker_y - radius],
            [center_x + radius, marker_y],
            "#7df4c5",
        );
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
                if let Some(horizon) = geometry.horizon() {
                    let angle = horizon.rotation_degrees.to_radians();
                    let length = (bounds.width * 0.43).min(bounds.height * 0.5);
                    for tick in &horizon.pitch_ticks {
                        let offset =
                            (horizon.pitch_offset_ratio + tick.offset_ratio) * bounds.height;
                        let [center_x, center_y] = [
                            center_x - angle.sin() * offset,
                            center_y + angle.cos() * offset,
                        ];
                        let half = tick.half_width_ratio * bounds.width;
                        self.line(
                            items,
                            [center_x - angle.cos() * half, center_y - angle.sin() * half],
                            [center_x + angle.cos() * half, center_y + angle.sin() * half],
                            &self.theme.text,
                        );
                    }
                    self.line(
                        items,
                        [center_x - length * 0.45, center_y],
                        [center_x + length * 0.45, center_y],
                        &self.theme.focus_outline,
                    );
                    self.line(
                        items,
                        [center_x, center_y - self.font * 0.25],
                        [center_x, center_y + self.font * 0.25],
                        &self.theme.focus_outline,
                    );
                }
                if let Some(ratio) = geometry.flight_path_y_ratio() {
                    self.flight_path(items, center_x, bounds.y + ratio * bounds.height);
                }
            }
            GameUiHudNodeContent::HeadingTape { ticks, .. } => {
                for tick in ticks {
                    let tick_x = bounds.x + tick.position_ratio * bounds.width;
                    self.line(
                        items,
                        [
                            tick_x,
                            bounds.y + bounds.height * if tick.major { 0.5 } else { 0.75 },
                        ],
                        [tick_x, bounds.y + bounds.height],
                        if tick.major {
                            &self.theme.text
                        } else {
                            &self.theme.muted_text
                        },
                    );
                }
                self.line(
                    items,
                    [center_x - self.font * 0.2, bounds.y + bounds.height * 0.9],
                    [center_x + self.font * 0.2, bounds.y + bounds.height * 0.9],
                    &self.theme.focus_outline,
                );
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
            GameUiHudNodeContent::LinearGauge { position_ratio, .. } => {
                let left = bounds.x + bounds.width * 0.05;
                let width = bounds.width * 0.9;
                self.line(
                    items,
                    [left, center_y],
                    [left + width, center_y],
                    &self.theme.text,
                );
                self.line(
                    items,
                    [center_x, bounds.y + bounds.height * 0.3],
                    [center_x, bounds.y + bounds.height * 0.7],
                    &self.theme.muted_text,
                );
                if let Some(ratio) = position_ratio {
                    self.line(
                        items,
                        [left + ratio * width, bounds.y + bounds.height * 0.35],
                        [left + ratio * width, bounds.y + bounds.height * 0.85],
                        &self.theme.focus_outline,
                    );
                }
            }
        }
    }
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
