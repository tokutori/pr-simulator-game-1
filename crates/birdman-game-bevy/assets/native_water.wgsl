#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput},
    mesh_functions,
    view_transformations::position_world_to_clip,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> camera_time: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> sun_cloud: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> waves_sky: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> geometry_patch: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> geometry_waves: array<vec4<f32>, 24>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<uniform> geometry_motion: array<vec4<f32>, 24>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> geometry_parameters: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var detail_near: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var detail_near_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var detail_far: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var detail_far_sampler: sampler;

struct SurfaceProjection {
    offset: vec3<f32>,
    tangent_x: vec3<f32>,
    tangent_z: vec3<f32>,
    represented_slope: vec2<f32>,
    crest: f32,
    compression: f32,
}

fn lake_wave_hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn lake_value_noise(point: vec2<f32>) -> vec3<f32> {
    let cell = floor(point);
    let fraction = fract(point);
    let blend = fraction * fraction * (3.0 - 2.0 * fraction);
    let blend_gradient = 6.0 * fraction * (1.0 - fraction);
    let lower_left = lake_wave_hash(cell);
    let lower_right = lake_wave_hash(cell + vec2<f32>(1.0, 0.0));
    let upper_left = lake_wave_hash(cell + vec2<f32>(0.0, 1.0));
    let upper_right = lake_wave_hash(cell + vec2<f32>(1.0, 1.0));
    let value = mix(mix(lower_left, lower_right, blend.x), mix(upper_left, upper_right, blend.x), blend.y);
    let gradient = vec2<f32>(
        blend_gradient.x * mix(lower_right - lower_left, upper_right - upper_left, blend.y),
        blend_gradient.y * mix(upper_left - lower_left, upper_right - lower_right, blend.x)
    );
    return vec3<f32>(value * 2.0 - 1.0, gradient * 2.0);
}

fn lake_wave_modulation(point: vec2<f32>, direction: vec2<f32>, wave_number: f32, seed: f32) -> mat2x3<f32> {
    let across = vec2<f32>(-direction.y, direction.x);
    let frequency = max(0.11, wave_number * 0.055);
    let coordinate = vec2<f32>(dot(point, across), dot(point, direction)) * frequency + vec2<f32>(seed * 1.7, seed * 2.3);
    let second_coordinate = vec2<f32>(coordinate.x * 1.7 + coordinate.y * 0.37,
        coordinate.y * 1.9 - coordinate.x * 0.24) + vec2<f32>(13.7, -8.2);
    let broad = lake_value_noise(coordinate);
    let fine = lake_value_noise(second_coordinate);
    let broad_gradient = frequency * (across * broad.y + direction * broad.z);
    let fine_gradient = frequency * (fine.y * (across * 1.7 + direction * 0.37)
        + fine.z * (across * -0.24 + direction * 1.9));
    let packet = 1.0 + 0.38 * broad.x + 0.12 * fine.x;
    let packet_gradient = 0.38 * broad_gradient + 0.12 * fine_gradient;
    let offset = 4.6 * broad.x + 1.4 * fine.x;
    let offset_gradient = 4.6 * broad_gradient + 1.4 * fine_gradient;
    return mat2x3<f32>(vec3<f32>(packet, packet_gradient), vec3<f32>(offset, offset_gradient));
}

fn grid_spacing_gradient(local: vec2<f32>) -> vec2<f32> {
    let magnitude = geometry_parameters.w;
    if abs(local.x) > abs(local.y) { return vec2<f32>(sign(local.x) * magnitude, 0.0); }
    if abs(local.y) > abs(local.x) { return vec2<f32>(0.0, sign(local.y) * magnitude); }
    return sign(local) * (0.5 * magnitude);
}

fn detail_samples(position: vec2<f32>) -> mat2x4<f32> {
    let drift = waves_sky.xy * camera_time.w;
    let near_sample = textureSampleLevel(detail_near, detail_near_sampler, (position - drift * 0.42) / 64.0, 0.0);
    let far_sample = textureSampleLevel(detail_far, detail_far_sampler, (position - drift * 0.89) / 288.0, 0.0);
    return mat2x4<f32>(near_sample, far_sample);
}

fn lake_geometry(position: vec2<f32>, spacing: f32) -> SurfaceProjection {
    var surface: SurfaceProjection;
    surface.offset = vec3<f32>(0.0);
    surface.tangent_x = vec3<f32>(1.0, 0.0, 0.0);
    surface.tangent_z = vec3<f32>(0.0, 0.0, 1.0);
    surface.represented_slope = vec2<f32>(0.0);
    surface.crest = 0.0;
    surface.compression = 0.0;
    if geometry_patch.w <= 0.0 { return surface; }
    let local = position - geometry_patch.xy;
    let spacing_gradient = grid_spacing_gradient(local);
    let details = detail_samples(position);
    let near_visibility = 1.0 - smoothstep(0.12, 0.32, spacing);
    let far_visibility = 1.0 - smoothstep(0.8, 2.8, spacing);
    let near_slope = (details[0].xy * 255.0 - 128.0) / 127.0;
    let far_slope = (details[1].xy * 255.0 - 128.0) / 127.0;
    let resolved_detail_slope = (near_slope * (0.6 * near_visibility)
        + far_slope * (0.55 * far_visibility)) * waves_sky.z;
    surface.offset.y = ((details[0].w - 128.0 / 255.0) * 0.6 * near_visibility
        + (details[1].w - 128.0 / 255.0) * 0.55 * far_visibility) * waves_sky.z;
    var horizontal_jacobian = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    var height_gradient = resolved_detail_slope;
    var crest = 0.0;
    var slope_energy = 0.0;
    for (var component: u32 = 0u; component < 24u; component += 1u) {
        if component < u32(geometry_parameters.x) {
            let wave = geometry_waves[component];
            let timing = geometry_motion[component];
            let visibility = 1.0 - smoothstep(1.3, 2.5, wave.z * spacing);
            let transition = clamp((wave.z * spacing - 1.3) / 1.2, 0.0, 1.0);
            let visibility_gradient = (-6.0 * wave.z / 1.2 * transition * (1.0 - transition)) * spacing_gradient;
            let modulation = lake_wave_modulation(position, wave.xy, wave.z, timing.y);
            let packet = modulation[0];
            let warp = modulation[1];
            let amplitude = wave.w * visibility * packet.x;
            let amplitude_gradient = wave.w * (visibility * packet.yz + packet.x * visibility_gradient);
            let phase = wave.z * dot(wave.xy, position) + warp.x - timing.x * camera_time.w + timing.y;
            let phase_gradient = wave.z * wave.xy + warp.yz;
            let phase_sine = sin(phase);
            let phase_cosine = cos(phase);
            let crest_basis = 0.5 + 0.5 * phase_sine;
            surface.offset.x += geometry_parameters.y * amplitude * wave.x * phase_cosine;
            surface.offset.z += geometry_parameters.y * amplitude * wave.y * phase_cosine;
            surface.offset.y += amplitude * (2.0 * crest_basis * crest_basis * crest_basis - 0.625);
            let horizontal_gradient = geometry_parameters.y * (phase_cosine * amplitude_gradient - amplitude * phase_sine * phase_gradient);
            horizontal_jacobian += vec4<f32>(wave.x * horizontal_gradient, wave.y * horizontal_gradient);
            height_gradient += amplitude * 3.0 * crest_basis * crest_basis * phase_cosine * phase_gradient
                + (2.0 * crest_basis * crest_basis * crest_basis - 0.625) * amplitude_gradient;
            crest += phase_sine * wave.z * amplitude;
            slope_energy += wave.z * wave.w;
        }
    }
    let determinant = horizontal_jacobian.x * horizontal_jacobian.w - horizontal_jacobian.y * horizontal_jacobian.z;
    surface.represented_slope = vec2<f32>(
        horizontal_jacobian.w * resolved_detail_slope.x - horizontal_jacobian.z * resolved_detail_slope.y,
        -horizontal_jacobian.y * resolved_detail_slope.x + horizontal_jacobian.x * resolved_detail_slope.y
    ) / max(determinant, 0.01);
    surface.tangent_x = vec3<f32>(horizontal_jacobian.x, height_gradient.x, horizontal_jacobian.z);
    surface.tangent_z = vec3<f32>(horizontal_jacobian.y, height_gradient.y, horizontal_jacobian.w);
    surface.crest = crest / max(slope_energy, 0.001);
    surface.compression = clamp(1.0 - determinant, 0.0, 1.0);
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
    var spacing = 1200.0;
#ifdef VERTEX_UVS_B
    spacing = vertex.uv_b.x;
#endif
    if waves_sky.w < 0.5 {
        let surface = lake_geometry(base_position.xz, spacing);
        output.world_position = vec4<f32>(base_position.xyz + surface.offset, 1.0);
        output.world_normal = normalize(cross(surface.tangent_z, surface.tangent_x));
#ifdef VERTEX_COLORS
        output.color = vec4<f32>(surface.represented_slope, surface.crest, surface.compression);
#endif
    }
    output.position = position_world_to_clip(output.world_position.xyz);
#ifdef VERTEX_UVS_A
    output.uv = base_position.xz;
#endif
#ifdef VERTEX_UVS_B
    output.uv_b = vec2<f32>(spacing, 0.0);
#endif
#ifdef VERTEX_TANGENTS
    output.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local, vertex.tangent, vertex.instance_index
    );
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

fn lake_wave_detail(position: vec2<f32>, spacing: f32, footprint_x: vec2<f32>, footprint_y: vec2<f32>) -> vec4<f32> {
    let drift = waves_sky.xy * camera_time.w;
    let near_sample = textureSampleGrad(detail_near, detail_near_sampler,
        (position - drift * 0.42) / 64.0, footprint_x / 64.0, footprint_y / 64.0);
    let far_sample = textureSampleGrad(detail_far, detail_far_sampler,
        (position - drift * 0.89) / 288.0, footprint_x / 288.0, footprint_y / 288.0);
    let near_slope = (near_sample.xy * 255.0 - 128.0) / 127.0;
    let far_slope = (far_sample.xy * 255.0 - 128.0) / 127.0;
    var slope = (near_slope * 0.6 + far_slope * 0.55) * waves_sky.z;
    for (var component: u32 = 0u; component < 24u; component += 1u) {
        if component < u32(geometry_parameters.x) {
            let wave = geometry_waves[component];
            let timing = geometry_motion[component];
            let modulation = lake_wave_modulation(position, wave.xy, wave.z, timing.y);
            let packet = modulation[0];
            let warp = modulation[1];
            let phase = wave.z * dot(wave.xy, position) + warp.x - timing.x * camera_time.w + timing.y;
            let phase_gradient = wave.z * wave.xy + warp.yz;
            let geometric_visibility = 1.0 - smoothstep(1.3, 2.5, wave.z * spacing);
            let phase_footprint = abs(dot(phase_gradient, footprint_x)) + abs(dot(phase_gradient, footprint_y));
            let pixel_visibility = 1.0 - smoothstep(2.0, 5.0, phase_footprint);
            let normal_weight = (1.0 - geometric_visibility) * pixel_visibility * 0.65;
            let crest_basis = 0.5 + 0.5 * sin(phase);
            slope += wave.w * (packet.x * 3.0 * crest_basis * crest_basis * cos(phase) * phase_gradient
                + (2.0 * crest_basis * crest_basis * crest_basis - 0.625) * packet.yz) * normal_weight;
        }
    }
    let near_variance = max(near_sample.z / 2.5 - dot(near_slope, near_slope), 0.0);
    let far_variance = max(far_sample.z / 2.5 - dot(far_slope, far_slope), 0.0);
    let unresolved_variance = (0.36 * near_variance + 0.3025 * far_variance) * waves_sky.z * waves_sky.z;
    return vec4<f32>(slope, unresolved_variance, 0.0);
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
    var spacing = 1200.0;
#ifdef VERTEX_UVS_B
    spacing = mesh.uv_b.x;
#endif
    var detail = lake_wave_detail(position, spacing, footprint_x, footprint_y);
#ifdef VERTEX_COLORS
    detail.x -= mesh.color.x;
    detail.y -= mesh.color.y;
    detail.w = smoothstep(0.68, 0.84, mesh.color.w) * smoothstep(0.7, 0.98, mesh.color.z) * 0.12;
#endif
    let geometric_normal = normalize(mesh.world_normal);
    let geometric_slope = -geometric_normal.xz / max(geometric_normal.y, 0.01);
    let total_slope = geometric_slope + detail.xy;
    let normal = normalize(vec3<f32>(-total_slope.x, 1.0, -total_slope.y));
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
    let whitecap_color = vec3<f32>(0.66, 0.72, 0.68) * (0.62 + 0.38 * sun_visibility);
    return vec4<f32>(mix(color, whitecap_color, detail.w), 1.0);
}
