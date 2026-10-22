#[path = "world/environment.rs"]
mod environment;

#[cfg(feature = "verify-harness")]
use super::projection::VerificationObserver;
use super::{
    CameraMode,
    native_session::NativeSession,
    projection::{camera_transform, render_aircraft_transform, replay_camera_transform},
    terrain_haze::TerrainMaterial,
    water::{WaterMaterial, patch_center},
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
    light::NotShadowCaster,
    mesh::Indices,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use birdman_game_app::{LaunchPlatform, launch_venue};
#[cfg(test)]
use birdman_game_core::HybridSurfaceRole;
use birdman_game_core::WingBendingFactor;
use birdman_game_presentation::render_pose::{RenderTailGeometry, RenderWingDeformation};
use birdman_game_scene::airframe_structure::{
    AirframeStructure, StructuralAttachment, StructuralPartKind,
};
use birdman_game_scene::cockpit_geometry::{CockpitGeometry, CockpitMaterial};
use birdman_game_scene::lake_sky::LAKE_SCENE_LIGHTING;
use birdman_game_scene::tail_geometry::{TailGeometry, TailMaterial};
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct RenderSample {
    aircraft: Transform,
    pose: birdman_game_presentation::render_pose::FlightRenderPose,
    elevator_rad: f64,
    rudder_rad: f64,
    simulation_time_seconds: f64,
}
impl RenderSample {
    fn from_pose(pose: birdman_game_presentation::render_pose::FlightRenderPose) -> Self {
        Self {
            aircraft: render_aircraft_transform(&pose),
            pose,
            elevator_rad: pose.controls.physical_incidence.horizontal_tail_radians,
            rudder_rad: pose.controls.physical_incidence.vertical_tail_radians,
            simulation_time_seconds: pose.simulation_time_seconds,
        }
    }
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

pub(crate) fn project_main_wing(
    mut session: ResMut<NativeSession>,
    mut wing: ResMut<MainWingProjection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: ParamSet<(MainWingVisibility, TailVisibility)>,
) {
    let (pose, valid) = match session.render_pose() {
        Ok(pose) => (pose, true),
        Err(error) => {
            if session.game.flight_failure().is_none() {
                session.game.fail_flight_adapter();
                eprintln!("Native render pose failed: {error}");
                session.notice = Some("表示を更新できないため飛行を停止した。".into());
            }
            (None, false)
        }
    };
    let geometry_visible = valid
        && pose
            .is_none_or(|pose| matches!(pose.tail_geometry, RenderTailGeometry::Available { .. }));
    let deformation = pose.map_or(RenderWingDeformation::Accepted { factor: 1.0 }, |pose| {
        pose.wing_deformation
    });
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
    let update = match deformation {
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

fn color_srgb(value: u32) -> Color {
    Color::srgb_u8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

pub(crate) fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut water: ResMut<Assets<WaterMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
) {
    let platform = launch_venue().expect("登録launch venueが不正").platform;
    environment::setup(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut water,
        &mut images,
        &mut terrain_materials,
        platform,
    );
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
                base_color: color_srgb(source.color()),
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
    #[cfg(feature = "verify-harness")] observer: Option<Res<VerificationObserver>>,
    mut transforms: Query<(&WorldProjection, &mut Transform, Option<&mut Projection>)>,
    mut water: ResMut<Assets<WaterMaterial>>,
) {
    let display_active = matches!(
        session.game.core_session().snapshot().phase(),
        birdman_game_core::SessionPhase::FlightRunning
            | birdman_game_core::SessionPhase::FlightPaused { .. }
            | birdman_game_core::SessionPhase::Result
            | birdman_game_core::SessionPhase::Replay
            | birdman_game_core::SessionPhase::Attract
    );
    #[cfg(feature = "verify-harness")]
    let running = session.game.core_session().snapshot().phase()
        == birdman_game_core::SessionPhase::FlightRunning;
    let render_sample = match session.render_pose() {
        Ok(pose) => pose.map(RenderSample::from_pose),
        Err(error) => {
            if session.game.flight_failure().is_none() {
                session.game.fail_flight_adapter();
                eprintln!("Native render pose failed: {error}");
                session.notice = Some("表示を更新できないため飛行を停止した。".into());
            }
            None
        }
    };
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
    let projected_camera = display_active
        .then_some(render_sample)
        .flatten()
        .map_or_else(
            || {
                Ok(Transform::from_xyz(15.0, 14.0, 22.0)
                    .looking_at(Vec3::new(0.0, 8.0, 0.0), Vec3::Y))
            },
            |sample| {
                if let Some(ref view) = record_camera {
                    return replay_camera_transform(view, camera.look);
                }
                camera_transform(&sample.pose, camera.chase, camera.look)
            },
        );
    let projected_camera = match projected_camera {
        Ok(pose) => pose,
        Err(cause) => {
            session.game.fail_flight_adapter();
            session.notice = Some(format!("視点の更新に失敗した: {cause}"));
            return;
        }
    };
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
#[path = "world/tests/mod.rs"]
mod tests;
