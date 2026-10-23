use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::{
    ApplicationGraphicsTarget, ApplicationRequest, ApplicationResourceCommit,
    ApplicationResourceObservation,
};
use birdman_game_app::environment_snapshot::{
    Defined, EnvironmentProjection, EnvironmentSelection, EnvironmentSnapshotError,
    EnvironmentVenueProjection,
};
use birdman_game_format::EnvironmentFormatError;
use birdman_game_presentation::screen_render_auto::ScreenRenderResourceRequest;
use birdman_game_scene::lake_sky::{
    LAKE_SCENE_LIGHTING, LakeSceneHaze, LakeSkyRenderCondition, LakeSunlight, generate_lake_sky,
};
use birdman_game_scene::lake_water::{
    LakeDetailConfig, LakeRenderModelError, LakeVisualCondition, LakeWaveProjection,
    generate_lake_detail, production_lake_detail_configs,
};
use serde::Serialize;

/// Pixel encoding submitted directly to a graphics API.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApplicationImageEncoding {
    /// Four unsigned bytes per texel, with sRGB colour channels.
    SrgbRgba8,
    /// Four unsigned bytes per texel, interpreted as linear data.
    LinearRgba8,
}

/// An immutable image descriptor; bytes travel through the common asset-buffer channel.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationImageAsset {
    /// Owner-issued buffer identity, stable while the same image is reused.
    pub buffer_id: u32,
    /// Texture dimensions in texels.
    pub size: [u32; 2],
    /// Pixel interpretation at upload.
    pub encoding: ApplicationImageEncoding,
}

/// One prepared periodic normal/variance/height image.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationWaterDetailAsset {
    /// Linear-data RGBA texture.
    pub image: ApplicationImageAsset,
    /// World-space repeat length in metres.
    pub extent_meters: f64,
}

/// Final display-light and distance-fog values in the shared render convention.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationLightingAssets {
    /// Ambient sRGB 0xRRGGBB colour.
    pub ambient_color_srgb: u32,
    /// Hemispherical irradiance scale.
    pub ambient_irradiance: f64,
    /// Sunlight sRGB 0xRRGGBB colour.
    pub sunlight_color_srgb: u32,
    /// Daylight-gated directional irradiance scale.
    pub sunlight_irradiance: f64,
    /// Render-world direction toward the sun.
    pub sunlight_direction: [f64; 3],
    /// Airlight sRGB 0xRRGGBB colour.
    pub haze_color_srgb: u32,
    /// Near/far distance-fog bounds in metres.
    pub haze_range_meters: [f64; 2],
}

/// Prepared wave uniforms and immutable detail images.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationWaterAssets {
    /// Complete final-axis water meshes and conservative culling bounds.
    pub geometry: crate::geometry::ApplicationWaterGeometry,
    /// Complete GPU wave coefficients, counts and non-inversion bounds.
    pub projection: LakeWaveProjection,
    /// Wind magnitude used by the shared visual condition.
    pub wind_speed_meters_per_second: f64,
    /// Downstream detail multiplier; zero denotes the flat-water case.
    pub detail_amplitude_scale: f64,
    /// Near/far periodic images when the wind supports detail generation.
    pub detail: Option<[ApplicationWaterDetailAsset; 2]>,
}

/// Revision-scoped drawing resources prepared by the shared application owner.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationEnvironmentAssets {
    /// First immutable scene manifest; later preparations reuse the same owner-issued identities.
    pub scene: Option<crate::ApplicationGeometryAssets>,
    /// The resource-preparation request owning this bundle.
    pub revision: u32,
    /// Equirectangular sRGB panorama.
    pub sky: ApplicationImageAsset,
    /// Final light and fog projection.
    pub lighting: ApplicationLightingAssets,
    /// Final water uniforms and data images.
    pub water: ApplicationWaterAssets,
    /// Shared registration decision for venue geometry.
    pub venue: EnvironmentVenueProjection,
}

/// Failures from registered environment resolution or immutable CPU resource generation.
#[derive(Debug)]
pub enum ApplicationAssetError {
    /// Registered mesh preparation or accepted-frame projection failed.
    Geometry(String),
    /// The registered environment could not be resolved.
    Environment(EnvironmentSnapshotError),
    /// The selected identity has no registered display environment.
    UnavailableEnvironment,
    /// Sky or distance-fog metadata is invalid.
    Sky(EnvironmentFormatError),
    /// Water resource generation rejected the recorded condition.
    Water(LakeRenderModelError),
    /// Buffer identities have been consumed.
    IdentifiersExhausted,
}

struct ImageBuffer {
    descriptor: ApplicationImageAsset,
    bytes: Vec<u8>,
}

struct SkyAsset {
    condition: Option<LakeSkyRenderCondition>,
    image: ImageBuffer,
}

struct DetailAssets {
    configuration: [LakeDetailConfig; 2],
    images: [ImageBuffer; 2],
}

struct PreparedAssets {
    venue_visible: bool,
    sky: Arc<SkyAsset>,
    detail: Option<Arc<DetailAssets>>,
}

enum PreparationStage {
    Building,
    Cancelling,
    Candidate(u32),
    Committing(u32),
}

struct PendingAssets {
    bundle: PreparedAssets,
    stage: PreparationStage,
    target: ApplicationGraphicsTarget,
    policy_request: Option<ScreenRenderResourceRequest>,
}

pub(super) struct AssetCommit {
    pub request_id: u32,
    pub target: ApplicationGraphicsTarget,
    pub policy_request: Option<ScreenRenderResourceRequest>,
    pub actual: ApplicationResourceCommit,
}

pub(super) struct AssetFailure {
    pub request_id: u32,
    pub target: ApplicationGraphicsTarget,
    pub policy_request: Option<ScreenRenderResourceRequest>,
    pub cause: String,
}

impl PendingAssets {
    fn failed(self, request_id: u32, cause: String) -> AssetFailure {
        AssetFailure {
            request_id,
            target: self.target,
            policy_request: self.policy_request,
            cause,
        }
    }
}

#[derive(Default)]
pub(super) struct AssetCompletion {
    pub invalidated_renderer: Option<u32>,
    pub committed: Option<AssetCommit>,
    pub failed: Option<AssetFailure>,
    pub requests: Vec<ApplicationRequest>,
}

#[derive(Default)]
pub(super) struct AssetsOwner {
    geometry: Option<Box<crate::geometry::GeometryOwner>>,
    geometry_renderer: Option<u32>,
    next_buffer_id: u32,
    active: Option<PreparedAssets>,
    active_resource: Option<u32>,
    desired: Option<u32>,
    committing: Option<u32>,
    pending: BTreeMap<u32, PendingAssets>,
    retired: BTreeSet<u32>,
}

impl AssetsOwner {
    pub(super) fn venue_visible(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.venue_visible)
    }
    pub(super) fn prepare(
        &mut self,
        revision: u32,
        selection: EnvironmentSelection,
        target: ApplicationGraphicsTarget,
        policy_request: Option<ScreenRenderResourceRequest>,
    ) -> Result<ApplicationEnvironmentAssets, ApplicationAssetError> {
        if self.geometry.is_none() {
            let geometry = crate::geometry::GeometryOwner::prepare(&mut self.next_buffer_id)
                .map_err(ApplicationAssetError::Geometry)?;
            self.geometry = Some(Box::new(geometry));
        }
        let scene = (self.geometry_renderer != Some(target.renderer_resource_id))
            .then(|| self.geometry.as_ref().unwrap().assets().clone());
        let quality = target.quality.water_quality;
        let EnvironmentProjection::Available {
            metadata, venue, ..
        } = selection
            .resolve()
            .map_err(ApplicationAssetError::Environment)?
        else {
            return Err(ApplicationAssetError::UnavailableEnvironment);
        };
        let sky_condition = match metadata.sky {
            Defined::Defined(sky) => {
                Some(LakeSkyRenderCondition::try_from(sky).map_err(ApplicationAssetError::Sky)?)
            }
            Defined::Unavailable => None,
        };
        let existing_sky = self
            .bundles()
            .find(|bundle| bundle.sky.condition == sky_condition)
            .map(|bundle| Arc::clone(&bundle.sky));
        let sky = match existing_sky {
            Some(sky) => sky,
            None => {
                let image = generate_lake_sky(sky_condition);
                let size = [image.width(), image.height()];
                Arc::new(SkyAsset {
                    condition: sky_condition,
                    image: self.image(
                        size,
                        ApplicationImageEncoding::SrgbRgba8,
                        image.into_rgba(),
                    )?,
                })
            }
        };
        let wave = metadata.waves;
        let condition = LakeVisualCondition::try_new(
            wave.wind_velocity_ne_mps[0],
            wave.wind_velocity_ne_mps[1],
            wave.fetch_m,
            wave.detail_amplitude_scale,
            u64::from(wave.pattern_seed),
        )
        .map_err(ApplicationAssetError::Water)?;
        let projection = LakeWaveProjection::try_new(condition, quality)
            .map_err(ApplicationAssetError::Water)?;
        let water_geometry = self
            .geometry
            .as_mut()
            .unwrap()
            .water(&mut self.next_buffer_id, quality, &projection)
            .map_err(ApplicationAssetError::Geometry)?;
        let wind_speed = wave.wind_velocity_ne_mps[0].hypot(wave.wind_velocity_ne_mps[1]);
        let detail = if wind_speed < 0.05 {
            None
        } else {
            let configuration =
                production_lake_detail_configs(condition).map_err(ApplicationAssetError::Water)?;
            let existing_detail = self
                .bundles()
                .filter_map(|bundle| bundle.detail.as_ref())
                .find(|detail| detail.configuration == configuration)
                .cloned();
            match existing_detail {
                Some(detail) => Some(detail),
                None => {
                    let mut images = Vec::with_capacity(2);
                    for config in configuration {
                        let image =
                            generate_lake_detail(config).map_err(ApplicationAssetError::Water)?;
                        let metadata = image.metadata();
                        let size = [metadata.width as u32, metadata.height as u32];
                        images.push(self.image(
                            size,
                            ApplicationImageEncoding::LinearRgba8,
                            image.into_rgba(),
                        )?);
                    }
                    Some(Arc::new(DetailAssets {
                        configuration,
                        images: images.try_into().unwrap_or_else(|_| unreachable!()),
                    }))
                }
            }
        };
        let sunlight = LakeSunlight::for_condition(sky_condition);
        let haze = LakeSceneHaze::try_new(sky_condition.map(LakeSkyRenderCondition::visibility_m))
            .map_err(ApplicationAssetError::Sky)?;
        let output = ApplicationEnvironmentAssets {
            scene,
            revision,
            sky: sky.image.descriptor.clone(),
            lighting: ApplicationLightingAssets {
                ambient_color_srgb: LAKE_SCENE_LIGHTING.ambient_color_srgb,
                ambient_irradiance: LAKE_SCENE_LIGHTING.ambient_irradiance,
                sunlight_color_srgb: LAKE_SCENE_LIGHTING.sunlight_color_srgb,
                sunlight_irradiance: if sunlight.daylight {
                    LAKE_SCENE_LIGHTING.sunlight_irradiance
                } else {
                    0.0
                },
                sunlight_direction: sunlight.direction,
                haze_color_srgb: haze.color_srgb,
                haze_range_meters: [haze.near_m, haze.far_m],
            },
            water: ApplicationWaterAssets {
                geometry: water_geometry,
                projection,
                wind_speed_meters_per_second: wind_speed,
                detail_amplitude_scale: if detail.is_some() {
                    wave.detail_amplitude_scale
                } else {
                    0.0
                },
                detail: detail.as_ref().map(|detail| {
                    std::array::from_fn(|index| ApplicationWaterDetailAsset {
                        image: detail.images[index].descriptor.clone(),
                        extent_meters: detail.configuration[index].extent_meters(),
                    })
                }),
            },
            venue,
        };
        self.pending.insert(
            revision,
            PendingAssets {
                bundle: PreparedAssets {
                    sky,
                    detail,
                    venue_visible: venue.is_visible(),
                },
                stage: PreparationStage::Building,
                target,
                policy_request,
            },
        );
        Ok(output)
    }

    pub(super) fn select(&mut self, desired: Option<u32>) -> Vec<ApplicationRequest> {
        self.desired = desired;
        let mut requests = Vec::new();
        let mut released = Vec::new();
        for (&request_id, pending) in &mut self.pending {
            if Some(request_id) == desired {
                continue;
            }
            match pending.stage {
                PreparationStage::Building => {
                    pending.stage = PreparationStage::Cancelling;
                    requests.push(ApplicationRequest::CancelResources { request_id });
                }
                PreparationStage::Candidate(resource_id) => {
                    requests.push(ApplicationRequest::ReleaseResources { resource_id });
                    released.push(request_id);
                }
                PreparationStage::Cancelling | PreparationStage::Committing(_) => {}
            }
        }
        for request_id in released {
            self.pending.remove(&request_id);
        }
        if desired.is_none()
            && let Some(resource_id) = self.active_resource.take()
        {
            requests.push(ApplicationRequest::ReleaseResources { resource_id });
        }
        self.schedule_commit(&mut requests);
        self.track_releases(&requests);
        requests
    }

    pub(super) fn observe(
        &mut self,
        request_id: u32,
        observation: ApplicationResourceObservation,
    ) -> AssetCompletion {
        let mut output = AssetCompletion::default();
        match observation {
            ApplicationResourceObservation::BindingLost { resource_id, cause } => {
                if self.committing == Some(request_id) && self.pending.get(&request_id).is_some_and(|pending| matches!(pending.stage, PreparationStage::Committing(id) if id == resource_id)) {
                    let pending = self.pending.remove(&request_id).unwrap();
                    self.committing = None;
                    output.invalidated_renderer = Some(pending.target.renderer_resource_id);
                    output.requests.push(ApplicationRequest::ReleaseResources { resource_id });
                    output.failed = Some(pending.failed(request_id, cause));
                }
            }
            ApplicationResourceObservation::Prepared { resource_id } => {
                let protected = self.retired.contains(&resource_id) || self.active_resource == Some(resource_id) || self.pending.values().any(|pending| matches!(pending.stage, PreparationStage::Candidate(id) | PreparationStage::Committing(id) if id == resource_id));
                if resource_id == 0 || protected {
                    if self.pending.get(&request_id).is_some_and(|pending| matches!(pending.stage, PreparationStage::Building)) {
                        let pending = self.pending.remove(&request_id).unwrap();
                        output.failed = Some(pending.failed(request_id, "描画資源の識別子が無効または重複している".to_owned()));
                    }
                    return output;
                }
                if let Some(pending) = self.pending.get_mut(&request_id) && matches!(pending.stage, PreparationStage::Building) && self.desired == Some(request_id) {
                    self.geometry_renderer = Some(pending.target.renderer_resource_id);
                    pending.stage = PreparationStage::Candidate(resource_id);
                } else {
                    output.requests.push(ApplicationRequest::ReleaseResources { resource_id });
                    if self.pending.get(&request_id).is_some_and(|pending| matches!(pending.stage, PreparationStage::Building | PreparationStage::Cancelling)) { self.pending.remove(&request_id); }
                }
            }
            ApplicationResourceObservation::Committed { resource_id, actual } => {
                if self.committing == Some(request_id) && self.pending.get(&request_id).is_some_and(|pending| matches!(pending.stage, PreparationStage::Committing(id) if id == resource_id)) {
                    let pending = self.pending.remove(&request_id).unwrap();
                    if let Some(geometry) = &mut self.geometry { geometry.rebind(); }
                    output.committed = Some(AssetCommit { request_id, target: pending.target, policy_request: pending.policy_request, actual });
                    self.active = Some(pending.bundle);
                    if let Some(previous) = self.active_resource.replace(resource_id) && previous != resource_id {
                        output.requests.push(ApplicationRequest::ReleaseResources { resource_id: previous });
                    }
                    self.committing = None;
                    if self.desired.is_none() {
                        self.active_resource = None;
                        output.requests.push(ApplicationRequest::ReleaseResources { resource_id });
                    }
                }
            }
            ApplicationResourceObservation::Failed { cause } => {
                if let Some(pending) = self.pending.remove(&request_id) {
                    if let PreparationStage::Candidate(resource_id) | PreparationStage::Committing(resource_id) = pending.stage {
                        output.requests.push(ApplicationRequest::ReleaseResources { resource_id });
                    }
                    if self.committing == Some(request_id) { self.committing = None; }
                    output.failed = Some(pending.failed(request_id, cause));
                }
            }
            ApplicationResourceObservation::Cancelled => {
                if self.pending.get(&request_id).is_some_and(|pending| matches!(pending.stage, PreparationStage::Building | PreparationStage::Cancelling)) {
                    let pending = self.pending.remove(&request_id).unwrap();
                    output.failed = Some(pending.failed(request_id, "描画資源の準備が中止された".to_owned()));
                }
            }
        }
        self.track_releases(&output.requests);
        output
    }

    pub(super) fn continue_commit(&mut self) -> Vec<ApplicationRequest> {
        let mut requests = Vec::new();
        self.schedule_commit(&mut requests);
        requests
    }

    pub(super) fn has_policy_request(
        &self,
        target: ApplicationGraphicsTarget,
        request: Option<ScreenRenderResourceRequest>,
    ) -> bool {
        self.pending.values().any(|pending| {
            pending.target.renderer_resource_id == target.renderer_resource_id
                && pending.policy_request == request
                && !matches!(pending.stage, PreparationStage::Cancelling)
        })
    }

    pub(super) fn released(&mut self, resource_id: u32, failed: bool) -> bool {
        if !self.retired.contains(&resource_id) {
            return false;
        }
        if !failed {
            self.retired.remove(&resource_id);
        }
        true
    }

    fn track_releases(&mut self, requests: &[ApplicationRequest]) {
        for request in requests {
            if let ApplicationRequest::ReleaseResources { resource_id } = request {
                self.retired.insert(*resource_id);
            }
        }
    }

    fn schedule_commit(&mut self, requests: &mut Vec<ApplicationRequest>) {
        if self.committing.is_some() {
            return;
        }
        if let Some(request_id) = self.desired
            && let Some(pending) = self.pending.get_mut(&request_id)
            && let PreparationStage::Candidate(resource_id) = pending.stage
        {
            pending.stage = PreparationStage::Committing(resource_id);
            self.committing = Some(request_id);
            requests.push(ApplicationRequest::CommitResources {
                request_id,
                resource_id,
            });
        }
    }

    pub(super) fn bytes(&self, buffer_id: u32) -> Option<&[u8]> {
        for bundle in self.bundles() {
            if bundle.sky.image.descriptor.buffer_id == buffer_id {
                return Some(&bundle.sky.image.bytes);
            }
            if let Some(detail) = &bundle.detail {
                for image in &detail.images {
                    if image.descriptor.buffer_id == buffer_id {
                        return Some(&image.bytes);
                    }
                }
            }
        }
        None
    }

    pub(super) fn buffer(&self, buffer_id: u32) -> Option<crate::ApplicationBufferSlice<'_>> {
        self.geometry
            .as_ref()
            .and_then(|geometry| geometry.buffer(buffer_id))
            .or_else(|| self.bytes(buffer_id).map(crate::ApplicationBufferSlice::U8))
    }

    pub(super) fn project_geometry(
        &mut self,
        pose: Option<birdman_game_presentation::render_pose::FlightRenderPose>,
    ) -> Result<Option<crate::ApplicationGeometryFrame>, ApplicationAssetError> {
        self.geometry
            .as_mut()
            .map(|geometry| {
                geometry
                    .project(pose)
                    .map_err(ApplicationAssetError::Geometry)
            })
            .transpose()
    }

    fn bundles(&self) -> impl Iterator<Item = &PreparedAssets> {
        self.active
            .iter()
            .chain(self.pending.values().map(|pending| &pending.bundle))
    }

    fn image(
        &mut self,
        size: [u32; 2],
        encoding: ApplicationImageEncoding,
        bytes: Vec<u8>,
    ) -> Result<ImageBuffer, ApplicationAssetError> {
        self.next_buffer_id = self
            .next_buffer_id
            .checked_add(1)
            .ok_or(ApplicationAssetError::IdentifiersExhausted)?;
        Ok(ImageBuffer {
            descriptor: ApplicationImageAsset {
                buffer_id: self.next_buffer_id,
                size,
                encoding,
            },
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use birdman_game_app::{HybridSessionPreparation, identity_for_selection};
    use birdman_game_format::{WaterQuality, WeatherClass};

    fn target(quality: WaterQuality) -> ApplicationGraphicsTarget {
        ApplicationGraphicsTarget {
            renderer_resource_id: 100,
            quality: crate::ApplicationRenderQuality {
                scale: birdman_game_presentation::screen_render_scale::ScreenRenderScale::Full,
                shadow: birdman_game_presentation::screen_render_auto::ScreenShadowMap::Full,
                water_quality: quality,
            },
            extent_px: [1280, 720],
            shadow_extent_px: [2048, 2048],
            reflection_extent_px: [640, 360],
        }
    }

    fn commit_observation(
        resource_id: u32,
        quality: WaterQuality,
    ) -> ApplicationResourceObservation {
        let target = target(quality);
        ApplicationResourceObservation::Committed {
            resource_id,
            actual: ApplicationResourceCommit {
                applied: target.quality,
                extent_px: target.extent_px,
            },
        }
    }

    #[test]
    fn immutable_images_survive_serial_commits_and_obsolete_candidates_are_released() {
        let exhibition = EnvironmentSelection::exhibition().unwrap();
        let mut owner = AssetsOwner::default();
        let first = owner
            .prepare(1, exhibition, target(WaterQuality::High), None)
            .unwrap();
        let sky_id = first.sky.buffer_id;
        let scene = first.scene.as_ref().unwrap();
        assert!(!scene.meshes.is_empty());
        assert_eq!(first.water.geometry.near.len(), 16);
        let position = first.water.geometry.near[0].position;
        let crate::ApplicationBufferSlice::F32(values) = owner.buffer(position.buffer_id).unwrap()
        else {
            panic!("Expected native Float32 attributes");
        };
        assert_eq!(
            values.len(),
            position.count as usize * position.components as usize
        );
        assert_eq!(
            &crate::ApplicationBufferSlice::F32(values).to_bytes()[..4],
            &values[0].to_le_bytes()
        );
        let detail_id = first.water.detail.as_ref().unwrap()[0].image.buffer_id;
        assert_eq!(
            owner.bytes(sky_id).unwrap().len(),
            first.sky.size[0] as usize * first.sky.size[1] as usize * 4
        );
        assert!(owner.select(Some(1)).is_empty());
        let prepared = owner.observe(
            1,
            ApplicationResourceObservation::Prepared { resource_id: 101 },
        );
        assert!(prepared.requests.is_empty());
        assert!(matches!(
            owner.continue_commit().as_slice(),
            [ApplicationRequest::CommitResources {
                request_id: 1,
                resource_id: 101
            }]
        ));
        let second = owner
            .prepare(2, exhibition, target(WaterQuality::Low), None)
            .unwrap();
        assert!(second.scene.is_none());
        assert_eq!(second.sky.buffer_id, sky_id);
        assert_eq!(
            second.water.detail.as_ref().unwrap()[0].image.buffer_id,
            detail_id
        );
        assert!(second.water.projection.wave_count() < first.water.projection.wave_count());
        assert!(owner.select(Some(2)).is_empty());
        assert!(
            owner
                .observe(
                    2,
                    ApplicationResourceObservation::Prepared { resource_id: 102 }
                )
                .requests
                .is_empty()
        );
        let committed = owner.observe(1, commit_observation(101, WaterQuality::High));
        assert!(committed.committed.is_some());
        assert!(matches!(
            owner.continue_commit().as_slice(),
            [ApplicationRequest::CommitResources {
                request_id: 2,
                resource_id: 102
            }]
        ));
        let committed = owner.observe(2, commit_observation(102, WaterQuality::Low));
        assert!(matches!(
            committed.requests.as_slice(),
            [ApplicationRequest::ReleaseResources { resource_id: 101 }]
        ));
        assert!(owner.bytes(sky_id).is_some());
        let selection = HybridSessionPreparation::select_scenario(
            WeatherClass::Calm,
            birdman_game_app::DEFAULT_SESSION_SEED,
        )
        .unwrap();
        let calm = EnvironmentSelection {
            identity: identity_for_selection(selection).into(),
            ..exhibition
        };
        let third = owner
            .prepare(3, calm, target(WaterQuality::Low), None)
            .unwrap();
        let abandoned_id = third.water.detail.as_ref().unwrap()[0].image.buffer_id;
        owner.select(Some(3));
        let cancelled = owner.select(None);
        assert!(cancelled.iter().any(|request| matches!(
            request,
            ApplicationRequest::CancelResources { request_id: 3 }
        )));
        assert!(owner.bytes(abandoned_id).is_some());
        let late = owner.observe(
            3,
            ApplicationResourceObservation::Prepared { resource_id: 103 },
        );
        assert!(matches!(
            late.requests.as_slice(),
            [ApplicationRequest::ReleaseResources { resource_id: 103 }]
        ));
        assert!(owner.bytes(abandoned_id).is_none());
        assert!(owner.bytes(detail_id).is_some());
        let mut replacement = target(WaterQuality::Low);
        replacement.renderer_resource_id += 1;
        assert!(
            owner
                .prepare(4, exhibition, replacement, None)
                .unwrap()
                .scene
                .is_some()
        );
        owner.select(Some(4));
        owner.observe(
            4,
            ApplicationResourceObservation::Prepared { resource_id: 104 },
        );
        assert_eq!(owner.continue_commit().len(), 1);
        let stale = owner.observe(
            4,
            ApplicationResourceObservation::BindingLost {
                resource_id: 105,
                cause: "stale bind".to_owned(),
            },
        );
        assert!(stale.invalidated_renderer.is_none());
        let lost = owner.observe(
            4,
            ApplicationResourceObservation::BindingLost {
                resource_id: 104,
                cause: "bind and rollback failed".to_owned(),
            },
        );
        assert_eq!(lost.invalidated_renderer, Some(101));
        assert_eq!(lost.failed.unwrap().cause, "bind and rollback failed");
        assert!(matches!(
            lost.requests.as_slice(),
            [ApplicationRequest::ReleaseResources { resource_id: 104 }]
        ));
    }
}
