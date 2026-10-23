//! Batch JSON entry point for direct-Rust and actual-WASM optical comparisons.

use birdman_game_presentation::{
    camera::{compose_pose, inverse_pose},
    game_ui::GameUiAnchor,
    xr::*,
};
use serde::Deserialize;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    viewer: XrViewerFrame,
    panel_pose: Pose,
    panel_size: [f64; 2],
    spatial: SpatialObservation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Batch {
    optics: Vec<Observation>,
    head: Vec<HeadObservation>,
    interaction: PointerScenario,
    menu: Vec<MenuObservation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeadObservation {
    view: birdman_game_presentation::game_ui::GameUiView,
    frame: birdman_game_presentation::game_ui::GameUiFrame,
    input: HeadHudPreparationInput,
    probes: Vec<TextProbe>,
}

fn head_results(observations: Vec<HeadObservation>) -> Vec<HeadHudPaintReply> {
    observations
        .into_iter()
        .map(|observation| {
            let mut probes = observation.probes.iter();
            let reply = prepare_game_ui_head_hud(
                &observation.view,
                &observation.frame,
                observation.input,
                &mut SurfaceFontCache::default(),
                |request, size, value| {
                    let probe = probes.next().ok_or(MenuLayoutFailure::InvalidTextMetrics)?;
                    if probe.value != value
                        || probe.weight != request.font.weight
                        || (probe.font_meters - request.font_meters).abs() > 1.0e-12
                        || (probe.size.width - size.width).abs() > 1.0e-12
                        || (probe.size.height - size.height).abs() > 1.0e-12
                    {
                        return Err(MenuLayoutFailure::InvalidTextMetrics);
                    }
                    let [advance, left, right, ascent, descent] = probe.metrics;
                    Ok(MenuGlyphMetrics {
                        advance,
                        left,
                        right,
                        ascent,
                        descent,
                    })
                },
            );
            assert!(
                probes.next().is_none(),
                "Native Head HUD consumed a different font observation trace"
            );
            reply
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MenuObservation {
    view: birdman_game_presentation::game_ui::GameUiView,
    frame: birdman_game_presentation::game_ui::GameUiFrame,
    input: MenuPreparationInput,
    probes: Vec<TextProbe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextProbe {
    value: String,
    font_meters: f64,
    weight: u16,
    size: MenuSize,
    metrics: [f64; 5],
}

fn menu_results(observations: Vec<MenuObservation>) -> Vec<serde_json::Value> {
    observations
        .into_iter()
        .map(|observation| {
            let mut probes = observation.probes.iter();
            let preparation = prepare_game_ui_menu(
                &observation.view,
                &observation.frame,
                observation.input,
                &mut SurfaceFontCache::default(),
                |request, size, value| {
                    let probe = probes.next().ok_or(MenuLayoutFailure::InvalidTextMetrics)?;
                    if probe.value != value
                        || probe.weight != request.font.weight
                        || (probe.font_meters - request.font_meters).abs() > 1.0e-12
                        || (probe.size.width - size.width).abs() > 1.0e-12
                        || (probe.size.height - size.height).abs() > 1.0e-12
                    {
                        return Err(MenuLayoutFailure::InvalidTextMetrics);
                    }
                    let [advance, left, right, ascent, descent] = probe.metrics;
                    Ok(MenuGlyphMetrics {
                        advance,
                        left,
                        right,
                        ascent,
                        descent,
                    })
                },
            );
            assert!(
                probes.next().is_none(),
                "Native Menu consumed a different font observation trace"
            );
            serde_json::json!({ "preparation": preparation })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointerScenario {
    surface: XrPointerSurface,
    steps: Vec<PointerStep>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PointerStep {
    observation: XrPointerObservation,
    select: Option<(XrPointerRay, f64)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpatialObservation {
    aircraft: Pose,
    head: Pose,
    mounts: Vec<XrCameraMount>,
    resets: Vec<Pose>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let batch: Batch = serde_json::from_str(&input)?;
    let mut optics = Vec::with_capacity(batch.optics.len());
    for observation in batch.optics {
        optics.push(serde_json::json!({
            "spatial": spatial_results(&observation)?,
        }));
    }
    let output = serde_json::to_vec(&serde_json::json!({
        "optics": optics,
        "head": head_results(batch.head),
        "interaction": pointer_results(&batch.interaction)?,
        "menu": menu_results(batch.menu),
    }))?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn pointer_results(
    scenario: &PointerScenario,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let mut owner = XrPointerSession::default();
    owner.set_surface(Some(scenario.surface.clone()))?;
    scenario
        .steps
        .iter()
        .map(|step| {
            let frame = owner.observe(step.observation.clone())?;
            let select = step
                .select
                .and_then(|(pointer, timestamp)| owner.select(pointer, timestamp));
            Ok(serde_json::json!({ "frame": frame, "select": select }))
        })
        .collect()
}

fn spatial_results(
    observation: &Observation,
) -> Result<Vec<Vec<serde_json::Value>>, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    for mount in &observation.spatial.mounts {
        let mut origin = XrTrackingOrigin::default();
        let mut spatial = XrSpatialSession::new();
        let mut head = observation.spatial.head;
        let mut frames = Vec::new();
        for step in 0..=observation.spatial.resets.len() {
            let rig = origin.camera_rig(observation.spatial.aircraft, *mount)?;
            let reference = xr_reference_frame(rig, head)?;
            let geometry = XrPanelGeometry {
                scope: XrMenuScope {
                    scene: "Replay".into(),
                    overlay: None,
                    panel_id: "main".into(),
                },
                anchor: GameUiAnchor::Menu,
                local_pose: observation.panel_pose,
                size_m: observation.panel_size,
            };
            let menu = spatial.place_panel(
                XrPanelObservation::Ready { geometry },
                &observation.viewer,
                Some(reference),
            );
            let mut anchors = Vec::new();
            for anchor in [
                GameUiAnchor::World,
                GameUiAnchor::Cockpit,
                GameUiAnchor::Head,
            ] {
                let panel = resolve_xr_anchor(
                    anchor,
                    observation.panel_pose,
                    XrAnchorFrames {
                        world: reference.world,
                        cockpit: reference.cockpit,
                        menu: Pose::IDENTITY,
                        head: reference.head,
                    },
                )?;
                anchors.push(xr_panel_camera_poses(rig, panel)?);
            }
            let eyes = match &observation.viewer {
                XrViewerFrame::Configured { eyes, .. }
                | XrViewerFrame::RuntimeDerived { eyes, .. } => eyes,
                XrViewerFrame::Unavailable { .. } => {
                    return Err("spatial comparison requires binocular geometry".into());
                }
            };
            let eyes: Vec<_> = eyes
                .iter()
                .map(|eye| {
                    compose_pose(
                        compose_pose(rig.world_from_tracking, head)?,
                        eye.head_from_eye,
                    )
                })
                .collect::<Result<_, _>>()?;
            frames.push(serde_json::json!({ "rig": rig, "reference": reference, "menu": menu, "anchors": anchors, "eyes": eyes }));
            if let Some(change) = observation.spatial.resets.get(step) {
                origin.reset_reference(*change)?;
                head = compose_pose(inverse_pose(*change)?, head)?;
            }
        }
        output.push(frames);
    }
    Ok(output)
}
