#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput},
    mesh_functions,
    view_transformations::position_world_to_clip,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> camera_time: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> sun_cloud: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> waves_sky: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> geometry_patch: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> geometry_waves: array<vec4<f32>, 4>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> geometry_motion: array<vec4<f32>, 4>;

struct SurfaceProjection {
    offset: vec3<f32>,
    tangent_x: vec3<f32>,
    tangent_z: vec3<f32>,
}

fn patch_fade(coordinate: f32) -> vec2<f32> {
    let width = geometry_patch.w - geometry_patch.z;
    let progress = clamp((abs(coordinate) - geometry_patch.z) / width, 0.0, 1.0);
    let weight = 1.0 - progress * progress * (3.0 - 2.0 * progress);
    let gradient = -6.0 * progress * (1.0 - progress) * sign(coordinate) / width;
    return vec2<f32>(weight, gradient);
}

fn lake_geometry(position: vec2<f32>) -> SurfaceProjection {
    var surface: SurfaceProjection;
    surface.offset = vec3<f32>(0.0);
    surface.tangent_x = vec3<f32>(1.0, 0.0, 0.0);
    surface.tangent_z = vec3<f32>(0.0, 0.0, 1.0);
    if geometry_patch.w <= 0.0 { return surface; }
    let local_position = position - geometry_patch.xy;
    let fade_x = patch_fade(local_position.x);
    let fade_z = patch_fade(local_position.y);
    let fade = fade_x.x * fade_z.x;
    let fade_gradient = vec2<f32>(fade_x.y * fade_z.x, fade_x.x * fade_z.y);
    for (var component: u32 = 0u; component < 4u; component += 1u) {
        let wave = geometry_waves[component];
        let motion = geometry_motion[component];
        let phase = wave.z * dot(wave.xy, position) - motion.x * camera_time.w + motion.y;
        let phase_sine = sin(phase);
        let phase_cosine = cos(phase);
        let displacement = wave.w * vec3<f32>(
            motion.z * wave.x * phase_cosine,
            phase_sine,
            motion.z * wave.y * phase_cosine
        );
        let phase_derivative = wave.w * vec3<f32>(
            -motion.z * wave.x * phase_sine,
            phase_cosine,
            -motion.z * wave.y * phase_sine
        );
        surface.offset += displacement * fade;
        surface.tangent_x += phase_derivative * (wave.z * wave.x * fade)
            + displacement * fade_gradient.x;
        surface.tangent_z += phase_derivative * (wave.z * wave.y * fade)
            + displacement * fade_gradient.y;
    }
    return surface;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var output: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let base_position = mesh_functions::mesh_position_local_to_world(
        world_from_local, vec4<f32>(vertex.position, 1.0)
    );
    output.world_position = base_position;
    output.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
    if waves_sky.w < 0.5 {
        let surface = lake_geometry(base_position.xz);
        output.world_position = vec4<f32>(base_position.xyz + surface.offset, 1.0);
        output.world_normal = normalize(cross(surface.tangent_z, surface.tangent_x));
    }
    output.position = position_world_to_clip(output.world_position.xyz);
#ifdef VERTEX_UVS_A
    output.uv = base_position.xz;
#endif
#ifdef VERTEX_UVS_B
    output.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    output.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local, vertex.tangent, vertex.instance_index
    );
#endif
#ifdef VERTEX_COLORS
    output.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    output.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    output.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]
    );
#endif
    return output;
}

fn sky_background(direction: vec3<f32>) -> vec3<f32> {
    let horizon = vec3<f32>(0.68, 0.80, 0.88);
    let zenith = vec3<f32>(0.13, 0.38, 0.67);
    var color = mix(horizon, zenith, pow(clamp(direction.y, 0.0, 1.0), 0.45));
    let cloud = smoothstep(0.3, 0.7, sin(direction.x * 15.0 + direction.z * 9.0) * sin(direction.z * 19.0 - direction.x * 5.0) * 0.5 + 0.5);
    color = mix(color, vec3<f32>(0.91, 0.94, 0.95), cloud * sun_cloud.w * smoothstep(0.08, 0.35, direction.y));
    return color;
}

fn sky_color(direction: vec3<f32>) -> vec3<f32> {
    return sky_background(direction) + vec3<f32>(1.0, 0.84, 0.58) * pow(max(dot(direction, sun_cloud.xyz), 0.0), 900.0);
}

fn wave_visibility(phase_footprint: f32) -> f32 {
    return 1.0 - smoothstep(0.65, 2.4, phase_footprint);
}

fn lake_wave_detail(position: vec2<f32>, footprint_x: vec2<f32>, footprint_y: vec2<f32>) -> vec3<f32> {
    let bands = array<vec4<f32>, 8>(
        vec4<f32>(-0.57, 0.86, 0.027, 0.73),
        vec4<f32>(0.43, 1.41, 0.029, 2.19),
        vec4<f32>(-0.18, 2.31, 0.026, 4.61),
        vec4<f32>(0.72, 3.73, 0.023, 1.37),
        vec4<f32>(-0.86, 6.04, 0.020, 5.83),
        vec4<f32>(0.21, 9.81, 0.016, 3.47),
        vec4<f32>(1.04, 15.86, 0.012, 0.31),
        vec4<f32>(-0.39, 25.70, 0.009, 4.03)
    );
    let wind_speed = length(waves_sky.xy);
    let wind_direction = waves_sky.xy / max(wind_speed, 0.001);
    let propagation = select(vec2<f32>(1.0, 0.0), wind_direction, wind_speed > 0.001);
    let strength = smoothstep(0.05, 0.6, wind_speed) * waves_sky.z;
    var slope = vec2<f32>(0.0);
    var unresolved_variance = 0.0;
    for (var component: u32 = 0u; component < 8u; component += 1u) {
        let band = bands[component];
        let direction = vec2<f32>(
            propagation.x * cos(band.x) - propagation.y * sin(band.x),
            propagation.x * sin(band.x) + propagation.y * cos(band.x)
        );
        let transverse = vec2<f32>(-direction.y, direction.x);
        let packet_gradient = transverse * 0.13 + direction * 0.037;
        let packet_phase = dot(position, packet_gradient) - camera_time.w * 0.18 + band.w;
        let packet = 0.72 + 0.28 * sin(packet_phase);
        let amplitude_gradient = packet_gradient * (0.28 * cos(packet_phase));
        let warp_gradient = transverse * (band.y * 0.19);
        let warp_phase = dot(position, warp_gradient) - camera_time.w * 0.12 + band.w * 1.73;
        let phase_gradient = direction * band.y + warp_gradient * (0.55 * cos(warp_phase));
        let phase = dot(position, direction) * band.y + 0.55 * sin(warp_phase)
            - sqrt(9.80665 * band.y) * camera_time.w + band.w;
        let footprint = abs(dot(phase_gradient, footprint_x)) + abs(dot(phase_gradient, footprint_y));
        let visibility = wave_visibility(footprint);
        let amplitude = band.z * strength / band.y;
        slope += amplitude * (packet * cos(phase) * phase_gradient + sin(phase) * amplitude_gradient) * visibility;
        let slope_energy = band.z * strength * packet;
        unresolved_variance += 0.5 * slope_energy * slope_energy * (1.0 - visibility * visibility);
    }
    return vec3<f32>(slope, unresolved_variance);
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let view = normalize(camera_time.xyz - mesh.world_position.xyz);
#ifdef VERTEX_UVS_A
    let position = mesh.uv;
#else
    let position = mesh.world_position.xz;
#endif
    let footprint_x = dpdx(position);
    let footprint_y = dpdy(position);
    if waves_sky.w > 0.5 { return vec4<f32>(sky_color(-view), 1.0); }
    let detail = lake_wave_detail(position, footprint_x, footprint_y);
    let geometric_normal = normalize(mesh.world_normal);
    let tangent_x = normalize(vec3<f32>(geometric_normal.y, -geometric_normal.x, 0.0));
    let tangent_z = cross(tangent_x, geometric_normal);
    let normal = normalize(geometric_normal - detail.x * tangent_x - detail.y * tangent_z);
    let geometric_ndv = clamp(dot(view, geometric_normal), 0.0, 1.0);
    let facet_ndv = dot(view, normal);
    let facet_visibility = smoothstep(0.0, 0.18, facet_ndv);
    let reflected = mix(reflect(-view, geometric_normal), reflect(-view, normal), facet_visibility);
    let reflection = normalize(vec3<f32>(reflected.x, max(reflected.y, 0.0), reflected.z));
    let geometric_fresnel = 0.02 + 0.98 * pow(1.0 - geometric_ndv, 5.0);
    let facet_fresnel = 0.02 + 0.98 * pow(1.0 - clamp(facet_ndv, 0.0, 1.0), 5.0);
    let fresnel = mix(geometric_fresnel, facet_fresnel, 0.35 * facet_visibility * smoothstep(0.03, 0.22, geometric_ndv));
    let roughness = clamp(0.24 + length(waves_sky.xy) * 0.018 + sqrt(detail.z) * 3.0, 0.24, 0.55);
    let reflected_sky = mix(sky_background(reflection), sky_background(normalize(reflection + vec3<f32>(0.0, 0.65, 0.0))), roughness * 0.4);
    let sun_visibility = 1.0 - sun_cloud.w * 0.65;
    let diffuse = 0.42 + 0.38 * max(dot(normal, sun_cloud.xyz), 0.0) * sun_visibility;
    let base = mix(vec3<f32>(0.027, 0.057, 0.046), vec3<f32>(0.058, 0.112, 0.089), diffuse);
    let sun = pow(max(dot(reflection, sun_cloud.xyz), 0.0), mix(180.0, 48.0, roughness));
    let glitter = sun * sun_visibility * 0.16 * facet_visibility;
    let color = mix(base, reflected_sky, fresnel * 0.45) + vec3<f32>(1.0, 0.82, 0.58) * glitter;
    return vec4<f32>(color, 1.0);
}
