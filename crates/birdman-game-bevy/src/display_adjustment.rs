use bevy::{
    prelude::*,
    render::{render_resource::RenderPipelineDescriptor, view::ColorGrading},
    shader::ShaderDefVal,
};
use birdman_game_app::display_settings::DisplayAdjustment;

/// Transfers the completed linear transform into Bevy's existing per-view
/// exposure and ASC slope/offset. Equal sections make the transform independent
/// of luminance. Scene materials read these values; UI materials do not.
pub(crate) fn color_grading(adjustment: &DisplayAdjustment) -> ColorGrading {
    let mut grading = ColorGrading::default();
    grading.global.exposure = -adjustment.exposure_ev_shift as f32;
    grading.global.post_saturation = adjustment.saturation as f32;
    for section in grading.all_sections_mut() {
        section.gain = adjustment.contrast as f32;
        section.lift = (adjustment.contrast_pivot * (1.0 - adjustment.contrast)) as f32;
    }
    grading
}

pub(crate) fn submit(world: &mut World, camera: Entity, adjustment: &DisplayAdjustment) {
    let grading = color_grading(adjustment);
    if world.get::<ColorGrading>(camera).is_none_or(|current| {
        current.global.exposure != grading.global.exposure
            || current.global.post_saturation != grading.global.post_saturation
            || current.shadows != grading.shadows
            || current.midtones != grading.midtones
            || current.highlights != grading.highlights
    }) {
        world.entity_mut(camera).insert(grading);
    }
}

/// Non-HDR PBR pipelines inline Bevy's grading even for Tonemapping::None.
/// Scene shaders own that stage and UI materials retain their supplied ink;
/// fog and alpha handling remain in main_pass_post_lighting_processing.
pub(crate) fn disable_inline_tonemapping(descriptor: &mut RenderPipelineDescriptor) {
    if let Some(fragment) = &mut descriptor.fragment {
        fragment.shader_defs.retain(|definition| {
            !matches!(definition, ShaderDefVal::Bool(name, _) if name == "TONEMAP_IN_SHADER")
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::view::ColorGradingUniform;
    use birdman_game_app::display_settings::DisplaySettings;

    #[test]
    fn view_uniform_retains_the_shared_linear_transform_including_negative_channels() {
        for settings in [
            DisplaySettings::default(),
            DisplaySettings {
                brightness: 0.5,
                contrast: 1.5,
                saturation: 0.75,
                ..Default::default()
            },
        ] {
            let adjustment = settings.adjustment();
            let packed = ColorGradingUniform::from(color_grading(&adjustment));
            assert_eq!(packed.gain.x, packed.gain.y);
            assert_eq!(packed.gain.x, packed.gain.z);
            assert_eq!(packed.lift.x, packed.lift.y);
            assert_eq!(packed.lift.x, packed.lift.z);
            let color = [-0.1_f64, 0.4, 2.0];
            let transformed =
                color.map(|c| c as f32 * packed.exposure.exp2() * packed.gain.x + packed.lift.x);
            let luminance =
                transformed[0] * 0.2126 + transformed[1] * 0.7152 + transformed[2] * 0.0722;
            for (value, expected) in transformed
                .map(|c| luminance + (c - luminance) * packed.post_saturation)
                .into_iter()
                .zip(adjustment.apply_linear_rgb(color))
            {
                assert!((f64::from(value) - expected).abs() < 1e-6);
            }
        }
    }
}
