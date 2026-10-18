use birdman_game_format::SkyStateDocument;
use birdman_game_session::lake_sky::{
    LAKE_SCENE_LIGHTING, LakeCloudAvailability, LakeClouds, LakeSceneHaze, LakeSkyImage,
    LakeSkyRenderCondition, generate_lake_clouds, generate_lake_sky,
};
use wasm_bindgen::prelude::*;

/// Returns the shared ambient sRGB colour as 0xRRGGBB.
#[wasm_bindgen]
pub fn lake_scene_ambient_color_srgb() -> u32 {
    LAKE_SCENE_LIGHTING.ambient_color_srgb
}

/// Returns the shared Web-reference hemispherical irradiance scale.
#[wasm_bindgen]
pub fn lake_scene_ambient_irradiance() -> f64 {
    LAKE_SCENE_LIGHTING.ambient_irradiance
}

/// Returns the shared directional sunlight sRGB colour as 0xRRGGBB.
#[wasm_bindgen]
pub fn lake_scene_sunlight_color_srgb() -> u32 {
    LAKE_SCENE_LIGHTING.sunlight_color_srgb
}

/// Returns the shared daylight directional irradiance scale.
#[wasm_bindgen]
pub fn lake_scene_sunlight_irradiance() -> f64 {
    LAKE_SCENE_LIGHTING.sunlight_irradiance
}

/// Copies shared airlight 0xRRGGBB and near/far metres without constructing a sky owner.
#[wasm_bindgen]
pub fn lake_scene_haze(visibility_m: Option<f64>) -> Result<Vec<f64>, JsValue> {
    let haze = LakeSceneHaze::try_new(visibility_m)
        .map_err(|cause| JsValue::from_str(&format!("lake scene haze: {cause:?}")))?;
    Ok(vec![f64::from(haze.color_srgb), haze.near_m, haze.far_m])
}

/// Owns a shared sky panorama until the adapter copies its sRGB RGBA bytes.
#[wasm_bindgen]
pub struct LakeSkyImageBridge {
    image: LakeSkyImage,
}

/// Owns the shared finite cloud mesh and atlas until the adapter copies them.
#[wasm_bindgen]
pub struct LakeCloudsBridge {
    clouds: LakeClouds,
}

#[wasm_bindgen]
impl LakeCloudsBridge {
    /// Uses the recorded cloud-base height above the fixed water surface.
    #[wasm_bindgen(constructor)]
    pub fn new(
        sun_azimuth_degrees: f64,
        sun_elevation_degrees: f64,
        cloud_fraction: f64,
        visibility_m: f64,
        cloud_base_m: f64,
    ) -> Result<Self, JsValue> {
        generate_lake_clouds(Some(SkyStateDocument {
            sun_azimuth_degrees,
            sun_elevation_degrees,
            cloud_fraction,
            visibility_m,
            cloud_base_m,
        }))
        .map(|clouds| Self { clouds })
        .map_err(|cause| JsValue::from_str(&format!("lake cloud model: {cause:?}")))
    }

    /// Keeps unrecorded sky separate from a recorded clear sky.
    pub fn unrecorded() -> Self {
        Self {
            clouds: generate_lake_clouds(None).expect("Absent cloud metadata is valid"),
        }
    }

    /// Copies center N/E/D, right/up offset, and atlas U/V for each vertex.
    pub fn vertices(&self) -> Vec<f32> {
        self.clouds.vertices().iter().flatten().copied().collect()
    }

    /// Copies triangle indices into the actual vertex packet.
    pub fn indices(&self) -> Vec<u32> {
        self.clouds.indices().to_vec()
    }

    /// Copies bottom-row-first straight-alpha sRGB RGBA atlas pixels.
    pub fn rgba(&self) -> Vec<u8> {
        self.clouds.rgba().to_vec()
    }

    /// Copies the linear RG8 normal atlas, sharing color-atlas dimensions and UVs.
    pub fn normal_rg(&self) -> Vec<u8> {
        self.clouds.normal_rg().to_vec()
    }

    /// Copies N/E/D toward the recorded sun and W=daylight as one float4.
    pub fn sun_direction_ned(&self) -> Vec<f32> {
        self.clouds.sun_direction_ned().to_vec()
    }

    /// Returns the atlas width.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.clouds.width()
    }

    /// Returns the atlas height.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.clouds.height()
    }

    /// Returns the explicit reason for presence or absence of elevated cumulus.
    #[wasm_bindgen(getter)]
    pub fn availability(&self) -> String {
        match self.clouds.availability() {
            LakeCloudAvailability::Unrecorded => "unrecorded",
            LakeCloudAvailability::Clear => "clear",
            LakeCloudAvailability::GroundLevelUnsupported => "ground_level_unsupported",
            LakeCloudAvailability::Elevated => "elevated",
        }
        .to_owned()
    }
}

#[wasm_bindgen]
impl LakeSkyImageBridge {
    /// Generates the Web-reference panorama from four actual render parameters.
    #[wasm_bindgen(constructor)]
    pub fn new(
        sun_azimuth_degrees: f64,
        sun_elevation_degrees: f64,
        cloud_fraction: f64,
        visibility_m: f64,
    ) -> Result<Self, JsValue> {
        let condition = LakeSkyRenderCondition::try_new(
            sun_azimuth_degrees,
            sun_elevation_degrees,
            cloud_fraction,
            visibility_m,
        )
        .map_err(|cause| JsValue::from_str(&format!("lake sky render model: {cause:?}")))?;
        Ok(Self {
            image: generate_lake_sky(Some(condition)),
        })
    }

    /// Generates the neutral panorama when no sky metadata was recorded.
    pub fn neutral() -> Self {
        Self {
            image: generate_lake_sky(None),
        }
    }

    /// Copies row-major sRGB RGBA into an owned JavaScript Uint8Array.
    pub fn rgba(&self) -> Vec<u8> {
        self.image.rgba().to_vec()
    }

    /// Returns the panorama width.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Returns the panorama height.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.image.height()
    }
}
