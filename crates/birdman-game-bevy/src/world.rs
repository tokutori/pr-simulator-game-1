#[cfg(feature = "verify-harness")]
use super::projection::VerificationObserver;
use super::{
    CameraMode,
    environment::{EnvironmentSun, EnvironmentSurface, EnvironmentTarget, NativeEnvironment},
    native_session::{NativeSession, TailDisplay},
    projection::{
        aircraft_transform, camera_transform, replay_camera_transform, sunlight_transform,
    },
    terrain_haze::{TerrainHazeExtension, TerrainMaterial},
    water::{NearWaterSurface, WaterMaterial, far_mesh, near_mesh, patch_center},
};
use bevy::ecs as bevy_ecs;
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        Exposure,
        visibility::{NoFrustumCulling, RenderLayers},
    },
    core_pipeline::tonemapping::Tonemapping,
    image::{CompressedImageFormats, ImageSampler, ImageSamplerDescriptor, ImageType},
    light::{CascadeShadowConfigBuilder, NotShadowCaster},
    mesh::Indices,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use birdman_game_app::{LaunchPlatform, launch_venue};
#[cfg(test)]
use birdman_game_core::HybridSurfaceRole;
use birdman_game_core::WingBendingFactor;
use birdman_game_presentation::render_pose::{
    RenderTailGeometry, RenderWingDeformation, render_tail_geometry, render_wing_deformation,
};
use birdman_game_scene::airframe_structure::{
    AirframeStructure, StructuralAttachment, StructuralPartKind,
};
use birdman_game_scene::cockpit_geometry::{CockpitGeometry, CockpitMaterial};
use birdman_game_scene::lake_sky::LAKE_SCENE_LIGHTING;
use birdman_game_scene::tail_geometry::{TailGeometry, TailMaterial};
use birdman_game_scene::venue_structures::{ShoreStructureMesh, registered_structures};
use birdman_game_scene::venue_terrain::{VenueTerrainMaterial, registered_terrain};
use birdman_game_scene::wing_geometry::{WingFrame, WingGeometry, WingMaterial, WingSurface};
use birdman_game_scene::wing_texture;

pub(super) const SCENE_SUN_ILLUMINANCE_LUX: f32 = 18_000.0;
const SCENE_EXPOSURE_SCALE: f32 =
    LAKE_SCENE_LIGHTING.sunlight_irradiance as f32 / SCENE_SUN_ILLUMINANCE_LUX;

pub(super) fn scene_exposure() -> Exposure {
    Exposure {
        ev100: (1.0 / (1.2 * SCENE_EXPOSURE_SCALE)).log2(),
    }
}

#[derive(Component, PartialEq, Eq)]
pub(crate) enum WorldProjection {
    Aircraft,
    HorizontalTail,
    VerticalTail,
    FlightCamera,
    SkyDome,
    LakeSurface,
}

#[derive(Component)]
pub(crate) struct RegisteredVenue;

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
    wing_bending_factor: Option<WingBendingFactor>,
}

impl RenderSample {
    fn from_display(display: &TailDisplay) -> Self {
        Self {
            aircraft: aircraft_transform(display.state),
            pilot_position_m: display.state.pilot_position_m(),
            elevator_rad: display.incidence.elevator_rad(),
            rudder_rad: display.incidence.rudder_rad(),
            simulation_time_seconds: display.tick / f64::from(birdman_game_core::PHYSICS_HZ),
            wing_bending_factor: display.wing_bending_factor,
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
            elevator_rad: if fraction == 0.0 {
                previous.elevator_rad
            } else {
                current.elevator_rad
            },
            rudder_rad: if fraction == 0.0 {
                previous.rudder_rad
            } else {
                current.rudder_rad
            },
            wing_bending_factor: if fraction == 0.0 {
                previous.wing_bending_factor
            } else {
                current.wing_bending_factor
            },
            simulation_time_seconds: interpolate(
                previous.simulation_time_seconds,
                current.simulation_time_seconds,
            ),
        })
    }
}

fn project_display_sample(
    display: &TailDisplay,
    running: bool,
    history: &RenderHistory,
    fraction: f64,
) -> RenderSample {
    let exact = RenderSample::from_display(display);
    if running {
        history.interpolated(fraction).unwrap_or(exact)
    } else {
        exact
    }
}

fn venue_terrain_mesh(source: &birdman_game_scene::venue_terrain::VenueTerrainMesh) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        source
            .positions_ned
            .iter()
            .map(|&[north, east, down]| [east, -down, -north])
            .collect::<Vec<_>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        source
            .normals_ned
            .iter()
            .map(|&[north, east, down]| [east, -down, -north])
            .collect::<Vec<_>>(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_COLOR,
        source
            .colors_linear
            .iter()
            .map(|&[red, green, blue]| [red, green, blue, 1.0])
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(source.indices.clone()))
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

#[derive(Resource)]
pub(crate) struct MainWingProjection {
    geometry: WingGeometry,
    frame: WingFrame,
    surfaces: Vec<MainWingSurface>,
    structure: AirframeStructure<'static>,
    members: Vec<StructuralProjection>,
    projection_failed: bool,
}

#[derive(Component)]
pub(crate) struct MainWingPart;

type MainWingVisibility<'world, 'state> =
    Query<'world, 'state, &'static mut Visibility, With<MainWingPart>>;
type TailVisibility<'world, 'state> =
    Query<'world, 'state, (&'static WorldProjection, &'static mut Visibility)>;

struct StructuralProjection {
    entity: Entity,
    handle: Handle<Mesh>,
    candidate: Mesh,
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

fn main_wing_deformation(
    session: &NativeSession,
    history: &RenderHistory,
    fraction: f64,
) -> RenderWingDeformation {
    let display = session
        .display_state()
        .or_else(|| session.prepared_display_state());
    let Some(display) = display else {
        return RenderWingDeformation::Accepted { factor: 1.0 };
    };
    let running = session.game.core_session().snapshot().phase()
        == birdman_game_core::SessionPhase::FlightRunning;
    let sample = project_display_sample(&display, running, history, fraction);
    render_wing_deformation(
        sample.wing_bending_factor,
        tail_presentation_geometry(session),
    )
}

fn tail_presentation_geometry(session: &NativeSession) -> RenderTailGeometry {
    let identity = session
        .game
        .display_control_identity()
        .zip(session.game.core_session().configuration_identity())
        .map(|(identity, scenario)| {
            (
                identity.aircraft_configuration_id.as_str(),
                scenario.aircraft_model_version,
            )
        });
    render_tail_geometry(identity).expect("registered aircraft metadata must remain valid")
}

pub(crate) fn project_main_wing(
    session: Res<NativeSession>,
    mut wing: ResMut<MainWingProjection>,
    mut meshes: ResMut<Assets<Mesh>>,
    history: Res<RenderHistory>,
    fixed: Res<Time<Fixed>>,
    mut visibility: ParamSet<(MainWingVisibility, TailVisibility)>,
) {
    let geometry_visible = session
        .display_state()
        .or_else(|| session.prepared_display_state())
        .is_none()
        || matches!(
            tail_presentation_geometry(&session),
            RenderTailGeometry::Available { .. }
        );
    for (projection, mut visible) in &mut visibility.p1() {
        if matches!(
            projection,
            WorldProjection::Aircraft
                | WorldProjection::HorizontalTail
                | WorldProjection::VerticalTail
        ) {
            *visible = if !geometry_visible {
                Visibility::Hidden
            } else if *projection == WorldProjection::Aircraft {
                Visibility::Visible
            } else {
                Visibility::Inherited
            };
        }
    }
    let MainWingProjection {
        geometry,
        frame,
        surfaces,
        structure,
        members,
        projection_failed,
    } = &mut *wing;
    let update = match main_wing_deformation(&session, &history, fixed.overstep_fraction_f64()) {
        RenderWingDeformation::Accepted { factor } => {
            let factor =
                WingBendingFactor::try_new(factor).expect("shared adopted coefficient is finite");
            match prepare_main_wing_projection(
                geometry,
                frame,
                surfaces,
                structure,
                members,
                factor,
                *projection_failed,
            ) {
                Ok(updated) => {
                    *projection_failed = false;
                    Some(updated)
                }
                Err(cause) => {
                    if !*projection_failed {
                        warn!("主翼変形を表示できない: {cause:?}");
                    }
                    *projection_failed = true;
                    None
                }
            }
        }
        RenderWingDeformation::Unavailable { .. } => None,
    };
    for mut visible in &mut visibility.p0() {
        *visible = if update.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !update.unwrap_or(false) {
        return;
    }
    for (surface, positions) in surfaces.iter().zip(frame.surface_positions()) {
        for handle in &surface.material_meshes {
            publish_mesh_positions(
                &mut meshes.get_mut(handle).expect("主翼meshがない"),
                &surface.complete_mesh,
                positions,
            );
        }
    }
    for (source, target) in structure.meshes().iter().zip(members) {
        if source.attachment() == StructuralAttachment::MainWing {
            publish_mesh_positions(
                &mut meshes.get_mut(&target.handle).expect("構造meshがない"),
                &target.candidate,
                source.positions(),
            );
        }
    }
}

fn prepare_main_wing_projection(
    geometry: &WingGeometry,
    frame: &mut WingFrame,
    surfaces: &mut [MainWingSurface],
    structure: &mut AirframeStructure<'_>,
    members: &mut [StructuralProjection],
    factor: WingBendingFactor,
    previously_failed: bool,
) -> Result<bool, &'static str> {
    let skin_updated = geometry
        .update_frame_with_factor(frame, factor)
        .map_err(|_| "主翼座標を表現できない")?;
    let members_updated = structure
        .update_with_factor(factor)
        .map_err(|_| "主翼構造を表現できない")?;
    if !skin_updated && !members_updated {
        return if previously_failed {
            Err("主翼のGPU投影を表現できない")
        } else {
            Ok(false)
        };
    }
    for (surface, positions) in surfaces.iter_mut().zip(frame.surface_positions()) {
        prepare_mesh_positions(&mut surface.complete_mesh, positions)?;
    }
    for (source, target) in structure.meshes().iter().zip(members) {
        if source.attachment() == StructuralAttachment::MainWing {
            prepare_mesh_positions(&mut target.candidate, source.positions())?;
        }
    }
    Ok(true)
}

fn prepare_mesh_positions(mesh: &mut Mesh, positions: &[[f32; 3]]) -> Result<(), &'static str> {
    let bevy::mesh::VertexAttributeValues::Float32x3(values) = mesh
        .attribute_mut(Mesh::ATTRIBUTE_POSITION)
        .expect("mesh positionsがない")
    else {
        panic!("mesh positionsはFloat32x3を要する");
    };
    values.copy_from_slice(positions);
    mesh.compute_area_weighted_normals();
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .unwrap()
        .as_float3()
        .unwrap();
    if normals.iter().any(|normal| {
        let normal = Vec3::from_array(*normal);
        !normal.is_finite() || normal.length_squared() < 0.5
    }) {
        return Err("主翼法線を表現できない");
    }
    Ok(())
}

fn publish_mesh_positions(mesh: &mut Mesh, candidate: &Mesh, positions: &[[f32; 3]]) {
    let bevy::mesh::VertexAttributeValues::Float32x3(values) = mesh
        .attribute_mut(Mesh::ATTRIBUTE_POSITION)
        .expect("mesh positionsがない")
    else {
        panic!("mesh positionsはFloat32x3を要する");
    };
    values.copy_from_slice(positions);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        candidate.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().clone(),
    );
}

fn structural_mesh(source: &birdman_game_scene::airframe_structure::StructuralMesh) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, source.positions().to_vec())
    .with_inserted_indices(Indices::U32(source.indices().to_vec()));
    mesh.compute_area_weighted_normals();
    mesh
}

fn structural_name(kind: StructuralPartKind) -> &'static str {
    match kind {
        StructuralPartKind::Spar => "main-wing-spar",
        StructuralPartKind::Rib => "airframe-rib",
        StructuralPartKind::Stringer => "balsa-stringer",
        StructuralPartKind::Foam => "airframe-foam",
        StructuralPartKind::MainBeam => "single-carbon-main-beam",
        StructuralPartKind::CockpitFrame => "cockpit-frame",
        StructuralPartKind::CanopyRim => "canopy-rim",
    }
}

fn wing_color(value: u32) -> Color {
    Color::srgb_u8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn shore_structure_mesh(source: &ShoreStructureMesh) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        source
            .positions_ned
            .iter()
            .map(|&[north, east, down]| [east, -down, -north])
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(source.indices.clone()));
    mesh.compute_area_weighted_normals();
    mesh
}

pub(crate) fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
) {
    commands.init_resource::<RenderHistory>();
    commands.insert_resource(GlobalAmbientLight {
        color: wing_color(LAKE_SCENE_LIGHTING.ambient_color_srgb),
        brightness: LAKE_SCENE_LIGHTING.ambient_irradiance as f32
            / (std::f32::consts::PI * SCENE_EXPOSURE_SCALE),
        ..default()
    });
    let environment = NativeEnvironment::try_default().expect("登録環境が不正");
    let sky = environment
        .condition_for(EnvironmentTarget::exhibition())
        .expect("登録環境が欠損")
        .sky;
    commands.insert_resource(environment);
    let land_materials = std::array::from_fn::<_, 3, _>(|_| {
        terrain_materials.add(TerrainMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                double_sided: true,
                cull_mode: None,
                fog_enabled: false,
                opaque_render_method: bevy::material::OpaqueRendererMethod::Forward,
                ..default()
            },
            extension: TerrainHazeExtension::try_new(sky.map(|sky| sky.visibility_m))
                .expect("登録terrain hazeが不正"),
        })
    });
    for surface in registered_terrain().expect("登録地形・湖岸・島が不正") {
        let material = match surface.material {
            VenueTerrainMaterial::Terrain => &land_materials[0],
            VenueTerrainMaterial::Canopy => &land_materials[1],
            VenueTerrainMaterial::Rocks => &land_materials[2],
        };
        commands.spawn((
            RegisteredVenue,
            Name::new(surface.name.clone()),
            Visibility::Inherited,
            Mesh3d(meshes.add(venue_terrain_mesh(surface))),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            NotShadowCaster,
            bevy::light::NotShadowReceiver,
        ));
    }
    for structure in registered_structures().expect("登録OSM構造物が不正") {
        commands.spawn((
            RegisteredVenue,
            Name::new(format!("lake-biwa-{}-features-osm", structure.kind.name())),
            Visibility::Inherited,
            Mesh3d(meshes.add(shore_structure_mesh(structure))),
            MeshMaterial3d(
                terrain_materials.add(TerrainMaterial {
                    base: StandardMaterial {
                        base_color: wing_color(structure.kind.color_srgb()),
                        perceptual_roughness: 1.0,
                        reflectance: 0.0,
                        double_sided: true,
                        cull_mode: None,
                        fog_enabled: false,
                        opaque_render_method: bevy::material::OpaqueRendererMethod::Forward,
                        ..default()
                    },
                    extension: TerrainHazeExtension::try_new(sky.map(|sky| sky.visibility_m))
                        .expect("登録構造物hazeが不正"),
                }),
            ),
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
            color: wing_color(LAKE_SCENE_LIGHTING.sunlight_color_srgb),
            illuminance: SCENE_SUN_ILLUMINANCE_LUX,
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
        RegisteredVenue,
        Visibility::Inherited,
        Mesh3d(meshes.add(Cuboid::new(
            platform.width_meters as f32,
            platform_thickness,
            deck_length,
        ))),
        MeshMaterial3d(platform_material),
        platform_transform(&platform, platform_thickness),
    ));
    let tail_materials = TailMaterial::ALL.map(|kind| {
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
    let wing_cover = wing_cover_images().map(|image| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(images.add(image)),
            reflectance: 0.0,
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
    let wing_film = tail_materials[TailMaterial::Film as usize].clone();
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
    let structure =
        AirframeStructure::new(birdman_game_app::registered_aircraft().expect("登録機体が不正"))
            .expect("登録構造geometryが不正");
    let mut members: Vec<_> = structure
        .meshes()
        .iter()
        .map(|source| {
            let candidate = structural_mesh(source);
            StructuralProjection {
                entity: Entity::PLACEHOLDER,
                handle: meshes.add(candidate.clone()),
                candidate,
            }
        })
        .collect();
    let structural_materials: Vec<_> = structure
        .meshes()
        .iter()
        .map(|source| {
            materials.add(StandardMaterial {
                base_color: wing_color(source.color()),
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        })
        .collect();
    let tails = TailGeometry::registered().expect("登録尾翼表示geometryが不正");
    commands
        .spawn((
            WorldProjection::Aircraft,
            exhibition_aircraft_transform(platform),
            Visibility::Visible,
        ))
        .with_children(|parent| {
            for (index, source) in structure.meshes().iter().enumerate() {
                if !matches!(
                    source.attachment(),
                    StructuralAttachment::Rigid | StructuralAttachment::MainWing
                ) {
                    continue;
                }
                let mut child = parent.spawn((
                    Name::new(structural_name(source.kind())),
                    Mesh3d(members[index].handle.clone()),
                    MeshMaterial3d(structural_materials[index].clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(0).with(1),
                ));
                if source.attachment() == StructuralAttachment::MainWing {
                    child.insert((MainWingPart, Visibility::Inherited));
                }
                members[index].entity = child.id();
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
                        MainWingPart,
                        Visibility::Inherited,
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
            for (projection, geometry) in [
                WorldProjection::HorizontalTail,
                WorldProjection::VerticalTail,
            ]
            .into_iter()
            .zip(tails)
            {
                let attachment = match projection {
                    WorldProjection::HorizontalTail => StructuralAttachment::HorizontalTail,
                    WorldProjection::VerticalTail => StructuralAttachment::VerticalTail,
                    _ => unreachable!(),
                };
                parent
                    .spawn((
                        projection,
                        Transform::from_translation(Vec3::from_array(
                            geometry.origin.map(|value| value as f32),
                        )),
                        Visibility::Inherited,
                    ))
                    .with_children(|tail| {
                        for patch in geometry.patches {
                            let mut mesh = tail.spawn((
                                Name::new(match patch.material {
                                    TailMaterial::Cover => "physical-tail-cover",
                                    TailMaterial::Film => "physical-tail-film",
                                }),
                                Mesh3d(meshes.add(rectangular_surface(patch.vertices))),
                                MeshMaterial3d(tail_materials[patch.material as usize].clone()),
                                RenderLayers::layer(0).with(1),
                            ));
                            if patch.material == TailMaterial::Film {
                                mesh.insert(NotShadowCaster);
                            }
                        }
                        for (index, source) in structure.meshes().iter().enumerate() {
                            if source.attachment() != attachment {
                                continue;
                            }
                            members[index].entity = tail
                                .spawn((
                                    Name::new(structural_name(source.kind())),
                                    Mesh3d(members[index].handle.clone()),
                                    MeshMaterial3d(structural_materials[index].clone()),
                                    Transform::IDENTITY,
                                    RenderLayers::layer(0).with(1),
                                ))
                                .id();
                        }
                    });
            }
        });
    commands.insert_resource(MainWingProjection {
        geometry: wing_geometry,
        frame: wing_frame,
        surfaces: wing_surfaces,
        structure,
        members,
        projection_failed: false,
    });
    commands.spawn((
        WorldProjection::FlightCamera,
        Camera3d::default(),
        IsDefaultUiCamera,
        Tonemapping::None,
        scene_exposure(),
        Projection::Perspective(PerspectiveProjection {
            far: 200_000.0,
            fov: birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians()
                as f32,
            ..default()
        }),
        Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y),
    ));
}

pub(crate) fn project_world(
    mut session: ResMut<NativeSession>,
    camera: Res<CameraMode>,
    mut history: ResMut<RenderHistory>,
    fixed: Res<Time<Fixed>>,
    #[cfg(feature = "verify-harness")] observer: Option<Res<VerificationObserver>>,
    mut transforms: Query<(&WorldProjection, &mut Transform, Option<&mut Projection>)>,
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
        project_display_sample(display, running, &history, fixed.overstep_fraction_f64())
    });
    let projected = render_sample.map_or_else(
        || exhibition_aircraft_transform(launch_venue().expect("登録launch venueが不正").platform),
        |sample| sample.aircraft,
    );
    let record_camera = if matches!(
        session.game.core_session().snapshot().phase(),
        birdman_game_core::SessionPhase::Replay | birdman_game_core::SessionPhase::Attract
    ) {
        session
            .game
            .core_session()
            .playback_record()
            .zip(session.game.core_session().playback_clock())
            .map(|(record, clock)| {
                birdman_game_presentation::replay_camera::replay_camera_view(
                    record,
                    session.game.display_control_identity(),
                    clock.time_seconds(),
                    session.ui.replay_camera,
                    birdman_game_presentation::game_ui::GameUiPresentationMode::Screen,
                    session.game.core_session().snapshot().phase()
                        == birdman_game_core::SessionPhase::Attract,
                )
            })
    } else {
        None
    };
    let record_camera = match record_camera.transpose() {
        Ok(view) => view,
        Err(cause) => {
            session.notice = Some(format!("記録cameraを表示できない: {cause:?}"));
            None
        }
    };
    let fov = record_camera
        .as_ref()
        .and_then(|view| view.cinematic())
        .map_or(
            birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
            |view| view.vertical_field_of_view_degrees(),
        )
        .to_radians() as f32;
    let projected_camera = display.as_ref().and(render_sample).map_or_else(
        || Transform::from_xyz(15.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y),
        |sample| {
            if let Some(ref view) = record_camera {
                return replay_camera_transform(
                    view,
                    sample.aircraft,
                    sample.pilot_position_m,
                    session.initial_pilot_position_m,
                    camera.look,
                );
            }
            camera_transform(
                sample.aircraft,
                sample.pilot_position_m,
                session.initial_pilot_position_m,
                camera.chase,
                camera.look,
            )
        },
    );
    #[cfg(feature = "verify-harness")]
    let projected_camera = observer
        .as_ref()
        .filter(|_| running)
        .map_or(projected_camera, |observer| observer.0);
    for (projection, mut transform, lens) in &mut transforms {
        match projection {
            WorldProjection::Aircraft => *transform = projected,
            WorldProjection::FlightCamera => {
                *transform = projected_camera;
                if let Some(mut lens) = lens
                    && let Projection::Perspective(perspective) = &mut *lens
                    && perspective.fov != fov
                {
                    perspective.fov = fov;
                }
            }
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
    use birdman_game_app::application::SessionCommand;

    #[test]
    fn wing_textures_decode_the_registered_decal_and_upload_srgb_mips() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<TerrainMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .init_resource::<NativeSession>()
            .add_systems(Startup, setup_world);
        app.update();
        let mut projected_meshes = app
            .world_mut()
            .query::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>();
        let world = app.world();
        let materials = world.resource::<Assets<StandardMaterial>>();
        let meshes = world.resource::<Assets<Mesh>>();
        let images = world.resource::<Assets<Image>>();
        let wing = world.resource::<MainWingProjection>();
        let cover: Vec<_> = materials
            .iter()
            .filter_map(|(id, material)| {
                material
                    .base_color_texture
                    .as_ref()
                    .map(|texture| (id, material, texture))
            })
            .collect();
        assert_eq!(cover.len(), 2);
        assert_ne!(cover[0].2, cover[1].2);
        let cover_images: Vec<_> = cover
            .iter()
            .map(|(id, _, texture)| {
                assert!(projected_meshes.iter(world).any(|(mesh, material)| {
                    material.0.id() == *id
                        && meshes.get(&mesh.0).is_some()
                        && wing
                            .surfaces
                            .iter()
                            .any(|surface| surface.material_meshes.contains(&mesh.0))
                }));
                images.get(*texture).unwrap()
            })
            .collect();
        assert_ne!(cover_images[0].data, cover_images[1].data);
        for image in cover_images {
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
        for (_, material, _) in cover {
            assert_eq!(material.base_color, Color::WHITE);
            assert_eq!(material.reflectance, 0.0);
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

    fn assert_main_wing_structure_projection(world: &World) {
        let wing = world.resource::<MainWingProjection>();
        let materials = world.resource::<Assets<StandardMaterial>>();
        let meshes = world.resource::<Assets<Mesh>>();
        assert_eq!(wing.structure.meshes().len(), wing.members.len());
        for (source, member) in wing.structure.meshes().iter().zip(&wing.members) {
            let entity = member.entity;
            assert_eq!(world.get::<Mesh3d>(entity).unwrap().0, member.handle);
            assert_eq!(
                *world.get::<Transform>(entity).unwrap(),
                Transform::IDENTITY
            );
            let parent = world.get::<ChildOf>(entity).unwrap().parent();
            let expected = match source.attachment() {
                StructuralAttachment::HorizontalTail => WorldProjection::HorizontalTail,
                StructuralAttachment::VerticalTail => WorldProjection::VerticalTail,
                _ => WorldProjection::Aircraft,
            };
            assert!(*world.get::<WorldProjection>(parent).unwrap() == expected);
            let mesh = meshes.get(&member.handle).unwrap();
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap(),
                source.positions()
            );
            assert_eq!(
                mesh.indices(),
                Some(&Indices::U32(source.indices().to_vec()))
            );
            assert!(
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                    .unwrap()
                    .as_float3()
                    .unwrap()
                    .iter()
                    .all(|normal| {
                        let normal = Vec3::from_array(*normal);
                        normal.is_finite() && normal.length_squared() > 0.5
                    })
            );
            let material = materials
                .get(
                    &world
                        .get::<MeshMaterial3d<StandardMaterial>>(entity)
                        .unwrap()
                        .0,
                )
                .unwrap();
            assert_eq!(material.base_color, wing_color(source.color()));
            assert!(material.double_sided);
            assert_eq!(material.cull_mode, None);
            assert_eq!(
                *world.get::<RenderLayers>(entity).unwrap(),
                RenderLayers::layer(0).with(1)
            );
            assert!(world.get::<NotShadowCaster>(entity).is_none());
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

    #[test]
    fn replay_rigs_project_shared_pose_and_lens_then_restore_screen_fov() {
        use birdman_game_presentation::{
            game_ui::{GameUiPresentationMode, GameUiReplayCamera},
            replay_camera::replay_camera_view,
        };
        let mut app = projection_app();
        let camera = {
            let mut query = app.world_mut().query::<(Entity, &WorldProjection)>();
            query
                .iter(app.world())
                .find_map(|(entity, kind)| {
                    (*kind == WorldProjection::FlightCamera).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .entity_mut(camera)
            .insert(Projection::Perspective(PerspectiveProjection::default()));
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            for _ in 0..3 {
                session.tick(FlightInput::default());
            }
            session.action(MenuAction::Abort).unwrap();
            session
                .execute_ui_command(SessionCommand::EnterReplay {})
                .unwrap();
        }
        let before = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .playback_record()
            .unwrap()
            .samples()
            .to_vec();
        for rig in GameUiReplayCamera::ALL {
            let expected = {
                let mut session = app.world_mut().resource_mut::<NativeSession>();
                session.ui.replay_camera = rig;
                session
                    .execute_ui_record_action(
                        birdman_game_app::application::RecordCommand::SeekReplay {
                            time_seconds: 0.015,
                        },
                    )
                    .unwrap();
                let record = session.game.core_session().playback_record().unwrap();
                replay_camera_view(
                    record,
                    session.game.display_control_identity(),
                    0.015,
                    rig,
                    GameUiPresentationMode::Screen,
                    false,
                )
                .unwrap()
            };
            app.update();
            let session = app.world().resource::<NativeSession>();
            let sample = RenderSample::from_display(&session.display_state().unwrap());
            let expected_transform = replay_camera_transform(
                &expected,
                sample.aircraft,
                sample.pilot_position_m,
                session.initial_pilot_position_m,
                Vec2::ZERO,
            );
            assert!(session.notice.is_none());
            assert_eq!(
                before,
                session
                    .game
                    .core_session()
                    .playback_record()
                    .unwrap()
                    .samples()
            );
            assert_eq!(
                projected_transform(&mut app, WorldProjection::FlightCamera),
                expected_transform
            );
            let Projection::Perspective(lens) = app.world().get::<Projection>(camera).unwrap()
            else {
                panic!("Perspective projection required")
            };
            let expected_fov = expected.cinematic().map_or(
                birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES,
                |view| view.vertical_field_of_view_degrees(),
            );
            assert_eq!(lens.fov, expected_fov.to_radians() as f32);
            assert_eq!(
                projected_transform(&mut app, WorldProjection::Aircraft),
                sample.aircraft
            );
        }
        app.world_mut()
            .resource_mut::<NativeSession>()
            .execute_ui_command(SessionCommand::LeaveReplay {})
            .unwrap();
        app.update();
        let Projection::Perspective(lens) = app.world().get::<Projection>(camera).unwrap() else {
            panic!("Perspective projection required")
        };
        assert_eq!(
            lens.fov,
            birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
        );
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

    #[cfg(feature = "verify-harness")]
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
                elevator_rad: if fraction == 0.0 {
                    initial.elevator_rad
                } else {
                    current.elevator_rad
                },
                rudder_rad: if fraction == 0.0 {
                    initial.rudder_rad
                } else {
                    current.rudder_rad
                },
                wing_bending_factor: if fraction == 0.0 {
                    initial.wing_bending_factor
                } else {
                    current.wing_bending_factor
                },
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
        for _step in 0..birdman_game_app::DEFAULT_MAXIMUM_FLIGHT_TICKS {
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

    fn wing_projection_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<TerrainMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .init_resource::<NativeSession>()
            .init_resource::<RenderHistory>()
            .init_resource::<Time<Fixed>>()
            .add_systems(Startup, setup_world)
            .add_systems(Update, project_main_wing);
        app.update();
        app
    }

    #[test]
    fn main_wing_visibility_tracks_accepted_unrepresentable_and_unrecorded_sources() {
        let mut app = wing_projection_app();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            session.action(MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
        }
        let display = app
            .world()
            .resource::<NativeSession>()
            .display_state()
            .unwrap();
        let initial = RenderSample::from_display(&display);
        app.world_mut().resource_mut::<RenderHistory>().samples = Some(RenderSamples {
            previous: initial,
            current: RenderSample {
                wing_bending_factor: Some(WingBendingFactor::try_new(f64::MAX).unwrap()),
                ..initial
            },
        });
        set_render_fraction(&mut app, 0.5);
        app.update();
        let assert_visible = |app: &mut App, expected: Visibility| {
            let wing = app.world().resource::<MainWingProjection>();
            let expected_count = wing
                .structure
                .meshes()
                .iter()
                .filter(|source| source.attachment() == StructuralAttachment::MainWing)
                .count()
                + wing
                    .surfaces
                    .iter()
                    .map(|surface| surface.material_meshes.len())
                    .sum::<usize>();
            let mut query = app
                .world_mut()
                .query_filtered::<&Visibility, With<MainWingPart>>();
            assert_eq!(query.iter(app.world()).count(), expected_count);
            assert!(query.iter(app.world()).all(|value| *value == expected));
        };
        assert_visible(&mut app, Visibility::Hidden);
        assert!(
            app.world()
                .resource::<MainWingProjection>()
                .projection_failed
        );
        app.world_mut()
            .resource_mut::<RenderHistory>()
            .samples
            .as_mut()
            .unwrap()
            .current
            .wing_bending_factor = Some(WingBendingFactor::try_new(1.21).unwrap());
        app.update();
        assert_visible(&mut app, Visibility::Inherited);
        assert!(
            !app.world()
                .resource::<MainWingProjection>()
                .projection_failed
        );
        assert_main_wing_projection(app.world());
        assert_main_wing_structure_projection(app.world());
        app.world_mut()
            .resource_mut::<RenderHistory>()
            .samples
            .as_mut()
            .unwrap()
            .current
            .wing_bending_factor = Some(WingBendingFactor::try_new(1.0e30).unwrap());
        for _ in 0..2 {
            app.update();
            assert_visible(&mut app, Visibility::Hidden);
            assert!(
                app.world()
                    .resource::<MainWingProjection>()
                    .projection_failed
            );
            let mut transforms = app
                .world_mut()
                .query_filtered::<&Transform, With<MainWingPart>>();
            assert!(transforms.iter(app.world()).all(
                |transform| transform.translation.is_finite()
                    && transform.rotation.is_finite()
                    && transform.scale.is_finite()
            ));
        }
        app.world_mut()
            .resource_mut::<RenderHistory>()
            .samples
            .as_mut()
            .unwrap()
            .current
            .wing_bending_factor = Some(WingBendingFactor::try_new(1.21).unwrap());
        app.update();
        assert_visible(&mut app, Visibility::Inherited);
        app.world_mut()
            .resource_mut::<NativeSession>()
            .action(MenuAction::Abort)
            .unwrap();
        app.world_mut()
            .resource_mut::<NativeSession>()
            .open_stored_record(include_str!(
                "../../../tools/game-check/fixtures/legacy-physics-three-record.json"
            ))
            .unwrap();
        app.update();
        assert_visible(&mut app, Visibility::Hidden);
        assert!(
            !app.world()
                .resource::<MainWingProjection>()
                .projection_failed
        );
        let legacy = app
            .world()
            .resource::<NativeSession>()
            .game
            .core_session()
            .playback_record()
            .unwrap();
        let unknown = birdman_game_core::FlightRecord::try_from_finalized_samples(
            legacy.header(),
            legacy.samples().to_vec(),
            legacy.finalization().unwrap(),
        )
        .unwrap();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session
                .game
                .execute(
                    birdman_game_app::application::SessionCommand::LeaveReplay {},
                    birdman_game_app::application::BackendReadiness::Ready,
                )
                .unwrap();
            session.game.open_archived_replay(unknown).unwrap();
        }
        app.update();
        let mut tails = app.world_mut().query::<(&WorldProjection, &Visibility)>();
        assert_eq!(
            tails
                .iter(app.world())
                .filter(|(projection, visibility)| matches!(
                    projection,
                    WorldProjection::HorizontalTail | WorldProjection::VerticalTail
                ) && **visibility == Visibility::Hidden)
                .count(),
            2
        );
    }

    #[test]
    fn world_setup_shares_the_registered_sun_with_water_sky_and_pbr_light() {
        let mut app = wing_projection_app();
        assert_main_wing_projection(app.world());
        assert_main_wing_structure_projection(app.world());
        let mesh_count = app.world().resource::<Assets<Mesh>>().len();
        let material_count = app.world().resource::<Assets<StandardMaterial>>().len();
        {
            let mut session = app.world_mut().resource_mut::<NativeSession>();
            session.action(MenuAction::Start).unwrap();
            session.action(MenuAction::Prepare).unwrap();
            let prepared_factor = session
                .prepared_display_state()
                .unwrap()
                .wing_bending_factor
                .unwrap();
            assert_eq!(
                main_wing_deformation(&session, &RenderHistory::default(), 0.0),
                RenderWingDeformation::Accepted {
                    factor: prepared_factor.value()
                }
            );
            session.action(MenuAction::Launch).unwrap();
            for _ in 0..3 {
                session.presentation_frame(1.0);
            }
            assert_eq!(
                main_wing_deformation(&session, &RenderHistory::default(), 0.0),
                RenderWingDeformation::Accepted {
                    factor: session
                        .display_state()
                        .unwrap()
                        .wing_bending_factor
                        .unwrap()
                        .value()
                }
            );
        }
        app.update();
        assert_main_wing_projection(app.world());
        assert_main_wing_structure_projection(app.world());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), mesh_count);
        assert_eq!(
            app.world().resource::<Assets<StandardMaterial>>().len(),
            material_count
        );
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
            birdman_game_presentation::SCREEN_VERTICAL_FIELD_OF_VIEW_DEGREES.to_radians() as f32
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
        assert_eq!(
            light.color,
            wing_color(LAKE_SCENE_LIGHTING.sunlight_color_srgb)
        );
        assert!((transform.forward().as_vec3() + expected).length() < 1.0e-6);
        let illuminance = light.illuminance;
        let mut exposure_query = app
            .world_mut()
            .query_filtered::<(&Exposure, &Tonemapping), With<Camera3d>>();
        let (exposure, tone_mapping) = exposure_query.single(app.world()).unwrap();
        assert_eq!(*tone_mapping, Tonemapping::None);
        let scale = exposure.exposure();
        assert!(
            (illuminance * scale - LAKE_SCENE_LIGHTING.sunlight_irradiance as f32).abs() < 1.0e-6
        );
        let ambient = app.world().resource::<GlobalAmbientLight>();
        assert_eq!(
            ambient.color,
            wing_color(LAKE_SCENE_LIGHTING.ambient_color_srgb)
        );
        assert!(
            (ambient.brightness * std::f32::consts::PI * scale
                - LAKE_SCENE_LIGHTING.ambient_irradiance as f32)
                .abs()
                < 1.0e-6
        );
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
            .init_resource::<Assets<TerrainMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Startup, setup_world);
        app.update();
        let expected = CockpitGeometry::new();
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
            }
        }
        assert_eq!(surfaces, 2);
        assert_main_wing_structure_projection(app.world());
    }

    #[test]
    fn reflection_layer_contains_only_airframe_meshes_and_sunlight() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<TerrainMaterial>>()
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
            Option<&bevy::light::NotShadowReceiver>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&MeshMaterial3d<TerrainMaterial>>,
            Option<&Name>,
            &Transform,
            Option<&RegisteredVenue>,
        )>();
        let mut reflected_meshes = 0;
        let mut main_beams = 0;
        let mut transparent_films = 0;
        let mut excluded_meshes = 0;
        let mut terrain_meshes = 0;
        let mut structures = 0;
        let materials = world.resource::<Assets<StandardMaterial>>();
        let terrain_materials = world.resource::<Assets<TerrainMaterial>>();
        let mesh_assets = world.resource::<Assets<Mesh>>();
        for (
            mesh,
            layers,
            water,
            shadow_exclusion,
            receiver_exclusion,
            material,
            terrain_material,
            name,
            transform,
            structure,
        ) in meshes.iter(world)
        {
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
                if name.is_some_and(|name| name.as_str() == "single-carbon-main-beam") {
                    main_beams += 1;
                    assert_eq!(material.base_color, Color::srgb_u8(48, 59, 58));
                }
            } else {
                excluded_meshes += 1;
                if let Some(handle) = terrain_material {
                    let material = terrain_materials.get(&handle.0).unwrap();
                    if structure.is_some()
                        && name.is_some_and(|name| name.as_str().ends_with("-features-osm"))
                    {
                        let source = registered_structures()
                            .unwrap()
                            .iter()
                            .find(|source| {
                                name.unwrap().as_str()
                                    == format!("lake-biwa-{}-features-osm", source.kind.name())
                            })
                            .unwrap();
                        assert_eq!(
                            material.base.base_color,
                            wing_color(source.kind.color_srgb())
                        );
                        assert_eq!(material.base.reflectance, 0.0);
                        assert!(material.base.double_sided);
                        assert!(material.base.cull_mode.is_none());
                        assert_eq!(material.base.perceptual_roughness, 1.0);
                        assert!(!material.base.fog_enabled);
                        assert_eq!(
                            material.extension,
                            TerrainHazeExtension::try_new(Some(25_000.0)).unwrap()
                        );
                        assert!(shadow_exclusion.is_none());
                        assert!(receiver_exclusion.is_none());
                        assert!(layers.is_none());
                        assert_eq!(*transform, Transform::IDENTITY);
                        let mesh = mesh_assets.get(mesh.0.id()).unwrap();
                        let expected_positions: Vec<_> = source
                            .positions_ned
                            .iter()
                            .map(|&[north, east, down]| [east, -down, -north])
                            .collect();
                        assert_eq!(
                            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                                .unwrap()
                                .as_float3()
                                .unwrap(),
                            expected_positions
                        );
                        assert_eq!(mesh.indices(), Some(&Indices::U32(source.indices.clone())));
                        let normals = mesh
                            .attribute(Mesh::ATTRIBUTE_NORMAL)
                            .unwrap()
                            .as_float3()
                            .unwrap();
                        assert!(normals.iter().all(|normal| {
                            let normal = Vec3::from_array(*normal);
                            normal.is_finite() && (normal.length() - 1.0).abs() < 1e-5
                        }));
                        assert!(Vec3::from_array(normals[0]).distance(Vec3::Y) < 1e-5);
                        assert!(expected_positions.iter().zip(normals).any(
                            |(position, normal)| {
                                *position == expected_positions[0] && normal[1].abs() < 1e-5
                            }
                        ));
                        structures += 1;
                        continue;
                    }
                    assert_eq!(material.base.base_color, Color::WHITE);
                    assert_eq!(material.base.perceptual_roughness, 1.0);
                    assert_eq!(material.base.reflectance, 0.0);
                    assert!(material.base.double_sided);
                    assert!(material.base.cull_mode.is_none());
                    assert!(!material.base.fog_enabled);
                    assert_eq!(
                        material.extension,
                        TerrainHazeExtension::try_new(Some(25_000.0)).unwrap()
                    );
                    assert!(shadow_exclusion.is_some());
                    assert!(receiver_exclusion.is_some());
                    assert!(structure.is_some());
                    assert!(layers.is_none());
                    assert_eq!(*transform, Transform::IDENTITY);
                    terrain_meshes += 1;
                }
            }
        }
        assert_eq!(terrain_meshes, registered_terrain().unwrap().len());
        assert_eq!(structures, 3);
        let wing = world.resource::<MainWingProjection>();
        let surface_count: usize = wing
            .surfaces
            .iter()
            .map(|surface| surface.material_meshes.len())
            .sum();
        let tail_skin_count: usize = TailGeometry::registered()
            .unwrap()
            .iter()
            .map(|tail| tail.patches.len())
            .sum();
        assert_eq!(
            reflected_meshes,
            surface_count
                + wing.structure.meshes().len()
                + CockpitGeometry::new().surfaces.len()
                + tail_skin_count
        );
        assert_eq!(main_beams, 1);
        assert_eq!(transparent_films, 5);
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
        let preparation = birdman_game_app::HybridSessionPreparation::try_default().unwrap();
        let (configuration, record_identity) = preparation.into_parts();
        let definition = birdman_game_app::registered_aircraft().unwrap();
        assert_eq!(
            configuration.identity().aircraft_model_version,
            definition.document().model_version
        );
        assert_eq!(
            record_identity.aircraft_configuration_id,
            definition.document().configuration_id
        );
    }

    #[test]
    fn tail_surface_projection_uses_versioned_core_geometry_without_changing_incidence() {
        let definition = birdman_game_app::registered_aircraft().unwrap();
        for surface in definition.surfaces().unwrap() {
            let geometry = surface.geometry();
            if geometry.role() == HybridSurfaceRole::MainWing {
                assert!(
                    TailGeometry::from_shape(definition.shape().surface(geometry.role())).is_err()
                );
                continue;
            }
            let projected =
                TailGeometry::from_shape(definition.shape().surface(geometry.role())).unwrap();
            let expected = match geometry.role() {
                HybridSurfaceRole::HorizontalTail => Vec3::new(0.0, -0.1, 3.6),
                HybridSurfaceRole::VerticalTail => Vec3::new(0.0, 0.1, 1.8),
                HybridSurfaceRole::MainWing => unreachable!(),
            };
            let transform = Transform::from_translation(Vec3::from_array(
                projected.origin.map(|value| value as f32),
            ));
            assert!(transform.translation.distance(expected) < 1.0e-6);
            let vertices = [
                projected.patches[0].vertices[0],
                projected.patches[0].vertices[1],
                projected.patches[1].vertices[2],
                projected.patches[1].vertices[3],
            ];
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
                    let actual =
                        transform.transform_point(Vec3::from_array(vertices[vertex_index]));
                    assert!(actual.distance(expected) < 1.0e-6);
                }
            }
        }
    }

    #[test]
    fn tails_project_shared_patches_materials_and_ribs_under_their_physical_parents() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<TerrainMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<WaterMaterial>>()
            .add_systems(Startup, setup_world);
        app.update();
        let expected = TailGeometry::registered().unwrap();
        let world = app.world_mut();
        let mut query = world.query::<(
            &Name,
            &ChildOf,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &Transform,
        )>();
        let meshes = world.resource::<Assets<Mesh>>();
        let materials = world.resource::<Assets<StandardMaterial>>();
        let mut remaining_patches = expected
            .each_ref()
            .map(|tail| tail.patches.iter().collect::<Vec<_>>());
        for (name, parent, handle, material, transform) in query.iter(world) {
            let index = match world.get::<WorldProjection>(parent.parent()) {
                Some(WorldProjection::HorizontalTail) => 0,
                Some(WorldProjection::VerticalTail) => 1,
                _ => continue,
            };
            let material = materials.get(&material.0).unwrap();
            if matches!(name.as_str(), "physical-tail-cover" | "physical-tail-film") {
                let vertices = meshes
                    .get(&handle.0)
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap();
                let matched = remaining_patches[index]
                    .iter()
                    .position(|patch| vertices == patch.vertices)
                    .unwrap();
                let patch = remaining_patches[index].swap_remove(matched);
                assert_eq!(
                    material.alpha_mode == AlphaMode::Blend,
                    patch.material == TailMaterial::Film
                );
                assert!((material.base_color.alpha() - patch.material.opacity()).abs() < 1.0e-6);
                assert_eq!(*transform, Transform::IDENTITY);
            }
        }
        assert!(remaining_patches.iter().all(Vec::is_empty));
        assert_main_wing_structure_projection(app.world());
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
                    wing_bending_factor: Some(WingBendingFactor::try_new(0.3).unwrap()),
                },
                current: RenderSample {
                    aircraft: Transform::from_xyz(10.0, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_y(0.6)),
                    pilot_position_m: 0.2,
                    elevator_rad: 0.3,
                    rudder_rad: 0.2,
                    simulation_time_seconds: 0.05,
                    wing_bending_factor: Some(WingBendingFactor::try_new(1.6).unwrap()),
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
            assert_eq!(sample.elevator_rad, if fraction == 0.0 { 0.1 } else { 0.3 });
            assert_eq!(sample.rudder_rad, if fraction == 0.0 { -0.2 } else { 0.2 });
            assert_eq!(
                sample.wing_bending_factor.unwrap().value(),
                if fraction == 0.0 { 0.3 } else { 1.6 }
            );
            assert!((sample.simulation_time_seconds - (0.04 + 0.01 * fraction)).abs() < 1.0e-12);
        }
    }
    #[test]
    fn registered_terrain_attributes_are_projected_without_geometry_or_normal_recalculation() {
        for source in registered_terrain().unwrap() {
            let mesh = venue_terrain_mesh(source);
            let projected = |points: &[[f32; 3]]| {
                points
                    .iter()
                    .map(|&[north, east, down]| [east, -down, -north])
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap(),
                projected(&source.positions_ned)
            );
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                    .unwrap()
                    .as_float3()
                    .unwrap(),
                projected(&source.normals_ned)
            );
            let colors = source
                .colors_linear
                .iter()
                .map(|&[red, green, blue]| [red, green, blue, 1.0])
                .collect::<Vec<_>>();
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_COLOR),
                Some(&bevy::mesh::VertexAttributeValues::Float32x4(colors))
            );
            assert_eq!(mesh.indices(), Some(&Indices::U32(source.indices.clone())));
        }
    }
}
