//! Owned pure image generation, separate from canonical cache admission and buffer identities.
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
pub(super) struct GeneratedImages {
    pub sky: Option<(LakeSkyRenderCondition, RawImage)>,
    pub detail: Option<([LakeDetailConfig; 2], [RawImage; 2])>,
}
/// A unique, bounded CPU image job containing no application, cache or buffer allocator.
pub struct ApplicationAssetPreparationWork {
    id: u64,
    sky: Option<LakeSkyRenderCondition>,
    detail: Option<[LakeDetailConfig; 2]>,
    cancelled: Arc<AtomicBool>,
}
/// Completed unnumbered pixels; the server admits them only under the original job scope.
pub struct ApplicationAssetPreparationResult {
    pub(super) id: u64,
    pub(super) images: Result<GeneratedImages, String>,
}
impl ApplicationAssetPreparationWork {
    pub(super) fn new(
        id: u64,
        sky: Option<LakeSkyRenderCondition>,
        detail: Option<[LakeDetailConfig; 2]>,
    ) -> Self {
        Self {
            id,
            sky,
            detail,
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
    /// Runs the shared generators; either cancellation flag retires every partial pixel allocation.
    pub fn execute(self, stop: &AtomicBool) -> ApplicationAssetPreparationResult {
        let mut keep_running =
            || !self.cancelled.load(Ordering::Acquire) && !stop.load(Ordering::Acquire);
        let images = (|| {
            if !keep_running() {
                return Err("Image preparation cancelled".to_owned());
            }
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
            Ok(GeneratedImages { sky, detail })
        })();
        ApplicationAssetPreparationResult {
            id: self.id,
            images,
        }
    }
}
impl ApplicationAssetPreparationResult {
    /// Reports an actual executor failure without constructing or admitting image bytes.
    pub fn failed(id: u64, cause: String) -> Self {
        Self {
            id,
            images: Err(cause),
        }
    }
}
