use super::water_quality::{WaterQuality, WaterQualityFailure, WaterQualitySelection};
use bevy::{asset as bevy_asset, ecs as bevy_ecs, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    mesh::Indices,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, PrimitiveTopology},
    shader::ShaderRef,
};
use birdman_game_session::bundled_environment;

const GEOMETRY_WAVE_COUNT: usize = 4;
const NEAR_WIDTH_M: f32 = 64.0;
const NEAR_CELLS: u32 = 256;
const FADE_START_M: f32 = 24.0;
const FAR_HALF_WIDTH_M: f32 = 90_000.0;
const MAX_GEOMETRY_HEIGHT_M: f64 = 0.35;
const MAX_HORIZONTAL_DERIVATIVE: f64 = 0.45;
const CHOPPINESS: f64 = 0.75;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct WaterMaterial {
    #[uniform(0)]
    pub(crate) camera_time: Vec4,
    #[uniform(1)]
    pub(crate) sun_cloud: Vec4,
    #[uniform(2)]
    pub(crate) waves_sky: Vec4,
    #[uniform(3)]
    pub(crate) geometry_patch: Vec4,
    #[uniform(4)]
    geometry_waves: [Vec4; GEOMETRY_WAVE_COUNT],
    #[uniform(5)]
    geometry_motion: [Vec4; GEOMETRY_WAVE_COUNT],
}

impl Material for WaterMaterial {
    fn vertex_shader() -> ShaderRef {
        "native_water.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "native_water.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

impl WaterMaterial {
    pub(crate) fn registered(sky: bool) -> Result<Self, String> {
        let environment =
            bundled_environment().map_err(|error| format!("環境assetを読めない: {error:?}"))?;
        let document = environment.document();
        let direction = super::projection::sky_sun_direction(
            document.sky.sun_azimuth_degrees,
            document.sky.sun_elevation_degrees,
        );
        let wind = Vec2::new(
            document.waves.wind_velocity_ne_mps[1] as f32,
            -document.waves.wind_velocity_ne_mps[0] as f32,
        );
        let (geometry_waves, geometry_motion) = geometry_components(
            wind,
            document.waves.fetch_m,
            document.waves.detail_amplitude_scale,
            document.waves.pattern_seed,
        )?;
        Ok(Self {
            camera_time: Vec4::ZERO,
            sun_cloud: direction.extend(document.sky.cloud_fraction as f32),
            waves_sky: Vec4::new(
                document.waves.wind_velocity_ne_mps[1] as f32,
                -document.waves.wind_velocity_ne_mps[0] as f32,
                document.waves.detail_amplitude_scale as f32,
                f32::from(sky),
            ),
            geometry_patch: if sky {
                Vec4::ZERO
            } else {
                Vec4::new(0.0, 0.0, FADE_START_M, NEAR_WIDTH_M * 0.5)
            },
            geometry_waves,
            geometry_motion,
        })
    }

    pub(crate) fn far_surface(&self) -> Self {
        let mut material = self.clone();
        material.geometry_patch = Vec4::ZERO;
        material
    }

    pub(crate) fn registered_for_quality(quality: WaterQuality) -> Result<Self, String> {
        let mut material = Self::registered(false)?;
        if quality != WaterQuality::High {
            for wave in &mut material.geometry_waves {
                let high = geometry_resolution(wave.z, NEAR_CELLS);
                let selected = geometry_resolution(wave.z, quality.profile().near_cells);
                let ratio = if high > 0.0 {
                    (selected / high).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                wave.w *= ratio as f32;
            }
        }
        Ok(material)
    }

    pub(crate) fn near_bounds(&self) -> Aabb {
        let vertical = self.geometry_waves.iter().map(|wave| wave.w).sum::<f32>();
        let horizontal = self
            .geometry_waves
            .iter()
            .zip(self.geometry_motion)
            .map(|(wave, motion)| wave.w * motion.z)
            .sum::<f32>();
        let extent = Vec3::new(
            NEAR_WIDTH_M * 0.5 + horizontal,
            vertical + 0.001,
            NEAR_WIDTH_M * 0.5 + horizontal,
        );
        Aabb::from_min_max(-extent, extent)
    }

    pub(crate) fn project_camera(&mut self, camera: Vec3, time: f32) {
        self.camera_time = camera.extend(time);
        if self.geometry_patch.w > 0.0 {
            let center = patch_center(camera);
            self.geometry_patch.x = center.x;
            self.geometry_patch.y = center.y;
        }
    }
}

pub(crate) fn patch_center(camera: Vec3) -> Vec2 {
    let step = NEAR_WIDTH_M / NEAR_CELLS as f32;
    Vec2::new(
        (camera.x / step).floor() * step,
        (camera.z / step).floor() * step,
    )
}

pub(crate) fn near_mesh() -> Mesh {
    near_mesh_for_quality(WaterQuality::High)
}

pub(crate) fn near_mesh_for_quality(quality: WaterQuality) -> Mesh {
    Plane3d::default()
        .mesh()
        .size(NEAR_WIDTH_M, NEAR_WIDTH_M)
        .subdivisions(quality.profile().near_cells - 1)
        .build()
}

#[derive(Component)]
pub(crate) struct NearWaterSurface;

pub(crate) fn apply_quality(
    mut selection: ResMut<WaterQualitySelection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut near: Query<(&mut Mesh3d, &MeshMaterial3d<WaterMaterial>), With<NearWaterSurface>>,
) {
    let Some(requested) = selection.pending() else {
        return;
    };
    let result = (|| {
        let (mut current, material_handle) = near
            .single_mut()
            .map_err(|_| WaterQualityFailure::NearSurfaceUnavailable)?;
        let previous = current.0.id();
        if meshes.get(previous).is_none() {
            return Err(WaterQualityFailure::MeshUnavailable);
        }
        let mut current_material = materials
            .get_mut(material_handle.0.id())
            .ok_or(WaterQualityFailure::MaterialUnavailable)?;
        let prepared_mesh = near_mesh_for_quality(requested);
        let mut prepared_material = WaterMaterial::registered_for_quality(requested)
            .map_err(WaterQualityFailure::WavePreparation)?;
        prepared_material.camera_time = current_material.camera_time;
        prepared_material.geometry_patch = current_material.geometry_patch;
        current.0 = meshes.add(prepared_mesh);
        *current_material = prepared_material;
        meshes.remove(previous);
        Ok(())
    })();
    *selection = selection.completed(result);
}

fn geometry_resolution(wave_number: f32, cells: u32) -> f64 {
    let maximum_edge = f64::from(NEAR_WIDTH_M / cells as f32) * 2.0_f64.sqrt();
    let wavelength = std::f64::consts::TAU / f64::from(wave_number);
    let resolution = ((wavelength - maximum_edge * 2.0) / (maximum_edge * 2.0)).clamp(0.0, 1.0);
    resolution * resolution * (3.0 - 2.0 * resolution)
}

pub(crate) fn far_mesh() -> Mesh {
    let inner = NEAR_WIDTH_M * 0.5;
    let outer = FAR_HALF_WIDTH_M;
    let rectangles = [
        [-outer, -inner, -outer, outer],
        [inner, outer, -outer, outer],
        [-inner, inner, -outer, -inner],
        [-inner, inner, inner, outer],
    ];
    let mut positions = Vec::with_capacity(16);
    let mut indices = Vec::with_capacity(24);
    for [minimum_x, maximum_x, minimum_z, maximum_z] in rectangles {
        let first = positions.len() as u32;
        positions.extend([
            [minimum_x, 0.0, minimum_z],
            [minimum_x, 0.0, maximum_z],
            [maximum_x, 0.0, maximum_z],
            [maximum_x, 0.0, minimum_z],
        ]);
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 16])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; 16])
    .with_inserted_indices(Indices::U32(indices))
}

fn seeded_unit(seed: u32, index: u32) -> f64 {
    let mut value = seed.wrapping_add(index.wrapping_mul(0x9e37_79b9));
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb_352d);
    value = (value ^ (value >> 15)).wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    f64::from(value) / 4_294_967_296.0
}

fn geometry_components(
    wind: Vec2,
    fetch_m: f64,
    detail_scale: f64,
    seed: u32,
) -> Result<([Vec4; GEOMETRY_WAVE_COUNT], [Vec4; GEOMETRY_WAVE_COUNT]), String> {
    let wind_speed = f64::from(wind.x).hypot(f64::from(wind.y));
    if !wind.is_finite()
        || wind_speed > 60.0
        || !fetch_m.is_finite()
        || !(0.0..=50_000.0).contains(&fetch_m)
        || fetch_m == 0.0
        || !detail_scale.is_finite()
        || !(0.0..=3.0).contains(&detail_scale)
        || detail_scale == 0.0
    {
        return Err("描画用波浪入力が有限値または対応範囲を満たさない".into());
    }
    let mut waves = [Vec4::ZERO; GEOMETRY_WAVE_COUNT];
    let mut motion = [Vec4::ZERO; GEOMETRY_WAVE_COUNT];
    if wind_speed < 0.05 {
        return Ok((waves, motion));
    }
    let gravity = 9.80665;
    let dimensionless_fetch = gravity * fetch_m / wind_speed.powi(2);
    let significant_height =
        0.283 * wind_speed.powi(2) / gravity * (0.0125 * dimensionless_fetch.powf(0.42)).tanh();
    if significant_height < 0.001 {
        return Ok((waves, motion));
    }
    let peak_period =
        (7.54 * (0.077 * dimensionless_fetch.powf(0.25)).tanh() * wind_speed / gravity).max(0.8);
    let peak_frequency = std::f64::consts::TAU / peak_period;
    let mut weights = [0.0_f64; 18];
    let mut frequencies = [0.0_f64; 18];
    for index in 0..18 {
        let band = (index / 3) as f64 / 5.0;
        let frequency_jitter = (index % 3) as f64 * 0.035 - 0.035;
        let frequency = peak_frequency * ((band - 0.42) * 1.5 + frequency_jitter).exp();
        let ratio = peak_frequency / frequency;
        let sigma: f64 = if frequency <= peak_frequency {
            0.07
        } else {
            0.09
        };
        let enhancement = (-(frequency - peak_frequency).powi(2)
            / (2.0 * sigma.powi(2) * peak_frequency.powi(2)))
        .exp();
        let shape = frequency.powi(-5) * (-1.25 * ratio.powi(4)).exp() * 3.3_f64.powf(enhancement);
        let bandwidth = frequency * (0.3_f64.exp() - 1.0);
        weights[index] = (shape * bandwidth / 3.0).sqrt();
        frequencies[index] = frequency;
    }
    let normalization = significant_height
        / (4.0
            * weights
                .iter()
                .map(|weight| weight.powi(2))
                .sum::<f64>()
                .sqrt());
    let direction = wind / wind_speed as f32;
    let maximum_edge = f64::from(NEAR_WIDTH_M / NEAR_CELLS as f32) * 2.0_f64.sqrt();
    for (slot, index) in [1_usize, 4, 7, 8].into_iter().enumerate() {
        let band = (index / 3) as f64 / 5.0;
        let angle = ((index % 3) as f64 - 1.0) * (0.16 + 0.12 * band)
            + (seeded_unit(seed, index as u32 + 701) - 0.5) * 0.09;
        let wave_direction = Vec2::new(
            direction.x * angle.cos() as f32 - direction.y * angle.sin() as f32,
            direction.x * angle.sin() as f32 + direction.y * angle.cos() as f32,
        )
        .normalize();
        let frequency = frequencies[index];
        let wave_number = frequency.powi(2) / gravity;
        let wavelength = std::f64::consts::TAU / wave_number;
        let resolution = ((wavelength - maximum_edge * 2.0) / (maximum_edge * 2.0)).clamp(0.0, 1.0);
        let resolved_weight = resolution * resolution * (3.0 - 2.0 * resolution);
        let short_weight = ((wave_number - 1.5) / 4.5).clamp(0.0, 1.0);
        let visual_scale = 0.65 + 1.35 * short_weight.powi(2) * (3.0 - 2.0 * short_weight);
        let amplitude =
            weights[index] * normalization * detail_scale * resolved_weight * visual_scale;
        waves[slot] = wave_direction
            .extend(wave_number as f32)
            .extend(amplitude as f32);
        motion[slot] = Vec4::new(
            frequency as f32,
            (seeded_unit(seed, index as u32 + 101) * std::f64::consts::TAU) as f32,
            CHOPPINESS as f32,
            0.0,
        );
    }
    let height = waves.iter().map(|wave| f64::from(wave.w)).sum::<f64>();
    let fade_gradient = 1.5 * 2.0_f64.sqrt() / f64::from(NEAR_WIDTH_M * 0.5 - FADE_START_M);
    let derivative = waves
        .iter()
        .map(|wave| CHOPPINESS * f64::from(wave.w) * (f64::from(wave.z) + fade_gradient))
        .sum::<f64>();
    let scale = (MAX_GEOMETRY_HEIGHT_M / height.max(f64::MIN_POSITIVE))
        .min(MAX_HORIZONTAL_DERIVATIVE / derivative.max(f64::MIN_POSITIVE))
        .min(1.0) as f32;
    for wave in &mut waves {
        wave.w *= scale;
    }
    Ok((waves, motion))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn quality_profiles_preserve_registered_waves_and_bound_every_real_mesh_triangle() {
        let high = WaterMaterial::registered(false).unwrap();
        let high_bounds = high.near_bounds();
        assert_eq!(
            WaterMaterial::registered_for_quality(WaterQuality::High)
                .unwrap()
                .geometry_waves,
            high.geometry_waves
        );
        for quality in WaterQuality::ALL {
            let material = WaterMaterial::registered_for_quality(quality).unwrap();
            assert_eq!(material.camera_time, high.camera_time);
            assert_eq!(material.sun_cloud, high.sun_cloud);
            assert_eq!(material.waves_sky, high.waves_sky);
            assert_eq!(material.geometry_patch, high.geometry_patch);
            assert_eq!(material.geometry_motion, high.geometry_motion);
            for (wave, original) in material.geometry_waves.iter().zip(high.geometry_waves) {
                assert_eq!(wave.truncate(), original.truncate());
                assert!(wave.w >= 0.0 && wave.w <= original.w);
                let cells = quality.profile().near_cells;
                let edge = NEAR_WIDTH_M / cells as f32 * 2.0_f32.sqrt();
                if std::f32::consts::TAU / wave.z <= edge * 2.0 {
                    assert_eq!(wave.w, 0.0);
                }
            }
            let mesh = near_mesh_for_quality(quality);
            let cells = quality.profile().near_cells as usize;
            assert_eq!(mesh.count_vertices(), (cells + 1).pow(2));
            let positions: Vec<_> = mesh_positions(&mesh)
                .iter()
                .map(|position| {
                    let point = Vec3::from_array(*position);
                    let (offset, tangent_x, tangent_z) = reference_surface(&material, point.xz());
                    let normal = tangent_z.cross(tangent_x).normalize();
                    assert!(normal.is_finite() && normal.y > 0.0);
                    assert!(offset.y.abs() <= high_bounds.half_extents.y);
                    assert!(
                        offset.x.abs() <= high_bounds.half_extents.x - NEAR_WIDTH_M * 0.5 + 1.0e-5
                    );
                    assert!(
                        offset.z.abs() <= high_bounds.half_extents.z - NEAR_WIDTH_M * 0.5 + 1.0e-5
                    );
                    point + offset
                })
                .collect();
            let Some(Indices::U32(indices)) = mesh.indices() else {
                panic!("Expected uint32 quality mesh indices");
            };
            assert_eq!(indices.len(), cells * cells * 6);
            for triangle in indices.chunks_exact(3) {
                let first = positions[triangle[0] as usize];
                let second = positions[triangle[1] as usize];
                let third = positions[triangle[2] as usize];
                assert!((second - first).cross(third - first).y > 0.0);
            }
            assert_eq!(
                patch_center(Vec3::new(-0.01, 0.0, 0.24)),
                Vec2::new(-0.25, 0.0)
            );
        }
    }

    #[test]
    fn quality_exchange_replaces_only_owned_near_resources_and_releases_old_meshes() {
        let mut app = App::new();
        app.init_resource::<WaterQualitySelection>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Update, apply_quality);
        let mut material = WaterMaterial::registered(false).unwrap();
        material.project_camera(Vec3::new(1.01, 10.0, 2.04), 12.5);
        let bounds = material.near_bounds();
        let far_material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(material.far_surface());
        let near_material = app
            .world_mut()
            .resource_mut::<Assets<WaterMaterial>>()
            .add(material.clone());
        let far_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(far_mesh());
        let near_mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(near_mesh());
        let near = app
            .world_mut()
            .spawn((
                NearWaterSurface,
                Mesh3d(near_mesh),
                MeshMaterial3d(near_material.clone()),
                bounds,
            ))
            .id();
        for quality in [
            WaterQuality::Low,
            WaterQuality::Medium,
            WaterQuality::High,
            WaterQuality::Low,
        ] {
            let previous = app.world().get::<Mesh3d>(near).unwrap().0.id();
            let next = app
                .world()
                .resource::<WaterQualitySelection>()
                .request(quality, birdman_game_core::SessionPhase::FlightSetup)
                .unwrap();
            *app.world_mut().resource_mut::<WaterQualitySelection>() = next;
            app.update();
            let current = app.world().get::<Mesh3d>(near).unwrap().0.id();
            assert_ne!(current, previous);
            let meshes = app.world().resource::<Assets<Mesh>>();
            assert!(meshes.get(previous).is_none());
            assert!(meshes.get(far_mesh.id()).is_some());
            assert_eq!(meshes.len(), 2);
            let cells = quality.profile().near_cells as usize;
            assert_eq!(
                meshes.get(current).unwrap().count_vertices(),
                (cells + 1).pow(2)
            );
            let materials = app.world().resource::<Assets<WaterMaterial>>();
            assert_eq!(materials.len(), 2);
            let updated = materials.get(near_material.id()).unwrap();
            assert_eq!(updated.camera_time, material.camera_time);
            assert_eq!(updated.geometry_patch, material.geometry_patch);
            assert_eq!(updated.sun_cloud, material.sun_cloud);
            assert_eq!(updated.waves_sky, material.waves_sky);
            assert_eq!(
                materials.get(far_material.id()).unwrap().geometry_patch,
                Vec4::ZERO
            );
            assert_eq!(
                app.world().get::<Aabb>(near).unwrap().half_extents,
                material.near_bounds().half_extents
            );
            assert_eq!(
                app.world().resource::<WaterQualitySelection>().applied(),
                quality
            );
        }
    }

    #[test]
    fn quality_preparation_failure_preserves_applied_selection_and_mesh() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<WaterMaterial>>()
            .insert_resource(
                WaterQualitySelection::default()
                    .request(
                        WaterQuality::Low,
                        birdman_game_core::SessionPhase::FlightSetup,
                    )
                    .unwrap(),
            )
            .add_systems(Update, apply_quality);
        let previous = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(near_mesh());
        let missing_material = Handle::<WaterMaterial>::default();
        let entity = app
            .world_mut()
            .spawn((
                NearWaterSurface,
                Mesh3d(previous.clone()),
                MeshMaterial3d(missing_material),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, previous);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let quality = app.world().resource::<WaterQualitySelection>();
        assert_eq!(quality.applied(), WaterQuality::High);
        assert_eq!(quality.pending(), None);
        assert!(quality.status().contains("material"));
    }

    #[test]
    fn missing_near_surface_or_mesh_rejects_before_allocating_replacement_resources() {
        for has_surface in [false, true] {
            let mut app = App::new();
            app.init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<WaterMaterial>>()
                .insert_resource(
                    WaterQualitySelection::default()
                        .request(
                            WaterQuality::Low,
                            birdman_game_core::SessionPhase::FlightSetup,
                        )
                        .unwrap(),
                )
                .add_systems(Update, apply_quality);
            let material = app
                .world_mut()
                .resource_mut::<Assets<WaterMaterial>>()
                .add(WaterMaterial::registered(false).unwrap());
            if has_surface {
                app.world_mut().spawn((
                    NearWaterSurface,
                    Mesh3d(Handle::default()),
                    MeshMaterial3d(material),
                ));
            }
            app.update();
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
            assert_eq!(app.world().resource::<Assets<WaterMaterial>>().len(), 1);
            let quality = app.world().resource::<WaterQualitySelection>();
            assert_eq!(quality.applied(), WaterQuality::High);
            assert_eq!(quality.pending(), None);
            assert!(quality.status().contains(if has_surface {
                "mesh asset"
            } else {
                "描画対象"
            }));
        }
    }

    fn reference_surface(material: &WaterMaterial, position: Vec2) -> (Vec3, Vec3, Vec3) {
        let mut displacement = Vec3::ZERO;
        let mut tangent_x = Vec3::X;
        let mut tangent_z = Vec3::Z;
        if material.geometry_patch.w == 0.0 || material.waves_sky.w > 0.5 {
            return (displacement, tangent_x, tangent_z);
        }
        let fade_axis = |coordinate: f32| {
            let width = material.geometry_patch.w - material.geometry_patch.z;
            let progress = ((coordinate.abs() - material.geometry_patch.z) / width).clamp(0.0, 1.0);
            Vec2::new(
                1.0 - progress * progress * (3.0 - 2.0 * progress),
                -6.0 * progress * (1.0 - progress) * coordinate.signum() / width,
            )
        };
        let local = position - material.geometry_patch.truncate().truncate();
        let fade_x = fade_axis(local.x);
        let fade_z = fade_axis(local.y);
        let fade = fade_x.x * fade_z.x;
        let gradient = Vec2::new(fade_x.y * fade_z.x, fade_x.x * fade_z.y);
        for (wave, motion) in material.geometry_waves.iter().zip(material.geometry_motion) {
            let direction = wave.truncate().truncate();
            let phase =
                wave.z * direction.dot(position) - motion.x * material.camera_time.w + motion.y;
            let offset = Vec3::new(
                motion.z * wave.x * phase.cos(),
                phase.sin(),
                motion.z * wave.y * phase.cos(),
            ) * wave.w;
            let derivative = Vec3::new(
                -motion.z * wave.x * phase.sin(),
                phase.cos(),
                -motion.z * wave.y * phase.sin(),
            ) * wave.w;
            displacement += offset * fade;
            tangent_x += derivative * (wave.z * wave.x * fade) + offset * gradient.x;
            tangent_z += derivative * (wave.z * wave.y * fade) + offset * gradient.y;
        }
        (displacement, tangent_x, tangent_z)
    }

    fn mesh_positions(mesh: &Mesh) -> &[[f32; 3]] {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Expected float3 mesh positions");
        };
        positions
    }

    #[test]
    fn static_water_meshes_preserve_coverage_budget_and_an_open_far_center() {
        let near = near_mesh();
        assert_eq!(near.count_vertices(), 66_049);
        let Some(Indices::U32(near_indices)) = near.indices() else {
            panic!("Expected uint32 near mesh indices");
        };
        assert_eq!(near_indices.len(), 131_072 * 3);
        for [position_x, position_y, position_z] in mesh_positions(&near) {
            assert!(position_x.abs() <= 32.0 && position_z.abs() <= 32.0);
            assert_eq!(*position_y, 0.0);
        }
        let far = far_mesh();
        assert_eq!(far.count_vertices(), 16);
        let Some(Indices::U32(far_indices)) = far.indices() else {
            panic!("Expected uint32 far mesh indices");
        };
        assert_eq!(far_indices.len(), 24);
        let positions = mesh_positions(&far);
        for triangle in far_indices.chunks_exact(3) {
            let first = Vec3::from_array(positions[triangle[0] as usize]);
            let second = Vec3::from_array(positions[triangle[1] as usize]);
            let third = Vec3::from_array(positions[triangle[2] as usize]);
            assert!((second - first).cross(third - first).y > 0.0);
        }
        for rectangle in positions.chunks_exact(4) {
            let minimum_x = rectangle
                .iter()
                .map(|position| position[0])
                .fold(f32::INFINITY, f32::min);
            let maximum_x = rectangle
                .iter()
                .map(|position| position[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let minimum_z = rectangle
                .iter()
                .map(|position| position[2])
                .fold(f32::INFINITY, f32::min);
            let maximum_z = rectangle
                .iter()
                .map(|position| position[2])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                maximum_x <= -32.0 || minimum_x >= 32.0 || maximum_z <= -32.0 || minimum_z >= 32.0
            );
        }
        assert!(positions.iter().any(|position| position[0] == -90_000.0));
        assert!(positions.iter().any(|position| position[2] == 90_000.0));
    }

    #[test]
    fn geometry_coefficients_use_fetch_seed_and_registered_wind_direction() {
        let wind = Vec2::new(2.5, 0.0);
        let first = geometry_components(wind, 600.0, 1.0, 202607).unwrap();
        assert_eq!(
            first,
            geometry_components(wind, 600.0, 1.0, 202607).unwrap()
        );
        assert_ne!(
            first,
            geometry_components(wind, 1200.0, 1.0, 202607).unwrap()
        );
        assert_ne!(
            first,
            geometry_components(wind, 600.0, 1.0, 202608).unwrap()
        );
        assert!(first.0.iter().any(|wave| wave.w > 0.001));
        for wave in first.0 {
            assert!(wave.x > 0.9);
            assert!((wave.truncate().truncate().length() - 1.0).abs() < 1.0e-6);
        }
        let northward = geometry_components(Vec2::new(0.0, -2.5), 600.0, 1.0, 202607).unwrap();
        for wave in northward.0 {
            assert!(wave.y < -0.9);
        }
        assert_eq!(
            geometry_components(Vec2::ZERO, 600.0, 1.0, 202607)
                .unwrap()
                .0,
            [Vec4::ZERO; 4]
        );
    }

    #[test]
    fn geometry_rejects_invalid_render_inputs() {
        for wind in [Vec2::new(f32::NAN, 1.0), Vec2::new(61.0, 0.0)] {
            assert!(geometry_components(wind, 600.0, 1.0, 1).is_err());
        }
        for fetch in [0.0, -1.0, 50_001.0, f64::NAN, f64::INFINITY] {
            assert!(geometry_components(Vec2::X, fetch, 1.0, 1).is_err());
        }
        for detail in [0.0, -1.0, 3.01, f64::NAN, f64::INFINITY] {
            assert!(geometry_components(Vec2::X, 600.0, detail, 1).is_err());
        }
    }

    #[test]
    fn geometry_height_and_horizontal_derivative_are_bounded_for_supported_inputs() {
        let fade_gradient = 1.5 * 2.0_f32.sqrt() / (NEAR_WIDTH_M * 0.5 - FADE_START_M);
        for wind_speed in [0.0, 0.05, 1.2, 2.5, 12.0, 60.0] {
            for fetch in [1.0, 600.0, 50_000.0] {
                for detail in [0.1, 1.0, 3.0] {
                    let (waves, motion) =
                        geometry_components(Vec2::new(wind_speed, 0.0), fetch, detail, u32::MAX)
                            .unwrap();
                    let height = waves.iter().map(|wave| wave.w).sum::<f32>();
                    let derivative = waves
                        .iter()
                        .zip(motion)
                        .map(|(wave, motion)| wave.w * motion.z * (wave.z + fade_gradient))
                        .sum::<f32>();
                    assert!(height <= MAX_GEOMETRY_HEIGHT_M as f32 + 1.0e-6);
                    assert!(derivative <= MAX_HORIZONTAL_DERIVATIVE as f32 + 1.0e-6);
                    for (wave, motion) in waves.iter().zip(motion) {
                        assert!(wave.is_finite() && motion.is_finite());
                        assert!(wave.w >= 0.0);
                        assert!(motion.z >= 0.0);
                    }
                }
            }
        }
    }

    #[test]
    fn patch_tracking_retains_world_phase_and_displacement_bounds() {
        let mut material = WaterMaterial::registered(false).unwrap();
        let coefficients = material.geometry_waves;
        let motion = material.geometry_motion;
        material.project_camera(Vec3::new(-0.01, 10.0, 0.24), 3.0);
        assert_eq!(
            patch_center(material.camera_time.truncate()),
            Vec2::new(-0.25, 0.0)
        );
        let first = reference_surface(&material, Vec2::new(3.0, 4.0));
        material.project_camera(Vec3::new(1.01, 20.0, 2.04), 3.0);
        let second = reference_surface(&material, Vec2::new(3.0, 4.0));
        assert_eq!(first, second);
        assert_eq!(material.geometry_waves, coefficients);
        assert_eq!(material.geometry_motion, motion);
        assert_eq!(
            material.geometry_patch.truncate().truncate(),
            Vec2::new(1.0, 2.0)
        );
        let bounds = material.near_bounds();
        assert!(bounds.half_extents.y > 0.0);
        for position in [Vec2::ZERO, Vec2::new(27.0, 0.0), Vec2::new(30.0, 31.0)] {
            let (offset, _, _) = reference_surface(&material, position);
            assert!(offset.y.abs() < bounds.half_extents.y);
            assert!(offset.x.abs() <= bounds.half_extents.x - 32.0 + 1.0e-5);
            assert!(offset.z.abs() <= bounds.half_extents.z - 32.0 + 1.0e-5);
        }
    }

    #[test]
    fn boundary_fade_derivatives_match_finite_differences_and_remain_single_valued() {
        let mut material = WaterMaterial::registered(false).unwrap();
        material.project_camera(Vec3::new(0.0, 10.0, 0.0), 2.5);
        for position in [
            Vec2::new(1.0, 2.0),
            Vec2::new(26.0, 2.0),
            Vec2::new(-29.0, 28.0),
            Vec2::new(31.5, -30.0),
        ] {
            let (_, tangent_x, tangent_z) = reference_surface(&material, position);
            let delta = 0.005;
            let numeric_tangent = |direction: Vec2, base: Vec3| {
                let forward = reference_surface(&material, position + direction * delta).0;
                let backward = reference_surface(&material, position - direction * delta).0;
                base + (forward - backward) / (2.0 * delta)
            };
            assert!(tangent_x.distance(numeric_tangent(Vec2::X, Vec3::X)) < 0.002);
            assert!(tangent_z.distance(numeric_tangent(Vec2::Y, Vec3::Z)) < 0.002);
            let normal = tangent_z.cross(tangent_x).normalize();
            assert!(normal.is_finite() && normal.y > 0.0);
            let determinant = tangent_x.x * tangent_z.z - tangent_z.x * tangent_x.z;
            assert!(determinant >= (1.0 - MAX_HORIZONTAL_DERIVATIVE as f32).powi(2));
        }
        for position in [
            Vec2::new(32.0, 0.0),
            Vec2::new(-32.0, 10.0),
            Vec2::new(0.0, 32.0),
            Vec2::new(32.0, -32.0),
        ] {
            assert_eq!(
                reference_surface(&material, position),
                (Vec3::ZERO, Vec3::X, Vec3::Z)
            );
        }
        assert_eq!(
            reference_surface(&material.far_surface(), Vec2::ZERO),
            (Vec3::ZERO, Vec3::X, Vec3::Z)
        );
        assert_eq!(
            reference_surface(&WaterMaterial::registered(true).unwrap(), Vec2::ZERO),
            (Vec3::ZERO, Vec3::X, Vec3::Z)
        );
        assert!(!WaterMaterial::enable_prepass());
        assert!(!WaterMaterial::enable_shadows());
    }

    #[test]
    fn registered_water_and_sky_share_environment_without_transparency() {
        let water = WaterMaterial::registered(false).unwrap();
        let sky = WaterMaterial::registered(true).unwrap();
        let environment = bundled_environment().unwrap();
        let waves = &environment.document().waves;
        assert_eq!(water.camera_time, Vec4::ZERO);
        assert_eq!(sky.camera_time, water.camera_time);
        assert_eq!(sky.sun_cloud, water.sun_cloud);
        assert_eq!(sky.waves_sky.truncate(), water.waves_sky.truncate());
        assert_eq!(water.waves_sky.w, 0.0);
        assert_eq!(sky.waves_sky.w, 1.0);
        assert_eq!(water.waves_sky.x, waves.wind_velocity_ne_mps[1] as f32);
        assert_eq!(water.waves_sky.y, -waves.wind_velocity_ne_mps[0] as f32);
        assert_eq!(water.waves_sky.z, waves.detail_amplitude_scale as f32);
        assert!(water.sun_cloud.is_finite());
        assert!((water.sun_cloud.truncate().length() - 1.0).abs() < 1.0e-6);
        assert!(
            water.sun_cloud.truncate().distance(Vec3::new(
                0.405_579_78,
                0.819_152_06,
                0.405_579_78
            )) < 1.0e-6
        );
        assert!(matches!(water.alpha_mode(), AlphaMode::Opaque));
        assert!(matches!(sky.alpha_mode(), AlphaMode::Opaque));
    }
}
