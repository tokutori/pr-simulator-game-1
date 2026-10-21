//! Batch JSON entry point for direct-Rust and actual-WASM optical comparisons.

use birdman_game_presentation::{game_ui::GameUiHud, xr::*};
use serde::Deserialize;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    viewer: XrViewerFrame,
    hud: Option<GameUiHud>,
    profile: XrHeadHudProfile,
    panel_pose: XrPose,
    panel_size: [f64; 2],
    head_from_surface: XrPose,
    ink: Vec<XrInkSegment>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let observations: Vec<Observation> = serde_json::from_str(&input)?;
    let mut results = Vec::with_capacity(observations.len());
    for observation in observations {
        results.push(serde_json::json!({
            "menu": fit_xr_menu_view(&observation.viewer, observation.panel_size,
                observation.panel_pose, 2.4, 0.035)?,
            "head": project_xr_head_hud(observation.hud.as_ref(), &observation.viewer,
                observation.profile, Some("FPS 72.0"))?,
            "ink": fit_xr_text_ink(&observation.viewer, &observation.ink,
                observation.head_from_surface, 0.35),
        }));
    }
    let output = serde_json::to_vec(&results)?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")?;
    Ok(())
}
