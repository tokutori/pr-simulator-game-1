use super::{
    CaptureState, MOTION_DURATION, MotionEndpoint, VerificationCompletion, VerificationStage,
    WaterMotion, WaterVerificationCase,
    capture::{CaptureCompletion, CaptureIdentity, save_capture},
};
use bevy::prelude::{Image, Quat, Transform, Vec3};
use std::time::Duration;

fn timing_diagnostic(
    start: std::time::Instant,
    capacity: usize,
    samples: &[(u64, f64)],
) -> bevy::diagnostic::Diagnostic {
    use bevy::diagnostic::{Diagnostic, DiagnosticMeasurement, FrameTimeDiagnosticsPlugin};
    let mut diagnostic =
        Diagnostic::new(FrameTimeDiagnosticsPlugin::FRAME_TIME).with_max_history_length(capacity);
    for &(milliseconds, value) in samples {
        diagnostic.add_measurement(DiagnosticMeasurement {
            time: start + Duration::from_millis(milliseconds),
            value,
        });
    }
    diagnostic
}

#[test]
fn bundled_font_has_actual_cjk_and_mixed_script_glyph_layout() {
    super::ui_validation::verify_text_layout(crate::app::native_assets::bundled_font()).unwrap();
}

mod capture;
mod timing;
