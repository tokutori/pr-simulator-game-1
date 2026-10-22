use crate::{decode, encode, error};
use birdman_game_presentation::camera as shared;
use wasm_bindgen::prelude::*;

/// Composes a bounded chain of named reference-space poses.
#[wasm_bindgen]
pub fn pose_chain_json(poses_json: &str) -> Result<String, JsValue> {
    let poses: Vec<shared::Pose> = decode(poses_json, 8_192)?;
    if poses.is_empty() || poses.len() > 4 {
        return Err(error("Camera pose chain requires one to four poses"));
    }
    let pose = poses
        .into_iter()
        .try_fold(shared::Pose::IDENTITY, shared::compose_pose)
        .map_err(error)?;
    encode(&pose)
}

/// Inverts one admitted pose.
#[wasm_bindgen]
pub fn inverse_pose_json(pose_json: &str) -> Result<String, JsValue> {
    encode(&shared::inverse_pose(decode(pose_json, 1_024)?).map_err(error)?)
}

/// Converts a physical FRD pose into render coordinates.
#[wasm_bindgen]
pub fn frd_pose_json(pose_json: &str) -> Result<String, JsValue> {
    encode(&shared::frd_pose_to_render(decode(pose_json, 1_024)?).map_err(error)?)
}

/// Resolves the registered content-defined pilot eye.
#[wasm_bindgen]
pub fn registered_pilot_eye_json() -> Result<String, JsValue> {
    encode(&shared::registered_pilot_eye().map_err(error)?)
}

/// Projects the pilot displacement and returns physical and render-local poses.
#[wasm_bindgen]
pub fn pilot_eye_pose_json(
    initial_json: &str,
    position: f64,
    initial_position: f64,
) -> Result<String, JsValue> {
    encode(
        &shared::pilot_eye_pose(decode(initial_json, 1_024)?, position, initial_position)
            .map_err(error)?,
    )
}

/// Returns the shared physical Chase camera pose.
#[wasm_bindgen]
pub fn chase_pose_json() -> Result<String, JsValue> {
    encode(&shared::chase_pose_frd())
}

/// Projects a render-local pose through the physical aircraft frame.
#[wasm_bindgen]
pub fn flight_relative_pose_json(
    position_json: &str,
    attitude_json: &str,
    local_json: &str,
) -> Result<String, JsValue> {
    encode(
        &shared::flight_relative_pose(
            decode(position_json, 512)?,
            decode(attitude_json, 512)?,
            decode(local_json, 1_024)?,
        )
        .map_err(error)?,
    )
}

/// Projects a local Pilot/Chase mount with the shared Screen look policy.
#[wasm_bindgen]
pub fn camera_local_pose_json(
    initial_json: &str,
    position: f64,
    initial_position: f64,
    chase: bool,
    yaw: f64,
    pitch: f64,
) -> Result<String, JsValue> {
    encode(
        &shared::camera_local_pose(
            decode(initial_json, 1_024)?,
            position,
            initial_position,
            chase,
            [yaw, pitch],
        )
        .map_err(error)?,
    )
}
