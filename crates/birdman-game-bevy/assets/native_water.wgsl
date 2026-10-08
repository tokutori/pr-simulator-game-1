#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> camera_time: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> sun_cloud: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> waves_sky: vec4<f32>;

fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    let horizon = vec3<f32>(0.68, 0.80, 0.88);
    let zenith = vec3<f32>(0.13, 0.38, 0.67);
    var color = mix(horizon, zenith, pow(clamp(direction.y, 0.0, 1.0), 0.45));
    let cloud = smoothstep(0.3, 0.7, sin(direction.x * 15.0 + direction.z * 9.0) * sin(direction.z * 19.0 - direction.x * 5.0) * 0.5 + 0.5);
    color = mix(color, vec3<f32>(0.91, 0.94, 0.95), cloud * sun_cloud.w * smoothstep(0.08, 0.35, direction.y));
    return color + vec3<f32>(1.0, 0.84, 0.58) * pow(max(dot(direction, sun_cloud.xyz), 0.0), 900.0);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let view = normalize(camera_time.xyz - mesh.world_position.xyz);
    if waves_sky.w > 0.5 { return vec4<f32>(sky_color(-view), 1.0); }
    let position = mesh.world_position.xz;
    let drift = waves_sky.xy * camera_time.w * 0.08;
    let coarse = position + drift;
    let fine = position - drift * 0.7;
    let slope_x = (cos(dot(coarse, vec2<f32>(0.7, 0.25))) * 0.10 + cos(dot(fine, vec2<f32>(3.5, -1.9))) * 0.018) * waves_sky.z;
    let slope_z = (cos(dot(coarse, vec2<f32>(0.35, 0.8))) * 0.10 + cos(dot(fine, vec2<f32>(-2.1, 4.3))) * 0.018) * waves_sky.z;
    let normal = normalize(vec3<f32>(-slope_x, 1.0, -slope_z));
    let reflection = reflect(-view, normal);
    let fresnel = 0.035 + 0.965 * pow(1.0 - max(dot(view, normal), 0.0), 5.0);
    let deep = vec3<f32>(0.025, 0.13, 0.16);
    let ripple = 0.025 * sin(dot(fine, vec2<f32>(5.0, 3.0)));
    let sun = pow(max(dot(reflection, sun_cloud.xyz), 0.0), 250.0);
    return vec4<f32>(mix(deep + ripple, sky_color(reflection), fresnel) + vec3<f32>(1.0, 0.86, 0.58) * sun, 1.0);
}
