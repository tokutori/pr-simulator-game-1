//! Batch JSON entry point for direct-Rust and actual-WASM optical comparisons.

use birdman_game_presentation::{
    camera::{compose_pose, inverse_pose},
    game_ui::{GameUiAnchor, GameUiHud},
    xr::*,
};
use serde::Deserialize;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    viewer: XrViewerFrame,
    hud: Option<GameUiHud>,
    profile: XrHeadHudProfile,
    panel_pose: Pose,
    panel_size: [f64; 2],
    head_from_surface: Pose,
    ink: Vec<XrInkSegment>,
    spatial: SpatialObservation,
    interaction: PointerScenario,
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
            "spatial": spatial_results(&observation)?,
            "interaction": pointer_results(&observation.interaction)?,
        }));
    }
    let output = serde_json::to_vec(&results)?;
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
