use super::*;

#[test]
fn defaults_match_the_existing_presentation() {
    let settings = DisplaySettings::default();
    assert_eq!(settings.brightness, 1.0);
    assert_eq!(settings.contrast, 1.0);
    assert_eq!(settings.saturation, 1.0);
    assert!(settings.is_valid());
}

#[test]
fn validation_admits_only_the_documented_range() {
    for value in [DISPLAY_SETTING_MINIMUM, 1.0, DISPLAY_SETTING_MAXIMUM] {
        assert!(
            DisplaySettings {
                brightness: value,
                contrast: value,
                saturation: value,
                ..Default::default()
            }
            .is_valid()
        );
    }
    for value in [0.0, 0.49, 1.51, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            !DisplaySettings {
                brightness: value,
                ..Default::default()
            }
            .is_valid()
        );
        assert!(
            !DisplaySettings {
                contrast: value,
                ..Default::default()
            }
            .is_valid()
        );
        assert!(
            !DisplaySettings {
                saturation: value,
                ..Default::default()
            }
            .is_valid()
        );
    }
}

#[test]
fn every_offered_step_is_valid() {
    for step in DISPLAY_SETTING_STEPS {
        assert!(
            DisplaySettings {
                brightness: step,
                contrast: step,
                saturation: step,
                ..Default::default()
            }
            .is_valid()
        );
    }
}

#[test]
fn ui_scale_admits_only_the_offered_steps() {
    for step in UI_SCALE_STEPS {
        assert!(
            DisplaySettings {
                ui_scale: step,
                ..Default::default()
            }
            .is_valid()
        );
    }
    for value in [0.5, 0.9, 1.1, 2.5, f64::NAN, f64::INFINITY] {
        assert!(
            !DisplaySettings {
                ui_scale: value,
                ..Default::default()
            }
            .is_valid()
        );
    }
}

#[test]
fn stored_settings_without_ui_scale_read_as_the_standard_size() {
    let settings: DisplaySettings =
        serde_json::from_str(r#"{"brightness":1.25,"contrast":1.0,"saturation":0.75}"#)
            .expect("legacy settings decode");
    assert_eq!(settings.ui_scale, 1.0);
    assert!(settings.is_valid());
}

#[test]
fn default_adjustment_is_the_identity() {
    let adjustment = DisplaySettings::default().adjustment();
    assert_eq!(adjustment.exposure_multiplier, 1.0);
    assert_eq!(adjustment.exposure_ev_shift, 0.0);
    assert_eq!(adjustment.contrast, 1.0);
    assert_eq!(adjustment.contrast_pivot, DISPLAY_CONTRAST_PIVOT);
    assert_eq!(adjustment.saturation, 1.0);
    for rgb in [
        [0.0, 0.0, 0.0],
        [0.18, 0.18, 0.18],
        [0.25, 0.5, 0.75],
        [1.0, 1.0, 1.0],
        [2.5, 0.01, 0.33],
    ] {
        let applied = adjustment.apply_linear_rgb(rgb);
        for (channel, original) in applied.into_iter().zip(rgb) {
            assert!(
                (channel - original).abs() < 1e-12,
                "{channel} != {original}"
            );
        }
    }
}

#[test]
fn adjustment_applies_exposure_then_contrast_then_saturation() {
    let settings = DisplaySettings {
        brightness: 1.5,
        contrast: 1.25,
        saturation: 0.5,
        ..Default::default()
    };
    let adjustment = settings.adjustment();
    assert_eq!(adjustment.exposure_multiplier, 1.5);
    assert!((adjustment.exposure_ev_shift - -1.5_f64.log2()).abs() < 1e-12);
    // Exposure first: 0.4 * 1.5 = 0.6 for every channel of this gray input.
    // Contrast then saturation keep a gray color gray at its contrasted value.
    let gray = adjustment.apply_linear_rgb([0.4, 0.4, 0.4]);
    let expected = (0.6 - DISPLAY_CONTRAST_PIVOT) * 1.25 + DISPLAY_CONTRAST_PIVOT;
    for channel in gray {
        assert!((channel - expected).abs() < 1e-12);
    }

    let color = adjustment.apply_linear_rgb([0.4, 0.2, 0.0]);
    let exposed = [0.6, 0.3, 0.0];
    let contrasted =
        exposed.map(|channel| (channel - DISPLAY_CONTRAST_PIVOT) * 1.25 + DISPLAY_CONTRAST_PIVOT);
    let luminance = 0.2126 * contrasted[0] + 0.7152 * contrasted[1] + 0.0722 * contrasted[2];
    let expected = contrasted.map(|channel| luminance + (channel - luminance) * 0.5);
    for (channel, expected) in color.into_iter().zip(expected) {
        assert!((channel - expected).abs() < 1e-12);
    }
}

#[test]
fn minimum_saturation_mixes_halfway_to_linear_luminance() {
    let adjustment = DisplaySettings {
        saturation: 0.5,
        ..Default::default()
    }
    .adjustment();
    let rgb = adjustment.apply_linear_rgb([0.2, 0.4, 0.6]);
    let luminance = 0.2126 * 0.2 + 0.7152 * 0.4 + 0.0722 * 0.6;
    for (channel, original) in rgb.into_iter().zip([0.2, 0.4, 0.6]) {
        assert!((channel - (luminance + (original - luminance) * 0.5)).abs() < 1e-12);
    }
}

#[test]
fn ev_shift_halves_and_doubles_with_brightness() {
    for (brightness, shift) in [(0.5, 1.0), (1.0, 0.0), (1.5, -0.584_962_500_721_156_2)] {
        let adjustment = DisplaySettings {
            brightness,
            ..Default::default()
        }
        .adjustment();
        assert!((adjustment.exposure_ev_shift - shift).abs() < 1e-12);
        // An exposure-based adapter multiplying by 2^(-shift) recovers the multiplier.
        assert!((2.0_f64.powf(-adjustment.exposure_ev_shift) - brightness).abs() < 1e-12);
    }
}
