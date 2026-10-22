use super::{decode, encode, error};
use birdman_game_presentation::xr as shared;
use wasm_bindgen::prelude::*;

/// Conjugates a new-from-previous tracking change by the mounted reference frame.
#[wasm_bindgen]
pub fn xr_reference_change_json(mount_json: &str, change_json: &str) -> Result<String, JsValue> {
    encode(
        &shared::xr_reference_change(decode(mount_json, 1_024)?, decode(change_json, 1_024)?)
            .map_err(error)?,
    )
}

/// Projects one explicitly anchored panel for both scene-root and tracking-parent draw paths.
#[wasm_bindgen]
pub fn xr_panel_camera_poses_json(rig_json: &str, panel_json: &str) -> Result<String, JsValue> {
    encode(
        &shared::xr_panel_camera_poses(decode(rig_json, 8_192)?, decode(panel_json, 1_024)?)
            .map_err(error)?,
    )
}

/// Resolves physical and head anchors from one camera rig.
#[wasm_bindgen]
pub fn xr_reference_frame_json(rig_json: &str, head_json: &str) -> Result<String, JsValue> {
    encode(
        &shared::xr_reference_frame(decode(rig_json, 8_192)?, decode(head_json, 1_024)?)
            .map_err(error)?,
    )
}

/// Owns passive tracking-origin corrections for one runtime session.
#[wasm_bindgen]
#[derive(Default)]
pub struct XrCameraBridge {
    origin: shared::XrTrackingOrigin,
}

#[wasm_bindgen]
impl XrCameraBridge {
    /// Creates an identity runtime origin.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Derives a rig with the current native-origin correction.
    pub fn camera_rig_json(
        &self,
        aircraft_json: &str,
        mount_json: &str,
    ) -> Result<String, JsValue> {
        encode(
            &self
                .origin
                .camera_rig(decode(aircraft_json, 1_024)?, decode(mount_json, 2_048)?)
                .map_err(error)?,
        )
    }

    /// Accumulates the new native origin expressed in previous tracking coordinates.
    pub fn reset_reference_json(&mut self, previous_from_new_json: &str) -> Result<(), JsValue> {
        self.origin
            .reset_reference(decode(previous_from_new_json, 1_024)?)
            .map_err(error)
    }
}

/// Owns retained menu anchors for one active presentation session.
#[wasm_bindgen]
pub struct XrSpatialBridge {
    session: shared::XrSpatialSession,
}

impl Default for XrSpatialBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl XrSpatialBridge {
    /// Creates a fresh menu owner.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            session: shared::XrSpatialSession::new(),
        }
    }

    /// Observes measured geometry and derives the same-frame panel pose.
    pub fn place_panel_json(
        &mut self,
        observation_json: &str,
        viewer_json: &str,
        reference_json: &str,
    ) -> Result<String, JsValue> {
        let observation = decode(observation_json, 8_192)?;
        let viewer = decode(viewer_json, 16_384)?;
        let reference = decode(reference_json, 4_096)?;
        encode(&self.session.place_panel(observation, &viewer, reference))
    }

    /// Recentres an open menu without changing its fitted distance.
    pub fn recenter_menu_json(&mut self, head_json: &str) -> Result<(), JsValue> {
        self.session
            .recenter_menu(decode(head_json, 1_024)?)
            .map_err(error)
    }

    /// Transports the retained menu after an explicit tracking recenter.
    pub fn recenter_tracking_json(&mut self, change_json: &str) -> Result<(), JsValue> {
        self.session
            .recenter_tracking(decode(change_json, 1_024)?)
            .map_err(error)
    }

    /// Closes the menu while retaining the presentation reference.
    pub fn close_menu(&mut self) {
        self.session.close_menu();
    }
}
