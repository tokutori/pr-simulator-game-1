use super::{Verification, VerificationStage, WaterVerificationCase};
use crate::app::native_runtime::NativeRuntime;
use bevy::{
    prelude::{Commands, Image, On, ResMut},
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};
use birdman_game_core::{
    FlightRecordFinalization, FlightRecordHeader, FlightRecordSample, SessionEndReason,
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

/// Accepts an already acknowledged quality, including a no-op selection of that quality.
pub(super) fn quality_matches(
    graphics: &birdman_game_runtime::ApplicationGraphicsProjection,
    renderer: &birdman_game_runtime::ApplicationRendererObservation,
    expected: birdman_game_runtime::ApplicationRenderQuality,
) -> bool {
    renderer.screen_extent_px.into_iter().all(|size| size > 0)
        && graphics.renderer_resource_id == Some(renderer.resource_id)
        && graphics.resource.is_some_and(|resource| {
            resource.renderer_resource_id == renderer.resource_id
                && resource.commit > 0
                && resource.quality == expected
                && resource.extent_px == renderer.extent_px
                && resource.extent_px
                    == expected
                        .scale
                        .physical_size(renderer.screen_extent_px[0], renderer.screen_extent_px[1])
        })
}

pub(crate) struct VerificationCompletion(Receiver<Result<(), String>>);

impl VerificationCompletion {
    pub(super) fn channel() -> (Sender<Result<(), String>>, Self) {
        let (sender, receiver) = mpsc::channel();
        (sender, Self(receiver))
    }

    pub(crate) fn ensure_completed(&self) -> Result<(), String> {
        self.0.try_recv().map_err(|error| {
            format!("Verification ended before completed GPU captures and core checks: {error}")
        })?
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CaptureIdentity {
    pub(super) stage: VerificationStage,
    pub(super) water_case: Option<WaterVerificationCase>,
    pub(super) serial: u64,
}

#[derive(Debug)]
pub(super) struct SavedCapture {
    pub(super) path: PathBuf,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) struct ReplayCaptureRecord {
    header: FlightRecordHeader,
    samples: Vec<FlightRecordSample>,
    finalization: FlightRecordFinalization,
}

impl ReplayCaptureRecord {
    pub(super) fn retain(session: &NativeRuntime) -> Result<Self, String> {
        let record = session
            .0
            .application()
            .core_session()
            .flight_record()
            .ok_or("Replay camera capture requires a retained flight record")?;
        let finalization = record
            .finalization()
            .ok_or("Replay camera capture requires a finalized record")?;
        if finalization.reason != SessionEndReason::WaterContact || finalization.failure.is_some() {
            return Err(
                "Replay camera capture requires natural WaterContact without failure".into(),
            );
        }
        Ok(Self {
            header: record.header(),
            samples: record.samples().to_vec(),
            finalization,
        })
    }

    pub(super) fn verify(&self, session: &NativeRuntime) -> Result<(), String> {
        let record = session
            .0
            .application()
            .core_session()
            .flight_record()
            .ok_or("Replay camera capture lost the retained record")?;
        if record.header() != self.header
            || record.samples() != self.samples
            || record.finalization() != Some(self.finalization)
        {
            return Err(
                "Replay camera selection changed record samples, identity or finalization".into(),
            );
        }
        Ok(())
    }

    pub(super) fn midpoint(&self) -> f64 {
        (self.finalization.terminal_tick as f64 + self.finalization.terminal_fraction)
            / f64::from(self.header.physics_hz)
            * 0.5
    }
}

pub(super) struct CaptureCompletion {
    pub(super) identity: CaptureIdentity,
    pub(super) result: Result<SavedCapture, String>,
}

pub(super) enum CaptureState {
    Idle,
    Requested(CaptureIdentity),
    Saving {
        identity: CaptureIdentity,
        task: Task<CaptureCompletion>,
    },
    Completed(CaptureCompletion),
}

impl CaptureState {
    pub(super) fn pending(&self) -> bool {
        !matches!(self, Self::Idle)
    }

    pub(super) fn accepts(&self, request: CaptureIdentity, current: CaptureIdentity) -> bool {
        matches!(self, Self::Requested(expected) if *expected == request && current == request)
    }

    pub(super) fn poll(
        &mut self,
        current: CaptureIdentity,
    ) -> Result<Option<SavedCapture>, String> {
        if let Self::Saving { identity, task } = self
            && let Some(completion) = check_ready(task)
        {
            let expected = *identity;
            *self = Self::Completed(completion);
            if expected != current {
                return Err("Capture owner changed before saving completed".into());
            }
        }
        if !matches!(self, Self::Completed(_)) {
            return Ok(None);
        }
        let Self::Completed(completion) = std::mem::replace(self, Self::Idle) else {
            unreachable!()
        };
        if completion.identity != current {
            return Err("Stale capture completion does not match case/stage/serial".into());
        }
        completion.result.map(Some)
    }
}

pub(super) fn verify_title_water_roi(width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    if pixels.len() != width as usize * height as usize * 3 {
        return Err("Title water ROI requires a complete RGB8 image".into());
    }
    if width < 640 || height < 480 || width as f64 / (height as f64) < 1.2 {
        println!(
            "Title water ROI skipped outside the fixed landscape layout ({width}x{height}); float shader probes remain required"
        );
        return Ok(());
    }
    let mut sum = 0.0;
    let mut count = 0;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for row in height * 86 / 100..height * 96 / 100 {
        for column in width * 3 / 100..width * 25 / 100 {
            let offset = ((row * width + column) * 3) as usize;
            let intensity = pixels[offset..offset + 3]
                .iter()
                .map(|&channel| f64::from(channel))
                .sum::<f64>()
                / 3.0;
            minimum = minimum.min(intensity);
            maximum = maximum.max(intensity);
            sum += intensity;
            count += 1;
        }
    }
    let mean = sum / f64::from(count) / 255.0;
    if !(0.005..0.75).contains(&mean) || maximum - minimum < 3.0 {
        return Err(format!(
            "Title production water ROI is black, clipped or lacks visible surface variation: mean={mean:.6}, range={minimum}..{maximum}"
        ));
    }
    println!("Title production water ROI passed: mean={mean:.6}, range={minimum}..{maximum}");
    Ok(())
}

pub(super) fn save_capture(
    identity: CaptureIdentity,
    image: Image,
    path: PathBuf,
) -> CaptureCompletion {
    let result = image
        .try_into_dynamic()
        .map_err(|cause| format!("Screenshot conversion: {cause}"))
        .and_then(|image| {
            let image = image.to_rgb8();
            if image.width() == 0 || image.height() == 0 {
                return Err("Screenshot has zero dimensions".into());
            }
            if identity.stage == VerificationStage::Title && identity.water_case.is_none() {
                verify_title_water_roi(image.width(), image.height(), image.as_raw())?;
            }
            image
                .save(&path)
                .map_err(|cause| format!("Screenshot {}: {cause}", path.display()))?;
            Ok(SavedCapture {
                path,
                width: image.width(),
                height: image.height(),
            })
        });
    CaptureCompletion { identity, result }
}

impl Verification {
    pub(super) fn capture(&mut self, commands: &mut Commands, filename: &str) {
        let path = self.directory.join(filename);
        self.capture_serial += 1;
        let identity = self.capture_identity();
        self.capture = CaptureState::Requested(identity);
        let requested_at = std::time::Instant::now();
        commands
            .spawn(Screenshot(self.capture_target.clone()))
            .observe(
                move |capture: On<ScreenshotCaptured>, mut verification: ResMut<Verification>| {
                    if verification.failure.is_some()
                        || !verification
                            .capture
                            .accepts(identity, verification.capture_identity())
                    {
                        return;
                    }
                    println!(
                        "Native capture readback: serial={} latency_ms={:.3}",
                        identity.serial,
                        requested_at.elapsed().as_secs_f64() * 1000.0
                    );
                    let image = capture.image.clone();
                    let path = path.clone();
                    let task = AsyncComputeTaskPool::get().spawn(async move {
                        let saving_at = std::time::Instant::now();
                        let result = save_capture(identity, image, path);
                        println!(
                            "Native capture save: serial={} duration_ms={:.3}",
                            identity.serial,
                            saving_at.elapsed().as_secs_f64() * 1000.0
                        );
                        result
                    });
                    verification.capture = CaptureState::Saving { identity, task };
                },
            );
    }

    pub(super) fn capture_identity(&self) -> CaptureIdentity {
        CaptureIdentity {
            stage: self.stage,
            water_case: self.water_case,
            serial: self.capture_serial,
        }
    }
}
