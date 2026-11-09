//! Owned pure scene and image generation, separate from canonical cache admission and buffer identities.
use birdman_game_scene::{
    lake_sky::{LakeSkyRenderCondition, generate_lake_sky_while},
    lake_water::{LakeDetailConfig, generate_lake_detail_while},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct RawImage {
    pub size: [u32; 2],
    pub bytes: Vec<u8>,
}
pub(super) struct PreparedGeometry {
    pub identity: [u8; 32],
    pub source: Arc<birdman_game_scene::prepared_scene::PreparedScene>,
    pub frame: birdman_game_scene::prepared_scene::PreparedAirframeFrame,
}
pub(super) struct GeneratedAssets {
    pub geometry: Option<PreparedGeometry>,
    pub sky: Option<(LakeSkyRenderCondition, RawImage)>,
    pub detail: Option<([LakeDetailConfig; 2], [RawImage; 2])>,
}
/// A unique, bounded CPU asset job sharing only immutable validated source data.
pub struct ApplicationAssetPreparationWork {
    id: u64,
    sky: Option<LakeSkyRenderCondition>,
    detail: Option<[LakeDetailConfig; 2]>,
    geometry: Option<([u8; 32], Arc<birdman_game_content::AircraftDefinition>)>,
    cancelled: Arc<AtomicBool>,
}
/// Completed unnumbered scene and image data admitted only under the original job scope.
pub struct ApplicationAssetPreparationResult {
    pub(super) id: u64,
    pub(super) prepared: Result<GeneratedAssets, String>,
}
impl ApplicationAssetPreparationWork {
    pub(super) fn new(
        id: u64,
        sky: Option<LakeSkyRenderCondition>,
        detail: Option<[LakeDetailConfig; 2]>,
        geometry: Option<([u8; 32], Arc<birdman_game_content::AircraftDefinition>)>,
    ) -> Self {
        Self {
            id,
            sky,
            detail,
            geometry,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub(super) fn cancellation(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
    /// Original job identity for raw executor diagnostics and failed execution completion.
    pub fn id(&self) -> u64 {
        self.id
    }
    /// Runs the shared generators; either cancellation flag retires every partial candidate allocation.
    pub fn execute(self, stop: &AtomicBool) -> ApplicationAssetPreparationResult {
        let mut keep_running =
            || !self.cancelled.load(Ordering::Acquire) && !stop.load(Ordering::Acquire);
        let prepared = (|| {
            if !keep_running() {
                return Err("Image preparation cancelled".to_owned());
            }
            let geometry = if let Some((identity, definition)) = self.geometry {
                let source = birdman_game_scene::prepared_scene::PreparedScene::prepare_while(
                    &definition,
                    &mut keep_running,
                )
                .map_err(|cause| cause.to_string())?
                .ok_or_else(|| "Scene preparation cancelled".to_owned())?;
                if !keep_running() {
                    return Err("Scene preparation cancelled".to_owned());
                }
                let frame = source
                    .frame(
                        &definition,
                        birdman_game_core::WingBendingFactor::try_new(0.0)
                            .expect("neutral bending factor"),
                    )
                    .map_err(|cause| format!("Scene frame preparation failed: {cause:?}"))?;
                Some(PreparedGeometry {
                    identity,
                    source: Arc::new(source),
                    frame,
                })
            } else {
                None
            };
            let sky = if let Some(condition) = self.sky {
                let image = generate_lake_sky_while(condition, &mut keep_running)
                    .ok_or_else(|| "Image preparation cancelled".to_owned())?;
                Some((
                    condition,
                    RawImage {
                        size: [image.width(), image.height()],
                        bytes: image.into_rgba(),
                    },
                ))
            } else {
                None
            };
            let detail = if let Some(configs) = self.detail {
                let mut images = Vec::with_capacity(2);
                for config in configs {
                    let image = generate_lake_detail_while(config, &mut keep_running)
                        .map_err(|cause| format!("Image preparation failed: {cause:?}"))?
                        .ok_or_else(|| "Image preparation cancelled".to_owned())?;
                    let metadata = image.metadata();
                    images.push(RawImage {
                        size: [metadata.width as u32, metadata.height as u32],
                        bytes: image.into_rgba(),
                    });
                }
                Some((
                    configs,
                    images
                        .try_into()
                        .unwrap_or_else(|_| unreachable!("two detail layers")),
                ))
            } else {
                None
            };
            Ok(GeneratedAssets {
                sky,
                detail,
                geometry,
            })
        })();
        ApplicationAssetPreparationResult {
            id: self.id,
            prepared,
        }
    }
}
impl ApplicationAssetPreparationResult {
    /// Reports an actual executor failure without constructing or admitting assets.
    pub fn failed(id: u64, cause: String) -> Self {
        Self {
            id,
            prepared: Err(cause),
        }
    }
}
