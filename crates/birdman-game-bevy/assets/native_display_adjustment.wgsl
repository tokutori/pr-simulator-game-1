#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct DisplayAdjustment {
    contrast: f32,
    saturation: f32,
    contrast_pivot: f32,
    padding: f32,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: DisplayAdjustment;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(screen_texture, texture_sampler, in.uv);
    var color = (sampled.rgb - settings.contrast_pivot) * settings.contrast
        + settings.contrast_pivot;
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, settings.saturation);
    return vec4<f32>(color, sampled.a);
}
