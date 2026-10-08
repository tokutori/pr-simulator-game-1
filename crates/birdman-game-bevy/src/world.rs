use super::{
    CameraMode,
    native_session::NativeSession,
    projection::{aircraft_transform, camera_transform},
    water::WaterMaterial,
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

#[derive(Component)]
pub(crate) enum WorldProjection {
    Aircraft,
    HorizontalTail,
    VerticalTail,
    FlightCamera,
    SkyDome,
}

#[derive(Resource, Default)]
pub(crate) struct RenderHistory {
    previous: Option<Transform>,
    current: Option<Transform>,
}

impl RenderHistory {
    pub(crate) fn capture(&mut self, state: birdman_game_core::FlightState) {
        let projected = aircraft_transform(state);
        self.previous = self.current.or(Some(projected));
        self.current = Some(projected);
    }
    pub(crate) fn reset(&mut self) {
        self.previous = None;
        self.current = None;
    }
    fn interpolated(&self, fraction: f32) -> Option<Transform> {
        let previous = self.previous?;
        let current = self.current?;
        Some(
            Transform::from_translation(previous.translation.lerp(current.translation, fraction))
                .with_rotation(previous.rotation.slerp(current.rotation, fraction)),
        )
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
    let surface = water.add(WaterMaterial::registered(false).expect("登録wavesが不正"));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(180_000.0, 180_000.0))),
        MeshMaterial3d(surface),
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
        Mesh3d(meshes.add(dome)),
        MeshMaterial3d(water.add(WaterMaterial::registered(true).expect("登録skyが不正"))),
        Transform::IDENTITY,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            ..default()
        },
        Transform::from_xyz(800.0, 1000.0, -300.0).looking_at(Vec3::ZERO, Vec3::Y),
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
            Transform::from_xyz(0.0, 10.0, 0.0),
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
    history: Res<RenderHistory>,
    fixed: Res<Time<Fixed>>,
    mut transforms: Query<(&WorldProjection, &mut Transform)>,
    mut water: ResMut<Assets<WaterMaterial>>,
) {
    let display = session.display_state();
    let projected = display
        .as_ref()
        .map_or(Transform::from_xyz(0.0, 10.0, 0.0), |display| {
            let exact = aircraft_transform(display.state);
            if session.game.snapshot().phase() == birdman_game_core::SessionPhase::FlightRunning {
                history
                    .interpolated(fixed.overstep_fraction())
                    .unwrap_or(exact)
            } else {
                exact
            }
        });
    let projected_camera = display.as_ref().map_or_else(
        || Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y),
        |display| {
            camera_transform(
                projected,
                display.state.pilot_position_m(),
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
                    display
                        .as_ref()
                        .map_or(0.0, |display| display.incidence.elevator_rad() as f32),
                )
            }
            WorldProjection::VerticalTail => {
                transform.rotation = Quat::from_rotation_y(
                    -display
                        .as_ref()
                        .map_or(0.0, |display| display.incidence.rudder_rad() as f32),
                )
            }
            WorldProjection::SkyDome => transform.translation = projected_camera.translation,
        }
    }
    let time = display.map_or(0.0, |display| {
        display.tick as f32 / birdman_game_core::PHYSICS_HZ as f32
    });
    for (_, material) in water.iter_mut() {
        material.camera_time = projected_camera.translation.extend(time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            previous: Some(Transform::from_xyz(0.0, 0.0, 0.0)),
            current: Some(Transform::from_xyz(10.0, 0.0, 0.0)),
        };
        assert_eq!(
            history.interpolated(0.5).unwrap().translation,
            Vec3::new(5.0, 0.0, 0.0)
        );
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
