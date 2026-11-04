use bevy::ecs as bevy_ecs;
use bevy::render as bevy_render;
use bevy::{
    core_pipeline::fullscreen_material::FullscreenMaterial,
    prelude::Component,
    render::{
        extract_component::ExtractComponent,
        render_resource::{ShaderType, encase},
    },
    shader::ShaderRef,
};
use birdman_game_app::display_settings::DisplayAdjustment;

/// Native fullscreen-pass parameters for contrast and saturation.
///
/// Brightness rides the camera `Exposure` instead: Bevy applies exposure in
/// its tonemapping pass after this pre-tonemapping pass, so the contrast
/// pivot is divided by the exposure multiplier to reproduce the shared
/// derivation order (exposure, then contrast, then saturation) exactly.
#[derive(Component, Clone, Copy, Debug, PartialEq, ShaderType)]
pub(crate) struct NativeDisplayAdjustment {
    pub(crate) contrast: f32,
    pub(crate) saturation: f32,
    pub(crate) contrast_pivot: f32,
    pub(crate) padding: f32,
}

impl Default for NativeDisplayAdjustment {
    fn default() -> Self {
        Self {
            contrast: 1.0,
            saturation: 1.0,
            contrast_pivot: birdman_game_app::display_settings::DISPLAY_CONTRAST_PIVOT as f32,
            padding: 0.0,
        }
    }
}

impl NativeDisplayAdjustment {
    pub(crate) fn from_shared(adjustment: &DisplayAdjustment) -> Self {
        Self {
            contrast: adjustment.contrast as f32,
            saturation: adjustment.saturation as f32,
            contrast_pivot: (adjustment.contrast_pivot / adjustment.exposure_multiplier) as f32,
            padding: 0.0,
        }
    }
}

// The ExtractComponent derive cannot resolve `bevy_ecs` from this crate's
// manifest, so extraction is written out like the XR surface order component.
impl bevy_render::sync_component::SyncComponent for NativeDisplayAdjustment {
    type Target = Self;
}

impl ExtractComponent for NativeDisplayAdjustment {
    type QueryData = &'static Self;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(
        item: bevy_ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(*item)
    }
}

impl FullscreenMaterial for NativeDisplayAdjustment {
    fn fragment_shader() -> ShaderRef {
        super::native_assets::display_adjustment_shader_path().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_identity_derivation_maps_to_the_identity_pass() {
        assert_eq!(
            NativeDisplayAdjustment::from_shared(&DisplayAdjustment::default()),
            NativeDisplayAdjustment::default()
        );
    }

    #[test]
    fn contrast_pivot_is_compensated_for_the_later_exposure_pass() {
        let adjustment = DisplayAdjustment {
            exposure_multiplier: 0.5,
            contrast: 0.75,
            saturation: 1.5,
            ..DisplayAdjustment::default()
        };
        let pass = NativeDisplayAdjustment::from_shared(&adjustment);
        assert_eq!(pass.contrast, 0.75);
        assert_eq!(pass.saturation, 1.5);
        assert_eq!(pass.contrast_pivot, 0.36);
    }
}
