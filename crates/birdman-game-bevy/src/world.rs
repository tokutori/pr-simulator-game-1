use super::{
    CameraMode,
    environment::{EnvironmentSun, EnvironmentSurface, NativeEnvironment},
    native_session::{NativeSession, TailDisplay},
    projection::{VerificationObserver, aircraft_transform, camera_transform, sunlight_transform},
    water::{NearWaterSurface, WaterMaterial, far_mesh, near_mesh, patch_center},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::{NoFrustumCulling, RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::{CompressedImageFormats, ImageSampler, ImageSamplerDescriptor, ImageType},
    light::{CascadeShadowConfigBuilder, NotShadowCaster},
    mesh::Indices,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use birdman_game_core::{
    HybridMockDefinition, HybridSection, HybridSurfaceGeometry, HybridSurfaceRole,
};
use birdman_game_session::cockpit_geometry::{CockpitGeometry, CockpitMaterial};
use birdman_game_session::wing_geometry::{WingFrame, WingGeometry, WingMaterial, WingSurface};
use birdman_game_session::wing_texture;
use birdman_game_session::{LaunchPlatform, launch_venue};
use serde::Deserialize;

#[derive(Component, PartialEq, Eq)]
pub(crate) enum WorldProjection {
    Aircraft,
    HorizontalTail,
    VerticalTail,
    FlightCamera,
    SkyDome,
    LakeSurface,
}

#[derive(Resource, Default)]
pub(crate) struct RenderHistory {
    samples: Option<RenderSamples>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RenderSample {
    aircraft: Transform,
    pilot_position_m: f64,
    elevator_rad: f64,
    rudder_rad: f64,
    simulation_time_seconds: f64,
}

impl RenderSample {
    fn from_display(display: &TailDisplay) -> Self {
        Self {
            aircraft: aircraft_transform(display.state),
            pilot_position_m: display.state.pilot_position_m(),
            elevator_rad: display.incidence.elevator_rad(),
            rudder_rad: display.incidence.rudder_rad(),
            simulation_time_seconds: display.tick / f64::from(birdman_game_core::PHYSICS_HZ),
        }
    }
}

struct RenderSamples {
    previous: RenderSample,
    current: RenderSample,
}

impl RenderHistory {
    pub(crate) fn capture_interval(&mut self, previous: &TailDisplay, current: &TailDisplay) {
        self.samples = Some(RenderSamples {
            previous: RenderSample::from_display(previous),
            current: RenderSample::from_display(current),
        });
    }
    pub(crate) fn reset(&mut self) {
        self.samples = None;
    }
    fn interpolated(&self, fraction: f64) -> Option<RenderSample> {
        let samples = self.samples.as_ref()?;
        let previous = samples.previous;
        let current = samples.current;
        let interpolate = |previous: f64, current: f64| previous + (current - previous) * fraction;
        Some(RenderSample {
            aircraft: Transform::from_translation(
                previous
                    .aircraft
                    .translation
                    .lerp(current.aircraft.translation, fraction as f32),
            )
            .with_rotation(
                previous
                    .aircraft
                    .rotation
                    .slerp(current.aircraft.rotation, fraction as f32),
            ),
            pilot_position_m: interpolate(previous.pilot_position_m, current.pilot_position_m),
            elevator_rad: interpolate(previous.elevator_rad, current.elevator_rad),
            rudder_rad: interpolate(previous.rudder_rad, current.rudder_rad),
            simulation_time_seconds: interpolate(
                previous.simulation_time_seconds,
                current.simulation_time_seconds,
            ),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerrainGrid {
    north_min_meters: f64,
    east_min_meters: f64,
    columns: usize,
    rows: usize,
    terrain_step_meters: f64,
    elevation_meters_above_water: Vec<Option<f64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinePatch {
    id: String,
    #[serde(flatten)]
    grid: TerrainGrid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Terrain {
    #[serde(flatten)]
    grid: TerrainGrid,
    fine_patches: Vec<FinePatch>,
}

#[derive(Deserialize)]
struct LandMasks {
    grids: Vec<LandMask>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LandMask {
    id: String,
    columns: usize,
    rows: usize,
    land_mask_hex: String,
}

fn land_vertex(mask: &LandMask, index: usize) -> bool {
    mask.land_mask_hex
        .as_bytes()
        .get(index / 4)
        .and_then(|byte| char::from(*byte).to_digit(16))
        .is_some_and(|bits| bits & (1 << (index % 4)) != 0)
}

fn within(grid: &TerrainGrid, north: f64, east: f64) -> bool {
    north >= grid.north_min_meters
        && north <= grid.north_min_meters + (grid.rows - 1) as f64 * grid.terrain_step_meters
        && east >= grid.east_min_meters
        && east <= grid.east_min_meters + (grid.columns - 1) as f64 * grid.terrain_step_meters
}

fn terrain_mesh(grid: &TerrainGrid, mask: &LandMask, finer: &[FinePatch]) -> Result<Mesh, String> {
    if grid.columns < 2
        || grid.rows < 2
        || grid.elevation_meters_above_water.len() != grid.columns * grid.rows
        || mask.columns != grid.columns
        || mask.rows != grid.rows
        || mask.land_mask_hex.len() * 4 < grid.columns * grid.rows
    {
        return Err("地形grid/mask寸法が一致しない".into());
    }
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for row in 0..grid.rows - 1 {
        for column in 0..grid.columns - 1 {
            let north =
                grid.north_min_meters + (grid.rows - 1 - row) as f64 * grid.terrain_step_meters;
            let east = grid.east_min_meters + column as f64 * grid.terrain_step_meters;
            if finer.iter().any(|patch| {
                within(
                    &patch.grid,
                    north - grid.terrain_step_meters / 2.0,
                    east + grid.terrain_step_meters / 2.0,
                )
            }) {
                continue;
            }
            let corners = [
                row * grid.columns + column,
                row * grid.columns + column + 1,
                (row + 1) * grid.columns + column,
                (row + 1) * grid.columns + column + 1,
            ];
            for triangle in [
                [corners[0], corners[2], corners[1]],
                [corners[1], corners[2], corners[3]],
            ] {
                if !triangle.iter().all(|index| {
                    land_vertex(mask, *index) && grid.elevation_meters_above_water[*index].is_some()
                }) {
                    continue;
                }
                for index in triangle {
                    let elevation =
                        grid.elevation_meters_above_water[index].ok_or("地形高度欠損")?;
                    if !elevation.is_finite() {
                        return Err("地形高度が非有限".into());
                    }
                    let point = [
                        grid.east_min_meters
                            + (index % grid.columns) as f64 * grid.terrain_step_meters,
                        elevation.max(0.02),
                        -(grid.north_min_meters
                            + (grid.rows - 1 - index / grid.columns) as f64
                                * grid.terrain_step_meters),
                    ];
                    positions.push(point.map(|coordinate| coordinate as f32));
                    colors.push(if elevation < 3.0 {
                        [0.48, 0.47, 0.31, 1.0]
                    } else {
                        [0.19, 0.32, 0.17, 1.0]
                    });
                }
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.compute_flat_normals();
    Ok(mesh)
}

fn rectangular_surface(vertices: [[f32; 3]; 4]) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices.to_vec());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );
    mesh.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));
    mesh.compute_normals();
    mesh
}

fn platform_transform(platform: &LaunchPlatform, thickness: f32) -> Transform {
    let [north, east] = platform.horizontal_direction_ned();
    let slope = platform.downward_slope_degrees.to_radians() as f32;
    let rotation =
        Quat::from_rotation_y(-platform.heading_rad() as f32) * Quat::from_rotation_x(-slope);
    let top_center = Vec3::new(
        -(east * platform.length_meters / 2.0) as f32,
        platform.front_lip_above_water_meters as f32
            + platform.length_meters as f32 * slope.tan() / 2.0,
        (north * platform.length_meters / 2.0) as f32,
    );
    Transform::from_translation(top_center - rotation * Vec3::Y * thickness / 2.0)
        .with_rotation(rotation)
}

struct TailSurfaceProjection {
    transform: Transform,
    vertices: [[f32; 3]; 4],
}

#[derive(Resource)]
pub(crate) struct MainWingProjection {
    geometry: WingGeometry,
    frame: WingFrame,
    surfaces: Vec<MainWingSurface>,
}

struct MainWingSurface {
    complete_mesh: Mesh,
    material_meshes: Vec<Handle<Mesh>>,
}

fn main_wing_mesh(surface: &WingSurface, positions: &[[f32; 3]]) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, surface.uvs().to_vec());
    mesh.insert_indices(Indices::U32(surface.indices().to_vec()));
    mesh.compute_area_weighted_normals();
    mesh
}

fn wing_cover_images() -> [Image; 2] {
    let mut pixels = wing_texture::indigo_wing_textures();
    let decal = Image::from_buffer(
        include_bytes!("../../../assets/indigo-resist-decal.png"),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    );
    let applied = decal.map_err(|cause| cause.to_string()).and_then(|image| {
        if image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb {
            return Err("Registered wing decal must decode to sRGB RGBA8".into());
        }
        let alpha: Vec<_> = image
            .data
            .as_deref()
            .unwrap_or_default()
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect();
        wing_texture::apply_indigo_wing_decal(&mut pixels, image.width(), image.height(), &alpha)
            .map_err(|cause| format!("Registered wing decal is invalid: {cause:?}"))
    });
    if let Err(cause) = applied {
        warn!("Wing dye decal could not be applied; using the procedural pattern: {cause}");
    }
    std::array::from_fn(|side| {
        let start = side * wing_texture::SIDE_BYTES;
        let rgba = &pixels[start..start + wing_texture::SIDE_BYTES];
        let mut image = Image::new(
            Extent3d {
                width: wing_texture::WIDTH,
                height: wing_texture::HEIGHT,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba.to_vec(),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.data = Some(
            wing_texture::indigo_wing_mipmaps(rgba)
                .expect("Shared opaque sRGB wing texture supports linear-light mip generation"),
        );
        image.texture_descriptor.mip_level_count = wing_texture::WIDTH.ilog2() + 1;
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
        image
    })
}

fn main_wing_airspeed(session: &NativeSession) -> f64 {
    let speed = match session.game.core_session().snapshot().phase() {
        birdman_game_core::SessionPhase::Replay | birdman_game_core::SessionPhase::Attract => {
            session
                .game
                .core_session()
                .playback_clock()
                .and_then(|clock| {
                    session
                        .game
                        .core_session()
                        .playback_sample_at_seconds(clock.time_seconds())
                        .ok()
                        .map(|sample| sample.telemetry.airspeed_mps)
                })
        }
        _ => session
            .game
            .core_session()
            .telemetry()
            .ok()
            .flatten()
            .map(|telemetry| telemetry.airspeed_mps),
    };
    speed.unwrap_or(f64::NAN)
}

pub(crate) fn project_main_wing(
    session: Res<NativeSession>,
    mut wing: ResMut<MainWingProjection>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let MainWingProjection {
        geometry,
        frame,
        surfaces,
    } = &mut *wing;
    if !geometry.update_frame(frame, main_wing_airspeed(&session)) {
        return;
    }
    for (surface, positions) in surfaces.iter_mut().zip(frame.surface_positions()) {
        let bevy::mesh::VertexAttributeValues::Float32x3(values) = surface
            .complete_mesh
            .attribute_mut(Mesh::ATTRIBUTE_POSITION)
            .expect("主翼positionsがない")
        else {
            panic!("主翼positionsはFloat32x3を要する");
        };
        values.copy_from_slice(positions);
        surface.complete_mesh.compute_area_weighted_normals();
        let normals = surface
            .complete_mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .expect("主翼normalがない");
        for handle in &surface.material_meshes {
            let mut mesh = meshes.get_mut(handle).expect("主翼meshがない");
            let bevy::mesh::VertexAttributeValues::Float32x3(values) = mesh
                .attribute_mut(Mesh::ATTRIBUTE_POSITION)
                .expect("主翼positionsがない")
            else {
                panic!("主翼positionsはFloat32x3を要する");
            };
            values.copy_from_slice(positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone());
        }
    }
}

fn project_tail_surface(
    geometry: HybridSurfaceGeometry<'_>,
) -> Result<TailSurfaceProjection, &'static str> {
    if geometry.role() == HybridSurfaceRole::MainWing {
        return Err("尾翼表示には尾翼geometryが必要");
    }
    let [first, last] = geometry.sections() else {
        return Err("登録尾翼表示には2つのsectionが必要");
    };
    let first_point = first.quarter_chord().components();
    let last_point = last.quarter_chord().components();
    let center = [
        (first_point[0] + last_point[0]) / 2.0,
        (first_point[1] + last_point[1]) / 2.0,
        (first_point[2] + last_point[2]) / 2.0,
    ];
    let vertex = |section: HybridSection, chord_fraction: f64| {
        let point = section.quarter_chord().components();
        [
            (point[1] - center[1]) as f32,
            (center[2] - point[2]) as f32,
            (center[0] - point[0] + chord_fraction * section.chord_m()) as f32,
        ]
    };
    Ok(TailSurfaceProjection {
        transform: Transform::from_xyz(center[1] as f32, -center[2] as f32, -center[0] as f32),
        vertices: [
            vertex(*first, -0.25),
            vertex(*last, -0.25),
            vertex(*last, 0.75),
            vertex(*first, 0.75),
        ],
    })
}

pub(crate) fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.init_resource::<RenderHistory>();
    commands.insert_resource(NativeEnvironment::try_default().expect("登録環境が不正"));
    let terrain: Terrain = serde_json::from_str(include_str!("../../../assets/biwa-terrain.json"))
        .expect("登録terrain JSONが不正");
    let masks: LandMasks =
        serde_json::from_str(include_str!("../../../assets/biwa-land-mask.json"))
            .expect("登録land mask JSONが不正");
    let land = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        ..default()
    });
    for (id, grid, finer) in std::iter::once((
        "broad-terrain",
        &terrain.grid,
        terrain.fine_patches.as_slice(),
    ))
    .chain(
        terrain
            .fine_patches
            .iter()
            .map(|patch| (patch.id.as_str(), &patch.grid, &[][..])),
    ) {
        let mask = masks
            .grids
            .iter()
            .find(|mask| mask.id == id)
            .expect("登録land maskが欠損");
        let mesh = terrain_mesh(grid, mask, finer).expect("登録terrain/maskが不正");
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(land.clone()),
            Transform::IDENTITY,
        ));
    }
    let mut registered_water = WaterMaterial::registered(false).expect("登録wavesが不正");
    registered_water
        .upload_detail(&mut images)
        .expect("登録微細波画像が不正");
    let sunlight = sunlight_transform(registered_water.sun_cloud.truncate());
    let mut registered_sky = registered_water.clone();
    registered_sky.waves_sky.w = 1.0;
    registered_sky.geometry_patch = Vec4::ZERO;
    let far_surface = water.add(registered_water.far_surface());
    let near_bounds = registered_water.near_bounds();
    let surface = water.add(registered_water);
    commands.spawn((
        WorldProjection::LakeSurface,
        NearWaterSurface,
        EnvironmentSurface::Near,
        Mesh3d(meshes.add(near_mesh())),
        MeshMaterial3d(surface),
        Transform::IDENTITY,
        near_bounds,
    ));
    commands.spawn((
        WorldProjection::LakeSurface,
        EnvironmentSurface::Far,
        Mesh3d(meshes.add(far_mesh())),
        MeshMaterial3d(far_surface),
        Transform::IDENTITY,
    ));
    let mut dome = Sphere::new(90_000.0).mesh().uv(48, 24);
    if let Some(Indices::U32(indices)) = dome.indices_mut() {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    commands.spawn((
        WorldProjection::SkyDome,
        EnvironmentSurface::Sky,
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(water.add(registered_sky)),
        Transform::IDENTITY,
    ));
    commands.spawn((
        EnvironmentSun,
        DirectionalLight {
            illuminance: 18_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 30.0,
            maximum_distance: 600.0,
            num_cascades: 3,
            ..default()
        }
        .build(),
        RenderLayers::layer(0).with(1),
        sunlight,
    ));
    let platform = launch_venue().expect("登録launch venueが不正").platform;
    let platform_material = materials.add(Color::srgb(0.55, 0.43, 0.25));
    let platform_thickness = 0.35;
    let deck_length =
        platform.length_meters as f32 / (platform.downward_slope_degrees.to_radians() as f32).cos();
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            platform.width_meters as f32,
            platform_thickness,
            deck_length,
        ))),
        MeshMaterial3d(platform_material),
        platform_transform(&platform, platform_thickness),
    ));
    let white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.87, 0.9, 0.93),
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let frame = materials.add(Color::srgb(0.13, 0.15, 0.18));
    let wing_cover = wing_cover_images().map(|image| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(images.add(image)),
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    });
    let wing_underside = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(242, 243, 237),
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let wing_film = materials.add(StandardMaterial {
        base_color: Color::srgba(213.0 / 255.0, 221.0 / 255.0, 217.0 / 255.0, 0.22),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let wing_geometry = WingGeometry::new();
    let wing_frame = wing_geometry.frame();
    let cockpit = CockpitGeometry::new();
    let cockpit_materials = CockpitMaterial::ALL.map(|kind| {
        let color = kind.color();
        let opacity = kind.opacity();
        materials.add(StandardMaterial {
            base_color: Color::srgba(
                ((color >> 16) & 255) as f32 / 255.0,
                ((color >> 8) & 255) as f32 / 255.0,
                (color & 255) as f32 / 255.0,
                opacity,
            ),
            alpha_mode: if opacity < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            double_sided: true,
            cull_mode: None,
            ..default()
        })
    });
    let mut wing_surfaces = Vec::new();
    let definition = HybridMockDefinition::try_new().expect("登録機体geometryが不正");
    let surfaces = definition.surfaces().expect("登録surface geometryが不正");
    let tail_projection = |role| {
        let geometry = surfaces
            .iter()
            .find(|surface| surface.geometry().role() == role)
            .expect("登録尾翼geometryがない")
            .geometry();
        project_tail_surface(geometry).expect("登録尾翼表示geometryが不正")
    };
    let horizontal_tail = tail_projection(HybridSurfaceRole::HorizontalTail);
    let vertical_tail = tail_projection(HybridSurfaceRole::VerticalTail);
    commands
        .spawn((
            WorldProjection::Aircraft,
            exhibition_aircraft_transform(platform),
            Visibility::Visible,
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 3.0))),
                MeshMaterial3d(frame),
                Transform::from_xyz(0.0, -0.2, 0.5),
                RenderLayers::layer(0).with(1),
            ));
            for rod in &cockpit.rods {
                let [from, to] = rod
                    .endpoints
                    .map(|point| Vec3::from_array(point.map(|value| value as f32)));
                let direction = to - from;
                parent.spawn((
                    Name::new(if rod.material == CockpitMaterial::Frame {
                        "cockpit-frame"
                    } else {
                        "canopy-rim-segment"
                    }),
                    Mesh3d(
                        meshes.add(
                            Cylinder::new(rod.radius as f32, 1.0)
                                .mesh()
                                .resolution(8)
                                .build(),
                        ),
                    ),
                    MeshMaterial3d(cockpit_materials[rod.material as usize].clone()),
                    Transform::from_translation((from + to) * 0.5)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction.normalize()))
                        .with_scale(Vec3::new(1.0, direction.length(), 1.0)),
                    RenderLayers::layer(0).with(1),
                ));
            }
            for surface in &cockpit.surfaces {
                let mut mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, surface.positions.clone())
                .with_inserted_indices(Indices::U32(surface.indices.clone()));
                mesh.compute_area_weighted_normals();
                let transparent = surface.material == CockpitMaterial::Canopy;
                let mut child = parent.spawn((
                    Name::new(if transparent {
                        "transparent-canopy"
                    } else {
                        "rear-cockpit-fairing"
                    }),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(cockpit_materials[surface.material as usize].clone()),
                    Transform::IDENTITY,
                    RenderLayers::layer(0).with(1),
                ));
                if transparent {
                    child.insert((NoFrustumCulling, NotShadowCaster));
                }
            }
            for (surface_index, (surface, positions)) in wing_geometry
                .surfaces()
                .iter()
                .zip(wing_frame.surface_positions())
                .enumerate()
            {
                let side_index = surface_index / 2;
                let complete_mesh = main_wing_mesh(surface, positions);
                let mut material_meshes = Vec::new();
                for range in surface.material_ranges() {
                    let mut mesh = complete_mesh.clone();
                    mesh.insert_indices(Indices::U32(
                        surface.indices()[range.start..range.start + range.count].to_vec(),
                    ));
                    let handle = meshes.add(mesh);
                    material_meshes.push(handle.clone());
                    let material = match range.material {
                        WingMaterial::Cover => wing_cover[side_index].clone(),
                        WingMaterial::Underside => wing_underside.clone(),
                        WingMaterial::Film => wing_film.clone(),
                    };
                    let mut child = parent.spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(material),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(0).with(1),
                    ));
                    if range.material == WingMaterial::Film {
                        child.insert(NotShadowCaster);
                    }
                }
                wing_surfaces.push(MainWingSurface {
                    complete_mesh,
                    material_meshes,
                });
            }
            parent
                .spawn((
                    WorldProjection::HorizontalTail,
                    horizontal_tail.transform,
                    Visibility::Visible,
                ))
                .with_children(|tail| {
                    tail.spawn((
                        Mesh3d(meshes.add(rectangular_surface(horizontal_tail.vertices))),
                        MeshMaterial3d(white.clone()),
                        RenderLayers::layer(0).with(1),
                    ));
                });
            parent
                .spawn((
                    WorldProjection::VerticalTail,
                    vertical_tail.transform,
                    Visibility::Visible,
                ))
                .with_children(|tail| {
                    tail.spawn((
                        Mesh3d(meshes.add(rectangular_surface(vertical_tail.vertices))),
                        MeshMaterial3d(white),
                        RenderLayers::layer(0).with(1),
                    ));
                });
        });
    commands.insert_resource(MainWingProjection {
        geometry: wing_geometry,
        frame: wing_frame,
        surfaces: wing_surfaces,
    });
    commands.spawn((
        WorldProjection::FlightCamera,
        Camera3d::default(),
        IsDefaultUiCamera,
        Tonemapping::Reinhard,
        Projection::Perspective(PerspectiveProjection {
            far: 200_000.0,
            fov: birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32,
            ..default()
        }),
        Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y),
    ));
}

pub(crate) fn project_world(
    session: Res<NativeSession>,
    camera: Res<CameraMode>,
    mut history: ResMut<RenderHistory>,
    fixed: Res<Time<Fixed>>,
    observer: Option<Res<VerificationObserver>>,
    mut transforms: Query<(&WorldProjection, &mut Transform)>,
    mut water: ResMut<Assets<WaterMaterial>>,
) {
    let display = session.display_state();
    let preview = session.prepared_display_state();
    let aircraft_display = display.as_ref().or(preview.as_ref());
    let running = session.game.core_session().snapshot().phase()
        == birdman_game_core::SessionPhase::FlightRunning;
    if !running {
        history.reset();
    }
    let render_sample = aircraft_display.map(|display| {
        let exact = RenderSample::from_display(display);
        if running {
            history
                .interpolated(fixed.overstep_fraction_f64())
                .unwrap_or(exact)
        } else {
            exact
        }
    });
    let projected = render_sample.map_or_else(
        || exhibition_aircraft_transform(launch_venue().expect("登録launch venueが不正").platform),
        |sample| sample.aircraft,
    );
    let projected_camera = display.as_ref().and(render_sample).map_or_else(
        || Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y),
        |sample| {
            camera_transform(
                sample.aircraft,
                sample.pilot_position_m,
                session.initial_pilot_position_m,
                camera.chase,
                camera.look,
            )
        },
    );
    let projected_camera = observer
        .as_ref()
        .filter(|_| running)
        .map_or(projected_camera, |observer| observer.0);
    for (projection, mut transform) in &mut transforms {
        match projection {
            WorldProjection::Aircraft => *transform = projected,
            WorldProjection::FlightCamera => *transform = projected_camera,
            WorldProjection::HorizontalTail => {
                transform.rotation = Quat::from_rotation_x(
                    render_sample.map_or(0.0, |sample| sample.elevator_rad as f32),
                )
            }
            WorldProjection::VerticalTail => {
                transform.rotation = Quat::from_rotation_y(
                    -render_sample.map_or(0.0, |sample| sample.rudder_rad as f32),
                )
            }
            WorldProjection::SkyDome => transform.translation = projected_camera.translation,
            WorldProjection::LakeSurface => {
                let center = patch_center(projected_camera.translation);
                transform.translation = Vec3::new(center.x, 0.0, center.y);
            }
        }
    }
    let time = render_sample.map_or(0.0, |sample| sample.simulation_time_seconds as f32);
    for (_, material) in water.iter_mut() {
        material.project_camera(projected_camera.translation, time);
    }
}

fn exhibition_aircraft_transform(platform: LaunchPlatform) -> Transform {
    Transform::from_xyz(0.0, platform.front_lip_above_water_meters as f32, 0.0)
        .with_rotation(Quat::from_rotation_y(-platform.heading_rad() as f32))
}

#[cfg(test)]
mod tests {
    use super::super::native_session::{FlightInput, MenuAction};
    use super::*;
    use birdman_game_session::application::SessionCommand;

    #[test]
    fn wing_textures_decode_the_registered_decal_and_upload_srgb_mips() {
        let images = wing_cover_images();
        assert_ne!(images[0].data, images[1].data);
        for image in &images {
            assert_eq!(
                image.texture_descriptor.format,
                TextureFormat::Rgba8UnormSrgb
            );
            assert_eq!([image.width(), image.height()], [1024, 256]);
            assert_eq!(image.texture_descriptor.mip_level_count, 11);
            let expected_bytes: usize = (0..11)
                .map(|level| {
                    (1024_u32 >> level).max(1) as usize * (256_u32 >> level).max(1) as usize * 4
                })
                .sum();
            assert_eq!(image.data.as_ref().unwrap().len(), expected_bytes);
            let ImageSampler::Descriptor(sampler) = &image.sampler else {
                panic!("Missing wing sampler");
            };
            assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
            assert_eq!(sampler.mag_filter, bevy::image::ImageFilterMode::Linear);
            assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
            assert_eq!(
                sampler.address_mode_u,
                bevy::image::ImageAddressMode::ClampToEdge
            );
            let rgba = &image.data.as_ref().unwrap()[..wing_texture::SIDE_BYTES];
            assert!(rgba.chunks_exact(4).all(|pixel| pixel[3] == 255));
            let leading_panel = &rgba[31 * 1024 * 4..97 * 1024 * 4];
            assert!(leading_panel.chunks_exact(4).any(|pixel| pixel[0] > 180));
        }
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .init_resource::<NativeSession>()
            .add_systems(Startup, setup_world);
        app.update();
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let cover: Vec<_> = materials
            .iter()
            .filter_map(|(_, material)| {
                material
                    .base_color_texture
                    .as_ref()
                    .map(|texture| (material, texture))
            })
            .collect();
        assert_eq!(cover.len(), 2);
        assert_ne!(cover[0].1, cover[1].1);
        for (material, _) in cover {
            assert_eq!(material.base_color, Color::WHITE);
            assert_eq!(material.cull_mode, None);
            assert!(material.double_sided);
        }
        assert!(materials.iter().any(|(_, material)| material.base_color
            == Color::srgb_u8(242, 243, 237)
            && material.base_color_texture.is_none()));
    }

    fn assert_main_wing_projection(world: &World) {
        let wing = world.resource::<MainWingProjection>();
        let meshes = world.resource::<Assets<Mesh>>();
        assert_eq!(wing.surfaces.len(), 4);
        for (surface, positions) in wing.surfaces.iter().zip(wing.frame.surface_positions()) {
            let expected_normals = surface
                .complete_mesh
                .attribute(Mesh::ATTRIBUTE_NORMAL)
                .unwrap()
                .as_float3()
                .unwrap();
            assert!(
                expected_normals
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            for handle in &surface.material_meshes {
                let mesh = meshes.get(handle).unwrap();
                assert_eq!(
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                        .as_float3()
                        .unwrap(),
                    positions
                );
                assert_eq!(
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                        .unwrap()
                        .as_float3()
                        .unwrap(),
                    expected_normals
                );
            }
        }
    }

    fn projection_app() -> App {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<CameraMode>()
            .init_resource::<FlightInput>()
            .init_resource::<RenderHistory>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                birdman_game_core::PHYSICS_HZ,
            )))
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(FixedUpdate, super::super::advance_physics)
            .add_systems(Update, project_world);
        for projection in [
            WorldProjection::Aircraft,
            WorldProjection::FlightCamera,
            WorldProjection::HorizontalTail,
            WorldProjection::VerticalTail,
            WorldProjection::SkyDome,
            WorldProjection::LakeSurface,
        ] {
            app.world_mut().spawn((projection, Transform::default()));
        }
        {
            let mut materials = app.world_mut().resource_mut::<Assets<WaterMaterial>>();
            materials.add(WaterMaterial::registered(false).unwrap());
            materials.add(WaterMaterial::registered(true).unwrap());
        }
        app
    }

    fn projected_transform(app: &mut App, projection: WorldProjection) -> Transform {
        let mut query = app.world_mut().query::<(&WorldProjection, &Transform)>();
        query
            .iter(app.world())
            .find_map(|(kind, transform)| (*kind == projection).then_some(*transform))
            .unwrap()
    }

    fn set_render_fraction(app: &mut App, fraction: f64) {
        let mut fixed = app.world_mut().resource_mut::<Time<Fixed>>();
        let timestep = fixed.timestep();
        fixed.discard_overstep(std::time::Duration::MAX);
        fixed.accumulate_overstep(timestep.mul_f64(fraction));
    }

    fn assert_sample_projection(app: &mut App, sample: RenderSample, live_camera: bool) {
        let aircraft = projected_transform(app, WorldProjection::Aircraft);
        assert!(aircraft.translation.distance(sample.aircraft.translation) < 1.0e-6);
        assert!((aircraft.rotation.dot(sample.aircraft.rotation).abs() - 1.0).abs() < 1.0e-6);
        let horizontal = projected_transform(app, WorldProjection::HorizontalTail);
        let vertical = projected_transform(app, WorldProjection::VerticalTail);
        assert!(
            (horizontal
                .rotation
                .dot(Quat::from_rotation_x(sample.elevator_rad as f32))
                .abs()
                - 1.0)
                .abs()
                < 1.0e-6
        );
        assert!(
            (vertical
                .rotation
                .dot(Quat::from_rotation_y(-sample.rudder_rad as f32))
                .abs()
                - 1.0)
                .abs()
                < 1.0e-6
        );
        let camera = projected_transform(app, WorldProjection::FlightCamera);
        let expected_camera = if live_camera {
            camera_transform(
                sample.aircraft,
                sample.pilot_position_m,
                app.world()
                    .resource::<NativeSession>()
                    .initial_pilot_position_m,
                false,
                Vec2::ZERO,
            )
        } else {
            Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y)
        };
        assert!(camera.translation.distance(expected_camera.translation) < 1.0e-6);
        assert!((camera.rotation.dot(expected_camera.rotation).abs() - 1.0).abs() < 1.0e-6);
        assert_eq!(
            projected_transform(app, WorldProjection::SkyDome).translation,
            camera.translation
        );
        let materials = app.world().resource::<Assets<WaterMaterial>>();
        assert_eq!(materials.len(), 2);
        for (_, material) in materials.iter() {
            assert_eq!(material.camera_time.truncate(), camera.translation);
            assert_eq!(
                material.camera_time.w,
                sample.simulation_time_seconds as f32
            );
            if material.geometry_patch.w > 0.0 {
                assert_eq!(
                    material.geometry_patch.truncate().truncate(),
                    patch_center(camera.translation)
                );
            }
        }
        let center = patch_center(camera.translation);
        assert_eq!(
            projected_transform(app, WorldProjection::LakeSurface).translation,
            Vec3::new(center.x, 0.0, center.y)
        );
    }

    #[test]
    fn verification_observer_changes_only_render_camera_and_preserves_the_common_sample() {
        let mut app = projection_app();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            assert_eq!(
                session.game.core_session().snapshot().phase(),
                birdman_game_core::SessionPhase::FlightRunning
            );
        }
        app.update();
        let sample = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .display_state()
                .unwrap(),
        );
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let count = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        let observer = Transform::from_xyz(31.0, 14.0, -47.0).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            0.3,
            0.1,
            0.2,
        ));
        app.insert_resource(VerificationObserver(observer));
        app.update();
        assert_eq!(
            projected_transform(&mut app, WorldProjection::FlightCamera),
            observer
        );
        assert_eq!(
            projected_transform(&mut app, WorldProjection::Aircraft),
            sample.aircraft
        );
        assert_eq!(
            projected_transform(&mut app, WorldProjection::SkyDome).translation,
            observer.translation
        );
        let center = patch_center(observer.translation);
        assert_eq!(
            projected_transform(&mut app, WorldProjection::LakeSurface).translation,
            Vec3::new(center.x, 0.0, center.y)
        );
        for (_, material) in app.world().resource::<Assets<WaterMaterial>>().iter() {
            assert_eq!(
                material.camera_time,
                observer
                    .translation
                    .extend(sample.simulation_time_seconds as f32)
            );
        }
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            before
        );
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .flight_record()
                .unwrap()
                .sample_count(),
            count
        );
        app.world_mut().remove_resource::<VerificationObserver>();
        app.update();
        assert_sample_projection(&mut app, sample, true);
        app.insert_resource(VerificationObserver(observer));
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Pause)
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, sample, true);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, sample, true);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .execute_ui_command(SessionCommand::Retry {})
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, sample, false);
    }

    #[test]
    fn projection_seeds_the_first_interval_and_uses_one_fraction_for_every_render_value() {
        let mut app = projection_app();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
        }
        let initial = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .prepared_display_state()
                .unwrap(),
        );
        app.update();
        assert_sample_projection(&mut app, initial, false);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Launch)
            .unwrap();
        for _step in 0..2 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .presentation_frame(1.0);
            app.update();
            assert_sample_projection(&mut app, initial, false);
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .presentation_frame(1.0);
        app.update();
        assert_sample_projection(&mut app, initial, true);
        *app.world_mut().resource_mut::<FlightInput>() = FlightInput {
            nose_up: 0.6,
            turn_right: 0.3,
            pilot: Some(1.0),
        };
        app.world_mut().run_schedule(FixedUpdate);
        let current = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .display_state()
                .unwrap(),
        );
        let history = app.world().resource::<RenderHistory>();
        assert_eq!(history.samples.as_ref().unwrap().previous, initial);
        assert_eq!(history.samples.as_ref().unwrap().current, current);
        assert!(current.pilot_position_m > initial.pilot_position_m);
        assert_ne!(current.elevator_rad, initial.elevator_rad);
        assert_ne!(current.rudder_rad, initial.rudder_rad);
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let samples = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .sample_count();
        for fraction in [0.0, 0.5, 1.0] {
            let blend = |previous: f64, next: f64| previous * (1.0 - fraction) + next * fraction;
            let expected = RenderSample {
                aircraft: Transform::from_translation(
                    initial
                        .aircraft
                        .translation
                        .lerp(current.aircraft.translation, fraction as f32),
                )
                .with_rotation(
                    initial
                        .aircraft
                        .rotation
                        .slerp(current.aircraft.rotation, fraction as f32),
                ),
                pilot_position_m: blend(initial.pilot_position_m, current.pilot_position_m),
                elevator_rad: blend(initial.elevator_rad, current.elevator_rad),
                rudder_rad: blend(initial.rudder_rad, current.rudder_rad),
                simulation_time_seconds: blend(
                    initial.simulation_time_seconds,
                    current.simulation_time_seconds,
                ),
            };
            set_render_fraction(&mut app, fraction);
            app.update();
            assert_sample_projection(&mut app, expected, true);
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.game.core_session().snapshot(), before);
            assert_eq!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                samples
            );
        }
    }

    #[test]
    fn pause_resume_and_retry_use_exact_samples_without_old_interpolation() {
        let mut app = projection_app();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        let initial = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .display_state()
                .unwrap(),
        );
        app.world_mut().run_schedule(FixedUpdate);
        let current = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .display_state()
                .unwrap(),
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Pause)
            .unwrap();
        let paused = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        for fraction in [0.0, 0.5, 1.0] {
            set_render_fraction(&mut app, fraction);
            app.update();
            assert_sample_projection(&mut app, current, true);
            assert!(app.world().resource::<RenderHistory>().samples.is_none());
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .snapshot(),
                paused
            );
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Resume)
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, current, true);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world()
                .resource::<RenderHistory>()
                .samples
                .as_ref()
                .unwrap()
                .previous,
            current
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        let aborted = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .display_state()
                .unwrap(),
        );
        app.update();
        assert_sample_projection(&mut app, aborted, true);
        assert!(app.world().resource::<RenderHistory>().samples.is_none());
        app.world_mut()
            .resource_mut::<NativeSession>()
            .execute_ui_command(SessionCommand::Retry {})
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, initial, false);
        assert!(app.world().resource::<RenderHistory>().samples.is_none());
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        app.update();
        assert_sample_projection(&mut app, initial, true);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world()
                .resource::<RenderHistory>()
                .samples
                .as_ref()
                .unwrap()
                .previous,
            initial
        );
    }

    #[test]
    fn fractional_water_contact_projects_the_authoritative_terminal_sample_at_every_fraction() {
        let mut app = projection_app();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        for _step in 0..birdman_game_session::DEFAULT_MAXIMUM_FLIGHT_TICKS {
            if app
                .world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot()
                .phase()
                == birdman_game_core::SessionPhase::Result
            {
                break;
            }
            app.world_mut().run_schedule(FixedUpdate);
        }
        let terminal = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        let result = terminal.result().unwrap();
        assert_eq!(
            result.reason,
            birdman_game_core::SessionEndReason::WaterContact
        );
        let birdman_game_core::SessionTerminalState::TailWaterContact(contact) = result.state
        else {
            panic!("Expected fractional water contact");
        };
        assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
        let exact = RenderSample::from_display(
            &app.world()
                .resource::<NativeSession>()
                .display_state()
                .unwrap(),
        );
        assert_eq!(
            exact.simulation_time_seconds,
            (contact.interval_start_tick() as f64 + contact.fraction())
                / f64::from(birdman_game_core::PHYSICS_HZ)
        );
        let finalization = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .flight_record()
            .unwrap()
            .finalization()
            .unwrap();
        for fraction in [0.0, 0.5, 1.0] {
            set_render_fraction(&mut app, fraction);
            app.update();
            assert_sample_projection(&mut app, exact, true);
            assert!(app.world().resource::<RenderHistory>().samples.is_none());
            let session = app.world().resource::<NativeSession>();
            assert_eq!(session.game.core_session().snapshot(), terminal);
            assert_eq!(
                session
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .finalization(),
                Some(finalization)
            );
        }
    }

    #[test]
    fn world_setup_shares_the_registered_sun_with_water_sky_and_pbr_light() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .init_resource::<NativeSession>()
            .add_systems(Startup, setup_world)
            .add_systems(Update, project_main_wing);
        app.update();
        assert_main_wing_projection(app.world());
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            assert!(main_wing_airspeed(&session).is_nan());
            session.action(MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            assert_eq!(
                main_wing_airspeed(&session),
                session
                    .game
                    .core_session()
                    .telemetry()
                    .unwrap()
                    .unwrap()
                    .airspeed_mps
            );
        }
        app.update();
        assert_main_wing_projection(app.world());
        let mut cameras = app.world_mut().query::<(&WorldProjection, &Projection)>();
        let Projection::Perspective(projection) = cameras
            .iter(app.world())
            .find_map(|(kind, projection)| {
                (*kind == WorldProjection::FlightCamera).then_some(projection)
            })
            .unwrap()
        else {
            panic!("Flight camera requires a perspective projection");
        };
        assert_eq!(
            projection.fov,
            birdman_game_session::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
        );
        use bevy::camera::CameraProjection;
        for (width, height) in [(1280.0, 720.0), (360.0, 300.0)] {
            let mut projection = projection.clone();
            projection.update(width, height);
            let scale = 1.0 / (projection.fov / 2.0).tan();
            for point in [
                Vec4::new(0.0, 0.0, -10.0, 1.0),
                Vec4::new(1.0, 2.0, -10.0, 1.0),
                Vec4::new(-5.0, -3.0, -10.0, 1.0),
            ] {
                let clip = projection.get_clip_from_view() * point;
                assert!(
                    (clip.x / clip.w - point.x * scale / projection.aspect_ratio / -point.z).abs()
                        < 1.0e-6
                );
                assert!((clip.y / clip.w - point.y * scale / -point.z).abs() < 1.0e-6);
            }
        }
        let expected = Vec3::new(0.405_579_78, 0.819_152_06, 0.405_579_78);
        let mut lights = app.world_mut().query::<(&DirectionalLight, &Transform)>();
        let (light, transform) = lights.single(app.world()).unwrap();
        assert_eq!(light.illuminance, 18_000.0);
        assert!(light.shadow_maps_enabled);
        assert!((transform.forward().as_vec3() + expected).length() < 1.0e-6);
        let materials = app.world().resource::<Assets<WaterMaterial>>();
        let mut material_modes = Vec::new();
        for (_, material) in materials.iter() {
            assert!(material.sun_cloud.truncate().distance(expected) < 1.0e-6);
            material_modes.push(material.waves_sky.w);
        }
        material_modes.sort_by(f32::total_cmp);
        assert_eq!(material_modes, vec![0.0, 0.0, 1.0]);
    }

    #[test]
    fn cockpit_projects_shared_surfaces_and_tubes_under_the_aircraft() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Startup, setup_world);
        app.update();
        let expected = CockpitGeometry::new();
        let mut remaining_rods: Vec<_> = expected.rods.iter().collect();
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &Mesh3d, &Transform, &ChildOf)>();
        let meshes = world.resource::<Assets<Mesh>>();
        let mut surfaces = 0;
        for (name, handle, transform, parent) in query.iter(world) {
            if let Some(index) = ["transparent-canopy", "rear-cockpit-fairing"]
                .iter()
                .position(|expected| *expected == name.as_str())
            {
                assert!(
                    *world.get::<WorldProjection>(parent.parent()).unwrap()
                        == WorldProjection::Aircraft
                );
                let mesh = meshes.get(&handle.0).unwrap();
                assert_eq!(
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                        .unwrap()
                        .as_float3()
                        .unwrap(),
                    expected.surfaces[index].positions
                );
                assert_eq!(
                    mesh.indices().unwrap().iter().collect::<Vec<_>>(),
                    expected.surfaces[index]
                        .indices
                        .iter()
                        .map(|index| *index as usize)
                        .collect::<Vec<_>>()
                );
                assert!(
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                        .unwrap()
                        .as_float3()
                        .unwrap()
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite())
                );
                assert_eq!(*transform, Transform::IDENTITY);
                surfaces += 1;
            } else if matches!(name.as_str(), "cockpit-frame" | "canopy-rim-segment") {
                assert!(
                    *world.get::<WorldProjection>(parent.parent()).unwrap()
                        == WorldProjection::Aircraft
                );
                let actual = [-0.5, 0.5].map(|height| transform.transform_point(Vec3::Y * height));
                let index = remaining_rods
                    .iter()
                    .position(|rod| {
                        actual.iter().zip(rod.endpoints).all(|(actual, expected)| {
                            actual.distance(Vec3::from_array(expected.map(|value| value as f32)))
                                < 1.0e-6
                        })
                    })
                    .unwrap();
                remaining_rods.swap_remove(index);
            }
        }
        assert_eq!(surfaces, 2);
        assert!(remaining_rods.is_empty());
    }

    #[test]
    fn reflection_layer_contains_airframe_meshes_and_sun_but_excludes_water_and_terrain() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Startup, setup_world);
        app.update();
        let world = app.world_mut();
        let mut meshes = world.query::<(
            &Mesh3d,
            Option<&RenderLayers>,
            Option<&EnvironmentSurface>,
            Option<&NotShadowCaster>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        )>();
        let mut reflected_meshes = 0;
        let mut transparent_films = 0;
        let mut excluded_meshes = 0;
        let materials = world.resource::<Assets<StandardMaterial>>();
        for (_, layers, water, shadow_exclusion, material) in meshes.iter(world) {
            if layers.is_some_and(|layers| layers.intersects(&RenderLayers::layer(1))) {
                assert!(water.is_none());
                let material = materials.get(&material.unwrap().0).unwrap();
                if material.alpha_mode == AlphaMode::Blend {
                    assert!(shadow_exclusion.is_some());
                    transparent_films += 1;
                } else {
                    assert!(shadow_exclusion.is_none());
                }
                reflected_meshes += 1;
            } else {
                excluded_meshes += 1;
            }
        }
        assert_eq!(reflected_meshes, 43);
        assert_eq!(transparent_films, 3);
        assert!(excluded_meshes >= 5);
        let mut lights = world.query_filtered::<&RenderLayers, With<EnvironmentSun>>();
        assert_eq!(
            *lights.single(world).unwrap(),
            RenderLayers::layer(0).with(1)
        );
        let mut default_ui = world.query_filtered::<&WorldProjection, With<IsDefaultUiCamera>>();
        assert!(*default_ui.single(world).unwrap() == WorldProjection::FlightCamera);
    }

    #[test]
    fn prepared_world_projection_preserves_menu_camera_and_continues_into_the_exact_launch() {
        let mut app = App::new();
        app.init_resource::<NativeSession>()
            .init_resource::<CameraMode>()
            .init_resource::<RenderHistory>()
            .init_resource::<Time<Fixed>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Update, project_world);
        let aircraft = app
            .world_mut()
            .spawn((WorldProjection::Aircraft, Transform::default()))
            .id();
        let camera = app
            .world_mut()
            .spawn((WorldProjection::FlightCamera, Transform::default()))
            .id();
        let horizontal_tail = app
            .world_mut()
            .spawn((WorldProjection::HorizontalTail, Transform::default()))
            .id();
        let vertical_tail = app
            .world_mut()
            .spawn((WorldProjection::VerticalTail, Transform::default()))
            .id();
        app.update();
        let menu_camera = *app.world().entity(camera).get::<Transform>().unwrap();
        let platform = launch_venue().unwrap().platform;
        let exhibition = *app.world().entity(aircraft).get::<Transform>().unwrap();
        let [north, east] = platform.horizontal_direction_ned();
        assert!(
            (exhibition.rotation * Vec3::NEG_Z).distance(Vec3::new(
                east as f32,
                0.0,
                -north as f32
            )) < 1.0e-5
        );
        assert_eq!(
            exhibition.translation.y,
            platform.front_lip_above_water_meters as f32
        );
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
        }
        let initial = app
            .world()
            .resource::<NativeSession>()
            .prepared_display_state()
            .unwrap();
        let prepared_snapshot = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .snapshot();
        app.update();
        assert_eq!(
            app.world()
                .resource::<NativeSession>()
                .game
                .core_session()
                .snapshot(),
            prepared_snapshot
        );
        assert_eq!(
            *app.world().entity(aircraft).get::<Transform>().unwrap(),
            aircraft_transform(initial.state)
        );
        assert_eq!(
            *app.world().entity(camera).get::<Transform>().unwrap(),
            menu_camera
        );
        assert_eq!(
            app.world()
                .entity(horizontal_tail)
                .get::<Transform>()
                .unwrap()
                .rotation,
            Quat::from_rotation_x(initial.incidence.elevator_rad() as f32)
        );
        assert_eq!(
            app.world()
                .entity(vertical_tail)
                .get::<Transform>()
                .unwrap()
                .rotation,
            Quat::from_rotation_y(-initial.incidence.rudder_rad() as f32)
        );
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Launch)
            .unwrap();
        for _step in 0..2 {
            app.world_mut()
                .resource_mut::<NativeSession>()
                .presentation_frame(1.0);
            app.update();
            assert_eq!(
                *app.world().entity(aircraft).get::<Transform>().unwrap(),
                aircraft_transform(initial.state)
            );
            assert_eq!(
                *app.world().entity(camera).get::<Transform>().unwrap(),
                menu_camera
            );
            assert_eq!(
                app.world()
                    .resource::<NativeSession>()
                    .game
                    .core_session()
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                0
            );
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .presentation_frame(1.0);
        app.update();
        assert_eq!(
            *app.world().entity(aircraft).get::<Transform>().unwrap(),
            aircraft_transform(initial.state)
        );
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Abort).unwrap();
            session
                .execute_ui_command(SessionCommand::Retry {})
                .unwrap();
        }
        app.update();
        assert_eq!(
            *app.world().entity(aircraft).get::<Transform>().unwrap(),
            aircraft_transform(initial.state)
        );
        assert_eq!(
            *app.world().entity(camera).get::<Transform>().unwrap(),
            menu_camera
        );
    }
    #[test]
    fn native_tail_presentation_selects_the_shared_default_model_identity() {
        let preparation = birdman_game_session::HybridSessionPreparation::try_default().unwrap();
        let (configuration, record_identity) = preparation.into_parts();
        assert_eq!(
            configuration.identity().aircraft_model_version,
            HybridMockDefinition::MODEL_VERSION
        );
        assert_eq!(
            record_identity.aircraft_configuration_id,
            HybridMockDefinition::CONFIGURATION_ID
        );
    }

    #[test]
    fn tail_surface_projection_uses_versioned_core_geometry_without_changing_incidence() {
        let definition = HybridMockDefinition::try_new().unwrap();
        for surface in definition.surfaces().unwrap() {
            let geometry = surface.geometry();
            if geometry.role() == HybridSurfaceRole::MainWing {
                assert!(project_tail_surface(geometry).is_err());
                continue;
            }
            let projected = project_tail_surface(geometry).unwrap();
            let expected = match geometry.role() {
                HybridSurfaceRole::HorizontalTail => Vec3::new(0.0, -0.1, 3.6),
                HybridSurfaceRole::VerticalTail => Vec3::new(0.0, 0.1, 1.8),
                HybridSurfaceRole::MainWing => unreachable!(),
            };
            assert!(projected.transform.translation.distance(expected) < 1.0e-6);
            assert_eq!(projected.transform.rotation, Quat::IDENTITY);
            for (section_index, leading_index, trailing_index) in [(0, 0, 3), (1, 1, 2)] {
                let section = geometry.sections()[section_index];
                let point = section.quarter_chord().components();
                for (vertex_index, chord_fraction) in
                    [(leading_index, -0.25), (trailing_index, 0.75)]
                {
                    let expected = Vec3::new(
                        point[1] as f32,
                        -point[2] as f32,
                        (-point[0] + chord_fraction * section.chord_m()) as f32,
                    );
                    let actual = projected
                        .transform
                        .transform_point(Vec3::from_array(projected.vertices[vertex_index]));
                    assert!(actual.distance(expected) < 1.0e-6);
                }
            }
        }
    }

    #[test]
    fn launch_platform_front_lip_and_slope_match_the_shared_ned_venue() {
        let platform = launch_venue().unwrap().platform;
        let thickness = 0.35;
        let slope = platform.downward_slope_degrees.to_radians() as f32;
        let deck_length = platform.length_meters as f32 / slope.cos();
        let transform = platform_transform(&platform, thickness);
        let front = transform.transform_point(Vec3::new(0.0, thickness / 2.0, -deck_length / 2.0));
        let rear = transform.transform_point(Vec3::new(0.0, thickness / 2.0, deck_length / 2.0));
        assert!(
            front.distance(Vec3::new(
                0.0,
                platform.front_lip_above_water_meters as f32,
                0.0,
            )) < 1.0e-5
        );
        let [north, east] = platform.horizontal_direction_ned();
        assert!(
            rear.distance(Vec3::new(
                -(east * platform.length_meters) as f32,
                platform.front_lip_above_water_meters as f32
                    + platform.length_meters as f32 * slope.tan(),
                (north * platform.length_meters) as f32,
            )) < 1.0e-5
        );
    }

    #[test]
    fn interpolation_changes_only_render_projection() {
        let history = RenderHistory {
            samples: Some(RenderSamples {
                previous: RenderSample {
                    aircraft: Transform::from_xyz(0.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_y(0.2)),
                    pilot_position_m: -0.2,
                    elevator_rad: 0.1,
                    rudder_rad: -0.2,
                    simulation_time_seconds: 0.04,
                },
                current: RenderSample {
                    aircraft: Transform::from_xyz(10.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_y(0.6)),
                    pilot_position_m: 0.2,
                    elevator_rad: 0.3,
                    rudder_rad: 0.2,
                    simulation_time_seconds: 0.05,
                },
            }),
        };
        for fraction in [0.0, 0.5, 1.0] {
            let sample = history.interpolated(fraction).unwrap();
            assert_eq!(
                sample.aircraft.translation,
                Vec3::new(10.0 * fraction as f32, 0.0, 0.0)
            );
            let expected_rotation = Quat::from_rotation_y(0.2 + 0.4 * fraction as f32);
            assert!((sample.aircraft.rotation.dot(expected_rotation).abs() - 1.0).abs() < 1.0e-6);
            assert!((sample.pilot_position_m - (-0.2 + 0.4 * fraction)).abs() < 1.0e-12);
            assert!((sample.elevator_rad - (0.1 + 0.2 * fraction)).abs() < 1.0e-12);
            assert!((sample.rudder_rad - (-0.2 + 0.4 * fraction)).abs() < 1.0e-12);
            assert!((sample.simulation_time_seconds - (0.04 + 0.01 * fraction)).abs() < 1.0e-12);
        }
    }
    #[test]
    fn registered_terrain_and_masks_have_matching_finite_geometry() {
        let terrain: Terrain =
            serde_json::from_str(include_str!("../../../assets/biwa-terrain.json")).unwrap();
        let masks: LandMasks =
            serde_json::from_str(include_str!("../../../assets/biwa-land-mask.json")).unwrap();
        for patch in &terrain.fine_patches {
            let mask = masks.grids.iter().find(|mask| mask.id == patch.id).unwrap();
            assert!(terrain_mesh(&patch.grid, mask, &[]).is_ok());
        }
    }
}
