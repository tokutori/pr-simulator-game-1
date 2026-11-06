#define_import_path birdman_game::display_adjustment

#import bevy_render::view::ColorGrading

// Every luminance section receives the same shared ASC slope and offset.
// The reflection view stays at identity and both presenting eyes use the same
// transform through their existing view uniforms.
fn adjust_scene_color(color: vec3<f32>, grading: ColorGrading) -> vec3<f32> {
    let exposed = color * exp2(grading.exposure);
    let contrasted = exposed * grading.gain.x + grading.lift.x;
    let luminance = dot(contrasted, vec3<f32>(0.2126, 0.7152, 0.0722));
    return mix(vec3<f32>(luminance), contrasted, grading.post_saturation);
}
