//! Screen-only rendering resolution, independent of logical UI and simulation state.

use serde::{Deserialize, Serialize};

/// Supported manual Screen rendering scales, with full resolution as the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ScreenRenderScale {
    /// Original physical rendering resolution.
    #[default]
    #[serde(rename = "100")]
    Full,
    /// Three quarters of each physical rendering dimension.
    #[serde(rename = "75")]
    ThreeQuarters,
    /// Half of each physical rendering dimension.
    #[serde(rename = "50")]
    Half,
}

impl ScreenRenderScale {
    /// Shared choice order used by every Screen frontend.
    pub const ALL: [Self; 3] = [Self::Full, Self::ThreeQuarters, Self::Half];

    /// Percentage of each original physical dimension.
    pub const fn percent(self) -> u32 {
        match self {
            Self::Full => 100,
            Self::ThreeQuarters => 75,
            Self::Half => 50,
        }
    }

    /// Exact binary-representable multiplier for backend APIs.
    pub const fn factor(self) -> f64 {
        self.percent() as f64 / 100.0
    }

    /// Shared user-facing label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Full => "100%",
            Self::ThreeQuarters => "75%",
            Self::Half => "50%",
        }
    }

    /// Floors physical pixel dimensions and retains at least one pixel per axis.
    pub const fn physical_size(self, width: u32, height: u32) -> [u32; 2] {
        [
            self.physical_dimension(width),
            self.physical_dimension(height),
        ]
    }

    const fn physical_dimension(self, original: u32) -> u32 {
        let scaled = original as u64 * self.percent() as u64 / 100;
        if scaled == 0 { 1 } else { scaled as u32 }
    }
}

#[cfg(test)]
mod tests {
    use super::ScreenRenderScale;

    #[test]
    fn choices_and_serialization_define_the_same_scale_contract() {
        assert_eq!(ScreenRenderScale::default(), ScreenRenderScale::Full);
        for (scale, json, percent, factor, label) in [
            (ScreenRenderScale::Full, "\"100\"", 100, 1.0, "100%"),
            (ScreenRenderScale::ThreeQuarters, "\"75\"", 75, 0.75, "75%"),
            (ScreenRenderScale::Half, "\"50\"", 50, 0.5, "50%"),
        ] {
            assert!(ScreenRenderScale::ALL.contains(&scale));
            assert_eq!(serde_json::to_string(&scale).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<ScreenRenderScale>(json).unwrap(),
                scale
            );
            assert_eq!(scale.percent(), percent);
            assert_eq!(scale.factor(), factor);
            assert_eq!(scale.label(), label);
        }
        for invalid in ["75", "\"0\"", "\"25\"", "\"74\"", "\"101\"", "null"] {
            assert!(serde_json::from_str::<ScreenRenderScale>(invalid).is_err());
        }
    }

    #[test]
    fn physical_sizes_floor_odd_extents_and_handle_zero_without_overflow() {
        for (scale, ordinary, odd, maximum) in [
            (ScreenRenderScale::Full, [1280, 720], [1919, 1079], u32::MAX),
            (
                ScreenRenderScale::ThreeQuarters,
                [960, 540],
                [1439, 809],
                3_221_225_471,
            ),
            (
                ScreenRenderScale::Half,
                [640, 360],
                [959, 539],
                2_147_483_647,
            ),
        ] {
            assert_eq!(scale.physical_size(1280, 720), ordinary);
            assert_eq!(scale.physical_size(1919, 1079), odd);
            assert_eq!(scale.physical_size(0, 0), [1, 1]);
            assert_eq!(scale.physical_size(1, 1), [1, 1]);
            assert_eq!(scale.physical_size(u32::MAX, u32::MAX), [maximum, maximum]);
        }
    }
}
