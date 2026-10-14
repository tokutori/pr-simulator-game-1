#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput},
    mesh_functions,
    view_transformations::position_world_to_clip,
    mesh_view_bindings as view_bindings,
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    shadows::fetch_directional_shadow,
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
@group(#{MATERIAL_BIND_GROUP}) @binding(11) var<uniform> water_palette: array<vec4<f32>, 4>;
@group(#{MATERIAL_BIND_GROUP}) @binding(12) var<uniform> haze_parameters: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(13) var sky_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(14) var sky_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(15) var reflection_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(16) var reflection_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(17) var<uniform> reflection_clip_from_world: mat4x4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(18) var<uniform> reflection_parameters: vec4<f32>;

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
            slope_energy += wave.z * wave.w;
            if wave.z * spacing >= 2.5 { continue; }
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

fn lake_sky_radiance(direction: vec3<f32>) -> vec3<f32> {
    let ray = normalize(direction);
    let coordinate = vec2<f32>(
        atan2(ray.z, ray.x) / 6.2831853 + 0.5,
        asin(clamp(ray.y, -1.0, 1.0)) / 3.1415927 + 0.5
    );
    return textureSample(sky_texture, sky_sampler, coordinate).xyz;
}

fn lake_micro_detail(position: vec2<f32>, footprint_x: vec2<f32>, footprint_y: vec2<f32>) -> vec3<f32> {
    let fine_amplitude = 0.4;
    let ripple_amplitude = 0.1;
    let drift = waves_sky.xy * camera_time.w;
    let near_sample = textureSampleGrad(detail_near, detail_near_sampler,
        (position - drift * 0.42) / 64.0, footprint_x / 64.0, footprint_y / 64.0);
    let far_sample = textureSampleGrad(detail_far, detail_far_sampler,
        (position - drift * 0.89) / 288.0, footprint_x / 288.0, footprint_y / 288.0);
    let fine_rotation = mat2x2<f32>(vec2<f32>(0.6, 0.8), vec2<f32>(-0.8, 0.6));
    let ripple_rotation = mat2x2<f32>(vec2<f32>(0.8, 0.6), vec2<f32>(-0.6, 0.8));
    let fine_drift = transpose(fine_rotation) * drift * 0.26;
    let ripple_drift = transpose(ripple_rotation) * drift * 0.18;
    let fine_point = fine_rotation * ((position - fine_drift) * 2.6);
    let ripple_point = ripple_rotation * ((position - ripple_drift) * 5.2);
    let fine_sample = textureSampleGrad(detail_near, detail_near_sampler,
        fine_point / 64.0 + vec2<f32>(0.217, 0.631),
        fine_rotation * footprint_x * (2.6 / 64.0), fine_rotation * footprint_y * (2.6 / 64.0));
    let ripple_sample = textureSampleGrad(detail_near, detail_near_sampler,
        ripple_point / 64.0 + vec2<f32>(0.683, 0.173),
        ripple_rotation * footprint_x * (5.2 / 64.0), ripple_rotation * footprint_y * (5.2 / 64.0));
    let near_slope = (near_sample.xy * 255.0 - 128.0) / 127.0;
    let far_slope = (far_sample.xy * 255.0 - 128.0) / 127.0;
    let fine_raw_slope = (fine_sample.xy * 255.0 - 128.0) / 127.0;
    let ripple_raw_slope = (ripple_sample.xy * 255.0 - 128.0) / 127.0;
    let fine_slope = transpose(fine_rotation) * fine_raw_slope * (2.6 * fine_amplitude);
    let ripple_slope = transpose(ripple_rotation) * ripple_raw_slope * (5.2 * ripple_amplitude);
    let micro_slope = near_slope * 0.6 + far_slope * 0.55 + fine_slope + ripple_slope;
    let near_variance = max(near_sample.z / 2.5 - dot(near_slope, near_slope), 0.0);
    let far_variance = max(far_sample.z / 2.5 - dot(far_slope, far_slope), 0.0);
    let fine_variance = max(fine_sample.z / 2.5 - dot(fine_raw_slope, fine_raw_slope), 0.0);
    let ripple_variance = max(ripple_sample.z / 2.5 - dot(ripple_raw_slope, ripple_raw_slope), 0.0);
    let slope_moment = dot(micro_slope, micro_slope)
        + 0.6 * 0.6 * near_variance + 0.55 * 0.55 * far_variance
        + 2.6 * 2.6 * fine_amplitude * fine_amplitude * fine_variance
        + 5.2 * 5.2 * ripple_amplitude * ripple_amplitude * ripple_variance;
    let strength = smoothstep(0.05, 0.6, length(waves_sky.xy));
    let scaled_strength = strength * waves_sky.z;
    return vec3<f32>(micro_slope * scaled_strength, slope_moment * scaled_strength * scaled_strength);
}

fn lake_wave_slope(position: vec2<f32>, spacing: f32) -> vec2<f32> {
    var slope = vec2<f32>(0.0);
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
            let pixel_visibility = 1.0 - smoothstep(2.0, 5.0, fwidth(phase));
            let normal_weight = (1.0 - geometric_visibility) * pixel_visibility * 0.65;
            let crest_basis = 0.5 + 0.5 * sin(phase);
            slope += wave.w * (packet.x * 3.0 * crest_basis * crest_basis * cos(phase) * phase_gradient
                + (2.0 * crest_basis * crest_basis * crest_basis - 0.625) * packet.yz) * normal_weight;
        }
    }
    return slope;
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
    if waves_sky.w > 0.5 { return vec4<f32>(lake_sky_radiance(-view), 1.0); }
    var spacing = 1200.0;
#ifdef VERTEX_UVS_B
    spacing = mesh.uv_b.x;
#endif
    let quarter_x = footprint_x * 0.25;
    let quarter_y = footprint_y * 0.25;
    let micro_detail = (
        lake_micro_detail(position - quarter_x - quarter_y, footprint_x, footprint_y)
        + lake_micro_detail(position + quarter_x - quarter_y, footprint_x, footprint_y)
        + lake_micro_detail(position - quarter_x + quarter_y, footprint_x, footprint_y)
        + lake_micro_detail(position + quarter_x + quarter_y, footprint_x, footprint_y)
    ) * 0.25;
    let wave_slope = lake_wave_slope(position, spacing);
    let patches = 0.5 + 0.5 * lake_value_noise(mesh.world_position.xz * 0.17 + vec2<f32>(2.3, -7.1)).x;
    let unresolved_variance = max(micro_detail.z - dot(micro_detail.xy, micro_detail.xy), 0.0);
    var detail = vec4<f32>(wave_slope + micro_detail.xy, unresolved_variance, 0.0);
    var crest_light = 0.0;
#ifdef VERTEX_COLORS
    detail.x -= mesh.color.x;
    detail.y -= mesh.color.y;
    detail.w = smoothstep(0.68, 0.84, mesh.color.w) * smoothstep(0.7, 0.98, mesh.color.z) * 0.12;
    let sharp_crest = smoothstep(0.42, 0.76, mesh.color.w);
    crest_light = smoothstep(0.05, 1.05, mesh.color.z) * sharp_crest * mix(0.35, 1.0, patches);
#endif
    let geometric_normal = normalize(mesh.world_normal);
    let geometric_slope = -geometric_normal.xz / max(geometric_normal.y, 0.01);
    let total_slope = geometric_slope + detail.xy;
    let normal = normalize(vec3<f32>(-total_slope.x, 1.0, -total_slope.y));
    let geometric_ndv = max(dot(view, geometric_normal), 0.0);
    let facet_ndv = dot(view, normal);
    let facet_visibility = smoothstep(0.0, 0.18, facet_ndv);
    let reflected_direction = normalize(mix(reflect(-view, geometric_normal), reflect(-view, normal), facet_visibility));
    var sky_ray = vec3<f32>(reflected_direction.x, max(reflected_direction.y, 0.0), reflected_direction.z);
    if dot(sky_ray, sky_ray) < 1.0e-8 {
        sky_ray = vec3<f32>(1.0, 0.0, 0.0);
    }
    let reflection = lake_sky_radiance(sky_ray);
    let geometric_fresnel = 0.02 + 0.98 * pow(1.0 - geometric_ndv, 5.0);
    let facet_fresnel = 0.02 + 0.98 * pow(1.0 - max(facet_ndv, 0.0), 5.0);
    let fresnel = mix(geometric_fresnel, facet_fresnel, 0.4 * facet_visibility * smoothstep(0.03, 0.22, geometric_ndv));
    let roughness = clamp(0.23 + length(waves_sky.xy) * 0.018 + sqrt(detail.z) * 0.6, 0.23, 0.62);
    let sun_direction = select(vec3<f32>(0.0, 1.0, 0.0), sun_cloud.xyz, dot(sun_cloud.xyz, sun_cloud.xyz) > 0.5);
    var sun_visibility = 1.0;
    if haze_parameters.z > 0.5 && view_bindings::lights.n_directional_lights > 0u {
        let sun = view_bindings::lights.directional_lights[0];
        if (sun.flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            let view_depth = (view_bindings::view.view_from_world * mesh.world_position).z;
            sun_visibility = fetch_directional_shadow(0u, mesh.world_position,
                geometric_normal, view_depth, mesh.position.xy);
        }
    }
    let sun_alignment = max(dot(reflected_direction, sun_direction), 0.0);
    let glitter_lobe = pow(sun_alignment, mix(220.0, 45.0, roughness));
    let glitter_noise = 0.35 + 0.65 * clamp(sqrt(micro_detail.z) * 3.0, 0.0, 1.0);
    let glitter = glitter_lobe * glitter_noise * smoothstep(0.35, 2.0, length(waves_sky.xy)) * sun_visibility * haze_parameters.z;
    let diffuse = 0.58 + 0.42 * sun_visibility * max(dot(normal, sun_direction), 0.0) * haze_parameters.z;
    var base = mix(water_palette[0].xyz, water_palette[1].xyz, diffuse * 0.72);
    base = mix(base, water_palette[2].xyz, crest_light * 0.16 + patches * 0.07);
    base *= mix(0.82, 1.0, sun_visibility);
    base *= vec3<f32>(0.29, 0.34, 0.37);
    var color = mix(base, reflection, fresnel * 0.45);
    let reflected_clip = reflection_clip_from_world * mesh.world_position;
    if reflection_parameters.x > 0.5 && reflected_clip.w > 0.0 {
        let reflected_uv = reflected_clip.xy / reflected_clip.w * vec2<f32>(0.5, -0.5)
            + vec2<f32>(0.5) + total_slope * vec2<f32>(-0.006, -0.006);
        if all(reflected_uv >= vec2<f32>(0.0)) && all(reflected_uv <= vec2<f32>(1.0)) {
            let aircraft_reflection = textureSampleLevel(reflection_texture, reflection_sampler, reflected_uv, 0.0);
            color = mix(color, aircraft_reflection.rgb,
                aircraft_reflection.a * clamp(0.16 + fresnel * 0.7, 0.0, 0.72));
        }
    }
    let depth_tint = clamp(geometry_parameters.z / 0.35, 0.0, 1.0);
    color = mix(color, color * vec3<f32>(0.92, 0.97, 1.04), depth_tint * 0.12);
    color += vec3<f32>(1.0, 0.78, 0.52) * glitter * 0.15;
    color = mix(color, vec3<f32>(0.38, 0.43, 0.43), detail.w * 0.28);
    let distance_to_eye = length(camera_time.xyz - mesh.world_position.xyz);
    let atmospheric_fade = smoothstep(haze_parameters.x, haze_parameters.y, distance_to_eye);
    color = mix(color, water_palette[3].xyz, atmospheric_fade);
    return vec4<f32>(color, 1.0);
}
