use super::{
    CameraMode,
    environment::{EnvironmentSun, EnvironmentSurface, NativeEnvironment},
    native_session::{NativeSession, TailDisplay},
    projection::{aircraft_transform, camera_transform, sunlight_transform},
    water::{NearWaterSurface, WaterMaterial, far_mesh, near_mesh, patch_center},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    asset::RenderAssetUsages, core_pipeline::tonemapping::Tonemapping, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use birdman_game_core::{
    HybridMockConfiguration, HybridMockDefinition, HybridSection, HybridSurfaceGeometry,
    HybridSurfaceRole,
};
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
    let registered_water = WaterMaterial::registered(false).expect("登録wavesが不正");
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
            ..default()
        },
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
    let definition = HybridMockDefinition::try_new(HybridMockConfiguration::Playable)
        .expect("登録機体geometryが不正");
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
                MeshMaterial3d(frame.clone()),
                Transform::from_xyz(0.0, -0.2, 0.5),
            ));
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.55, 0.5, 0.7))),
                MeshMaterial3d(frame),
                Transform::from_xyz(0.0, -0.35, -0.25),
            ));
            let mut wing_vertices = Vec::new();
            for side in [-1.0_f32, 1.0] {
                for strip in 0..24 {
                    let inner = strip as f32 / 24.0;
                    let outer = (strip + 1) as f32 / 24.0;
                    let chord = |ratio: f32| 1.13 + (0.38 - 1.13) * ratio.powf(1.6);
                    let points = [
                        [side * inner * 10.4315, inner * 0.38, -chord(inner) * 0.25],
                        [side * outer * 10.4315, outer * 0.38, -chord(outer) * 0.25],
                        [side * outer * 10.4315, outer * 0.38, chord(outer) * 0.75],
                        [side * inner * 10.4315, inner * 0.38, chord(inner) * 0.75],
                    ];
                    wing_vertices.push(points);
                }
            }
            for vertices in wing_vertices {
                parent.spawn((
                    Mesh3d(meshes.add(rectangular_surface(vertices))),
                    MeshMaterial3d(white.clone()),
                    Transform::IDENTITY,
                ));
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
                    ));
                });
        });
    commands.spawn((
        WorldProjection::FlightCamera,
        Camera3d::default(),
        Tonemapping::Reinhard,
        Projection::Perspective(PerspectiveProjection {
            far: 200_000.0,
            fov: 70.0_f32.to_radians(),
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
    mut transforms: Query<(&WorldProjection, &mut Transform)>,
    mut water: ResMut<Assets<WaterMaterial>>,
) {
    let display = session.display_state();
    let preview = session.prepared_display_state();
    let aircraft_display = display.as_ref().or(preview.as_ref());
    let running = session.game.snapshot().phase() == birdman_game_core::SessionPhase::FlightRunning;
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
                .countdown(1.0);
            app.update();
            assert_sample_projection(&mut app, initial, false);
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .countdown(1.0);
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
        let before = app.world().resource::<NativeSession>().game.snapshot();
        let samples = app
            .world()
            .resource::<NativeSession>()
            .game
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
            assert_eq!(session.game.snapshot(), before);
            assert_eq!(
                session.game.flight_record().unwrap().sample_count(),
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
                session.countdown(1.0);
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
        let paused = app.world().resource::<NativeSession>().game.snapshot();
        for fraction in [0.0, 0.5, 1.0] {
            set_render_fraction(&mut app, fraction);
            app.update();
            assert_sample_projection(&mut app, current, true);
            assert!(app.world().resource::<RenderHistory>().samples.is_none());
            assert_eq!(
                app.world().resource::<NativeSession>().game.snapshot(),
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
            .action(MenuAction::Retry)
            .unwrap();
        app.update();
        assert_sample_projection(&mut app, initial, false);
        assert!(app.world().resource::<RenderHistory>().samples.is_none());
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Launch).unwrap();
            for _step in 0..3 {
                session.countdown(1.0);
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
                session.countdown(1.0);
            }
        }
        for _step in 0..birdman_game_session::DEFAULT_MAXIMUM_FLIGHT_TICKS {
            if app
                .world()
                .resource::<NativeSession>()
                .game
                .snapshot()
                .phase()
                == birdman_game_core::SessionPhase::Result
            {
                break;
            }
            app.world_mut().run_schedule(FixedUpdate);
        }
        let terminal = app.world().resource::<NativeSession>().game.snapshot();
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
            assert_eq!(session.game.snapshot(), terminal);
            assert_eq!(
                session.game.flight_record().unwrap().finalization(),
                Some(finalization)
            );
        }
    }

    #[test]
    fn world_setup_shares_the_registered_sun_with_water_sky_and_pbr_light() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Startup, setup_world);
        app.update();
        let expected = Vec3::new(0.405_579_78, 0.819_152_06, 0.405_579_78);
        let mut lights = app.world_mut().query::<(&DirectionalLight, &Transform)>();
        let (light, transform) = lights.single(app.world()).unwrap();
        assert_eq!(light.illuminance, 18_000.0);
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
        let prepared_snapshot = app.world().resource::<NativeSession>().game.snapshot();
        app.update();
        assert_eq!(
            app.world().resource::<NativeSession>().game.snapshot(),
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
                .countdown(1.0);
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
                    .flight_record()
                    .unwrap()
                    .sample_count(),
                0
            );
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .countdown(1.0);
        app.update();
        assert_eq!(
            *app.world().entity(aircraft).get::<Transform>().unwrap(),
            aircraft_transform(initial.state)
        );
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Abort).unwrap();
            session.action(MenuAction::Retry).unwrap();
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
            HybridMockConfiguration::Playable.model_version()
        );
        assert_eq!(
            record_identity.aircraft_configuration_id,
            HybridMockConfiguration::Playable.configuration_id()
        );
    }

    #[test]
    fn tail_surface_projection_uses_versioned_core_geometry_without_changing_incidence() {
        for (configuration, horizontal_arm) in [
            (HybridMockConfiguration::Standard, 1.8),
            (HybridMockConfiguration::ZeroDihedralOracle, 1.8),
            (HybridMockConfiguration::Playable, 3.6),
        ] {
            let definition = HybridMockDefinition::try_new(configuration).unwrap();
            for surface in definition.surfaces().unwrap() {
                let geometry = surface.geometry();
                if geometry.role() == HybridSurfaceRole::MainWing {
                    assert!(project_tail_surface(geometry).is_err());
                    continue;
                }
                let projected = project_tail_surface(geometry).unwrap();
                let expected = match geometry.role() {
                    HybridSurfaceRole::HorizontalTail => Vec3::new(0.0, -0.1, horizontal_arm),
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
