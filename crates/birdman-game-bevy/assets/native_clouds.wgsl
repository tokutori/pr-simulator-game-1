#import bevy_pbr::mesh_view_bindings::view

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var cloud_atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var cloud_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> haze: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var normal_atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> sun: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> shade_tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> sun_tint: vec4<f32>;

struct Vertex {
    @location(0) center: vec3<f32>,
    @location(1) offset: vec2<f32>,
    @location(2) uv: vec2<f32>,
}

struct CloudVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) distance: f32,
    @location(2) sunlight: vec3<f32>,
}

@vertex
fn vertex(input: Vertex) -> CloudVertex {
    let to_eye = view.world_from_view[3].xyz - input.center;
    var right = vec3<f32>(to_eye.z, 0.0, -to_eye.x);
    if length(right) > 0.0001 {
        right = normalize(right);
    } else {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    let world_point = input.center + right * input.offset.x + vec3<f32>(0.0, input.offset.y, 0.0);
    let center_view = view.view_from_world * vec4<f32>(world_point, 1.0);
    var output: CloudVertex;
    output.position = view.clip_from_view * center_view;
    output.uv = input.uv;
    output.distance = length(center_view.xyz);
    let front = vec3<f32>(-right.z, 0.0, right.x);
    output.sunlight = vec3<f32>(dot(sun.xyz, right), sun.y, dot(sun.xyz, front));
    return output;
}

@fragment
fn fragment(input: CloudVertex) -> @location(0) vec4<f32> {
    let cloud = textureSample(cloud_atlas, cloud_sampler, input.uv);
    let normal_xy = textureSample(normal_atlas, normal_sampler, input.uv).rg * 2.0 - 1.0;
    let normal = normalize(vec3<f32>(normal_xy, sqrt(max(1.0 - dot(normal_xy, normal_xy), 0.0))));
    let sunlight = clamp(dot(normal, input.sunlight), 0.0, 1.0) * sun.w;
    let color = cloud.rgb * mix(shade_tint.rgb, sun_tint.rgb, sunlight);
    let fog = smoothstep(haze.x, haze.y, input.distance);
    return vec4<f32>(color, cloud.a * (1.0 - fog));
}
