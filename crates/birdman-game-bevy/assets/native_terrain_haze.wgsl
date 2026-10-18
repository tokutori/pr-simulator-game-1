#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    view_transformations::position_world_to_view,
}
#import bevy_render::color_operations::{linear_rgb_to_srgb, srgb_to_linear_rgb}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> haze_color_srgb: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> haze_range: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    let depth = -position_world_to_view(in.world_position.xyz).z;
    let amount = smoothstep(haze_range.x, haze_range.y, depth);
    let fogged = mix(linear_rgb_to_srgb(out.color.rgb), haze_color_srgb.rgb, amount);
    out.color = vec4<f32>(srgb_to_linear_rgb(fogged), out.color.a);
    return out;
}
