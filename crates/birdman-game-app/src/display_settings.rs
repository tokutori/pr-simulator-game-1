//! Display adjustment selection retained independently of any flight.

use serde::{Deserialize, Serialize};

/// Lowest admitted value of every display adjustment item.
pub const DISPLAY_SETTING_MINIMUM: f64 = 0.5;
/// Highest admitted value of every display adjustment item.
pub const DISPLAY_SETTING_MAXIMUM: f64 = 1.5;
/// Discrete choices offered for every display adjustment item.
pub const DISPLAY_SETTING_STEPS: [f64; 5] = [0.5, 0.75, 1.0, 1.25, 1.5];
/// Discrete choices offered for the UI scale; 1.0 keeps the standard size.
pub const UI_SCALE_STEPS: [f64; 6] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

fn default_ui_scale() -> f64 {
    1.0
}
/// Linear middle gray that anchors the shared contrast adjustment.
pub const DISPLAY_CONTRAST_PIVOT: f64 = 0.18;

/// Display adjustment selection applied identically to every surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DisplaySettings {
    /// Exposure multiplier of the 3D scene; 1.0 keeps the existing brightness.
    pub brightness: f64,
    /// Contrast factor around linear middle gray; 1.0 keeps the existing contrast.
    pub contrast: f64,
    /// Saturation mix from linear luminance; 1.0 keeps the existing saturation.
    pub saturation: f64,
    /// Scale of every UI text and layout size relative to the platform root
    /// font; stored settings written before this field existed read as 1.0.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f64,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            ui_scale: 1.0,
        }
    }
}

impl DisplaySettings {
    /// Validates the complete selection before it can replace the retained settings.
    pub fn is_valid(self) -> bool {
        [self.brightness, self.contrast, self.saturation]
            .into_iter()
            .all(|value| {
                value.is_finite()
                    && (DISPLAY_SETTING_MINIMUM..=DISPLAY_SETTING_MAXIMUM).contains(&value)
            })
            && UI_SCALE_STEPS.contains(&self.ui_scale)
    }

    /// Derives the single color adjustment definition shared by every adapter.
    pub fn adjustment(self) -> DisplayAdjustment {
        DisplayAdjustment {
            exposure_multiplier: self.brightness,
            exposure_ev_shift: -self.brightness.log2(),
            contrast: self.contrast,
            contrast_pivot: DISPLAY_CONTRAST_PIVOT,
            saturation: self.saturation,
        }
    }
}

/// Color adjustment parameters derived once from the retained display settings.
///
/// Every adapter applies the same formula in linear RGB at its tone-mapping
/// stage: exposure multiplication, contrast around the middle-gray pivot, then
/// a saturation mix from linear luminance. The default settings derive the
/// identity adjustment.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DisplayAdjustment {
    /// Linear multiplier applied before tone mapping; equals the brightness setting.
    pub exposure_multiplier: f64,
    /// Exposure shift in EV100 stops; an exposure-based adapter adds this to its base EV.
    pub exposure_ev_shift: f64,
    /// Contrast factor around the pivot.
    pub contrast: f64,
    /// Linear middle gray that anchors the contrast factor.
    pub contrast_pivot: f64,
    /// Saturation mix factor from linear luminance.
    pub saturation: f64,
}

impl Default for DisplayAdjustment {
    fn default() -> Self {
        DisplaySettings::default().adjustment()
    }
}

impl DisplayAdjustment {
    /// Applies the shared adjustment formula to one linear RGB color.
    pub fn apply_linear_rgb(self, rgb: [f64; 3]) -> [f64; 3] {
        let exposed = rgb.map(|channel| channel * self.exposure_multiplier);
        let contrasted = exposed
            .map(|channel| (channel - self.contrast_pivot) * self.contrast + self.contrast_pivot);
        let luminance = 0.2126 * contrasted[0] + 0.7152 * contrasted[1] + 0.0722 * contrasted[2];
        contrasted.map(|channel| luminance + (channel - luminance) * self.saturation)
    }
}

#[cfg(test)]
mod tests;
