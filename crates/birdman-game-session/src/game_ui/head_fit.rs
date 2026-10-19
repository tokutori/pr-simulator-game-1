use super::types::*;
use serde::{Deserialize, Serialize};

/// Available head-fixed space after optical clipping, clear-region and menu reservation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHeadHudSpace {
    /// Complete visible HUD plane width and height, in metres.
    pub size_m: [f64; 2],
    /// Top edge and height of the optional upper band, in metres from the plane top.
    pub top_band_m: [f64; 2],
    /// Top edge and height of the preferred lower band, in metres from the plane top.
    pub bottom_band_m: [f64; 2],
    /// Required angular text height projected into the HUD plane, in metres.
    pub text_height_m: f64,
}

/// Shared placement of one retained element in the head-fixed plane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GameUiHeadHudNode {
    /// Identity of the unchanged source node.
    pub id: String,
    /// Normalized left/top rectangle within the complete HUD plane.
    pub bounds: GameUiRect,
    /// Shared instrument height in multiples of the required text height.
    pub instrument_height_em: f64,
}

/// Exhaustive head-layout result; insufficient space never omits an element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameUiHeadHudFit {
    /// Every source element fits at the requested text height.
    Fitted {
        /// Complete source-node placement in the original shared order.
        nodes: Vec<GameUiHeadHudNode>,
    },
    /// The reserved bands cannot contain the complete HUD.
    Unavailable,
}

/// Packs the shared nodes below the forward view, with whole-row upper fallback.
pub fn fit_game_ui_head_hud(hud: &GameUiHud, space: GameUiHeadHudSpace) -> GameUiHeadHudFit {
    let [width, height] = space.size_m;
    if ![width, height, space.text_height_m, hud.layout.font_size_px]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
        || ![space.top_band_m, space.bottom_band_m]
            .into_iter()
            .all(|[top, band_height]| {
                top.is_finite()
                    && band_height.is_finite()
                    && top >= 0.0
                    && band_height >= 0.0
                    && top + band_height <= height + 1.0e-9
            })
        || space.top_band_m[0] + space.top_band_m[1] > space.bottom_band_m[0]
        || hud
            .groups
            .iter()
            .map(|group| group.nodes.len())
            .sum::<usize>()
            > 128
    {
        return GameUiHeadHudFit::Unavailable;
    }
    let scale = space.text_height_m / hud.layout.font_size_px;
    let readouts = hud
        .groups
        .iter()
        .filter(|group| group.placement == GameUiHudPlacement::Readouts)
        .flat_map(|group| group.nodes.iter().cloned())
        .collect::<Vec<_>>();
    let instruments = hud
        .groups
        .iter()
        .filter(|group| group.placement == GameUiHudPlacement::Instruments)
        .flat_map(|group| group.nodes.iter().cloned())
        .collect::<Vec<_>>();
    let mut layout = hud.layout.clone();
    layout.width_px = width / scale;
    let mut groups =
        super::hud::position_hud(&mut layout, readouts.clone(), instruments.clone(), true);
    if layout.height_px * scale <= space.bottom_band_m[1] + 1.0e-9 {
        return GameUiHeadHudFit::Fitted {
            nodes: groups
                .into_iter()
                .flat_map(|group| group.nodes)
                .map(|node| GameUiHeadHudNode {
                    id: node.id,
                    bounds: GameUiRect {
                        x: node.bounds.x,
                        y: (space.bottom_band_m[0] + node.bounds.y * layout.height_px * scale)
                            / height,
                        width: node.bounds.width,
                        height: node.bounds.height * layout.height_px * scale / height,
                    },
                    instrument_height_em: node.instrument_height_em,
                })
                .collect(),
        };
    }
    groups = super::hud::position_hud(&mut layout, readouts, instruments, false);
    let gap = layout.gap_px[1] * scale;
    let padding = layout.padding_px[1] * scale;
    let row_heights = groups
        .iter()
        .map(|group| {
            group
                .nodes
                .iter()
                .map(|node| node.bounds.height * layout.height_px * scale)
                .fold(0.0, f64::max)
        })
        .collect::<Vec<_>>();
    let band_height = |heights: &[f64]| -> f64 {
        heights.iter().sum::<f64>() + heights.len().saturating_sub(1) as f64 * gap + 2.0 * padding
    };
    let Some(upper_rows) = (1..=groups.len()).find(|&count| {
        band_height(&row_heights[..count]) <= space.top_band_m[1] + 1.0e-9
            && band_height(&row_heights[count..]) <= space.bottom_band_m[1] + 1.0e-9
    }) else {
        return GameUiHeadHudFit::Unavailable;
    };
    let mut nodes = Vec::new();
    let mut row_top = space.top_band_m[0] + padding;
    for (row_index, group) in groups.into_iter().enumerate() {
        if row_index == upper_rows {
            row_top = space.bottom_band_m[0] + padding;
        }
        for node in group.nodes {
            nodes.push(GameUiHeadHudNode {
                id: node.id,
                bounds: GameUiRect {
                    x: node.bounds.x,
                    y: row_top / height,
                    width: node.bounds.width,
                    height: row_heights[row_index] / height,
                },
                instrument_height_em: node.instrument_height_em,
            });
        }
        row_top += row_heights[row_index] + gap;
    }
    GameUiHeadHudFit::Fitted { nodes }
}
