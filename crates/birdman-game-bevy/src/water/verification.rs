use super::{NearWaterSurface, WaterMaterial, WaterQuality};
use bevy::{asset as bevy_asset, ecs as bevy_ecs, reflect as bevy_reflect, render as bevy_render};
use bevy::{
    camera::{RenderTarget, ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureFormat,
            TextureUsages,
        },
    },
    shader::ShaderDefVal,
};

const SIDE: u32 = 16;
const CASES: [(&str, WaterQuality, bool, bool); 5] = [
    ("flat", WaterQuality::High, true, true),
    ("low", WaterQuality::Low, false, false),
    ("medium", WaterQuality::Medium, false, false),
    ("high", WaterQuality::High, false, false),
    ("far", WaterQuality::High, false, true),
];
const PROBE_COUNT: usize = CASES.len() * ProbeOutput::ALL.len();

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ProbeOutput {
    Displacement,
    Normal,
    Color,
}

impl ProbeOutput {
    const ALL: [Self; 3] = [Self::Displacement, Self::Normal, Self::Color];
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[bind_group_data(ProbeOutput)]
struct ProbeExtension {
    output: ProbeOutput,
}

impl From<&ProbeExtension> for ProbeOutput {
    fn from(extension: &ProbeExtension) -> Self {
        extension.output
    }
}

impl MaterialExtension for ProbeExtension {
    fn enable_prepass() -> bool {
        WaterMaterial::enable_prepass()
    }

    fn enable_shadows() -> bool {
        WaterMaterial::enable_shadows()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = &mut descriptor.fragment {
            let definition = match key.bind_group_data {
                ProbeOutput::Displacement => Some("VERIFY_WATER_DISPLACEMENT"),
                ProbeOutput::Normal => Some("VERIFY_WATER_NORMAL"),
                ProbeOutput::Color => None,
            };
            if let Some(definition) = definition {
                fragment
                    .shader_defs
                    .push(ShaderDefVal::Bool(definition.into(), true));
            }
        }
        Ok(())
    }
}

type ProbeMaterial = ExtendedMaterial<WaterMaterial, ProbeExtension>;

#[derive(Default)]
enum ProbeState {
    #[default]
    Waiting,
    Reading(usize),
    Complete,
    Failed(String),
}

#[derive(Resource, Default)]
pub(crate) struct WaterShaderVerification {
    state: ProbeState,
}

impl WaterShaderVerification {
    pub(crate) fn ready(&mut self, pipelines_ready: bool) -> Result<bool, String> {
        match &self.state {
            ProbeState::Waiting if pipelines_ready => {
                self.state = ProbeState::Reading(PROBE_COUNT);
                Ok(false)
            }
            ProbeState::Waiting | ProbeState::Reading(_) => Ok(false),
            ProbeState::Complete => Ok(true),
            ProbeState::Failed(cause) => Err(cause.clone()),
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<WaterShaderVerification>()
        .add_plugins(MaterialPlugin::<ProbeMaterial>::default())
        .add_systems(Startup, setup.after(super::super::world::setup_world));
}

fn setup(
    mut commands: Commands,
    source: Query<&MeshMaterial3d<WaterMaterial>, With<NearWaterSurface>>,
    source_materials: Res<Assets<WaterMaterial>>,
    mut materials: ResMut<Assets<ProbeMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut verification: ResMut<WaterShaderVerification>,
) {
    let Some(source) = source
        .single()
        .ok()
        .and_then(|handle| source_materials.get(&handle.0))
    else {
        verification.state =
            ProbeState::Failed("Shader probe requires the production near-water material".into());
        return;
    };
    let camera = Transform::from_xyz(0.0, 30.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z);
    for (case_index, (label, quality, flat, far)) in CASES.into_iter().enumerate() {
        let mut base = source.clone();
        if let Err(cause) = base.apply_profile(quality) {
            verification.state = ProbeState::Failed(cause);
            return;
        }
        base.project_camera(camera.translation, 7.0);
        if far {
            base.geometry_patch = Vec4::ZERO;
        }
        if flat {
            base.geometry_parameters.x = 0.0;
            base.waves_sky = Vec4::ZERO;
        }
        let bounds = base.displacement_bounds;
        for (output_index, output) in ProbeOutput::ALL.into_iter().enumerate() {
            let layers = RenderLayers::layer(2 + case_index * 3 + output_index);
            let mut image = Image::new_target_texture(SIDE, SIDE, TextureFormat::Rgba32Float, None);
            image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            let image = images.add(image);
            let camera_entity = commands
                .spawn((
                    Camera3d::default(),
                    Camera {
                        order: -2,
                        clear_color: ClearColorConfig::Custom(Color::linear_rgba(
                            1000.0, 1000.0, 1000.0, 0.0,
                        )),
                        ..default()
                    },
                    RenderTarget::Image(image.clone().into()),
                    Projection::Orthographic(OrthographicProjection {
                        scaling_mode: ScalingMode::Fixed {
                            width: 16.0,
                            height: 16.0,
                        },
                        ..OrthographicProjection::default_3d()
                    }),
                    Tonemapping::None,
                    Msaa::Off,
                    camera,
                    layers.clone(),
                ))
                .id();
            let mut mesh = Plane3d::default()
                .mesh()
                .size(32.0, 32.0)
                .subdivisions(32)
                .build();
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_UV_1,
                vec![[if far { 1200.0 } else { 0.05 }, 0.0]; mesh.count_vertices()],
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0; 4]; mesh.count_vertices()]);
            let mesh = meshes.add(mesh);
            let material = materials.add(ExtendedMaterial {
                base: base.clone(),
                extension: ProbeExtension { output },
            });
            let surface_entity = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    layers,
                ))
                .id();
            commands.spawn(Readback::texture(image.clone())).observe(move |event: On<ReadbackComplete>, mut commands: Commands, mut verification: ResMut<WaterShaderVerification>, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<ProbeMaterial>>, mut accepted: Local<bool>| {
                if *accepted || !matches!(verification.state, ProbeState::Reading(_)) { return; }
                let result = inspect_pixels(&event.data, output, flat, !far, bounds);
                let Some(result) = result else { return; };
                *accepted = true;
                match result {
                    Ok(()) => {
                        println!("Production water shader probe: {label}/{output:?} passed ({SIDE}x{SIDE}, Rgba32Float)");
                        if let ProbeState::Reading(remaining) = &mut verification.state {
                            *remaining -= 1;
                            if *remaining == 0 { verification.state = ProbeState::Complete; }
                        }
                    }
                    Err(cause) => verification.state = ProbeState::Failed(format!("Production water shader probe {label}/{output:?}: {cause}")),
                }
                commands.entity(event.entity).despawn();
                commands.entity(camera_entity).despawn();
                commands.entity(surface_entity).despawn();
                images.remove(image.id());
                meshes.remove(mesh.id());
                materials.remove(material.id());
            });
        }
    }
}

fn inspect_pixels(
    bytes: &[u8],
    output: ProbeOutput,
    flat: bool,
    displaced: bool,
    bounds: Vec2,
) -> Option<Result<(), String>> {
    if bytes.len() != (SIDE * SIDE * 16) as usize {
        return Some(Err(format!(
            "Unexpected float readback length: {}",
            bytes.len()
        )));
    }
    let pixels: Vec<Vec4> = bytes
        .chunks_exact(16)
        .map(|pixel| {
            Vec4::from_array(std::array::from_fn(|channel| {
                f32::from_le_bytes(
                    pixel[channel * 4..channel * 4 + 4]
                        .try_into()
                        .expect("Four-byte float"),
                )
            }))
        })
        .collect();
    if pixels.iter().all(|pixel| pixel.w == 0.0) {
        return None;
    }
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for &pixel in &pixels {
        if !pixel.is_finite() || (pixel.w - 1.0).abs() > 1.0e-6 {
            return Some(Err(format!("Invalid float shader output: {pixel:?}")));
        }
        let value = pixel.truncate();
        minimum = minimum.min(value);
        maximum = maximum.max(value);
        let valid = match output {
            ProbeOutput::Displacement if !displaced => value.abs().max_element() < 1.0e-6,
            ProbeOutput::Displacement => {
                value.x.abs().max(value.z.abs()) <= bounds.x + 1.0e-4
                    && value.y.abs() <= bounds.y + 1.0e-4
            }
            ProbeOutput::Normal if flat => value.distance(Vec3::Y) < 1.0e-5,
            ProbeOutput::Normal => (value.length() - 1.0).abs() < 1.0e-4 && value.y > 0.0,
            ProbeOutput::Color => value.min_element() >= 0.0 && value.max_element() < 16.0,
        };
        if !valid {
            return Some(Err(format!(
                "Output violates {output:?} contract: {pixel:?}"
            )));
        }
    }
    if output == ProbeOutput::Color && maximum.max_element() < 0.001 {
        return Some(Err("Production fragment returned black water".into()));
    }
    if !flat
        && (output == ProbeOutput::Normal || output == ProbeOutput::Displacement && displaced)
        && (maximum - minimum).max_element() < 1.0e-6
    {
        return Some(Err(format!("Waves have no spatial response in {output:?}")));
    }
    Some(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(pixel: [f32; 4]) -> Vec<u8> {
        (0..SIDE * SIDE)
            .flat_map(|_| pixel.into_iter().flat_map(f32::to_le_bytes))
            .collect()
    }

    #[test]
    fn readback_rejects_nonfinite_black_invalid_normals_and_displacement() {
        for (pixel, output, flat, displaced) in [
            ([f32::NAN, 0.0, 0.0, 1.0], ProbeOutput::Color, false, false),
            ([0.0, 0.0, 0.0, 1.0], ProbeOutput::Color, false, false),
            ([100.0, 100.0, 100.0, 1.0], ProbeOutput::Color, false, false),
            ([0.0, 0.0, 0.0, 1.0], ProbeOutput::Normal, false, false),
            ([0.0, -1.0, 0.0, 1.0], ProbeOutput::Normal, true, false),
            ([2.0, 0.0, 0.0, 1.0], ProbeOutput::Displacement, false, true),
            (
                [0.1, 0.0, 0.0, 1.0],
                ProbeOutput::Displacement,
                false,
                false,
            ),
        ] {
            assert!(
                inspect_pixels(&data(pixel), output, flat, displaced, Vec2::ONE)
                    .unwrap()
                    .is_err()
            );
        }
        assert!(
            inspect_pixels(&[0; 4], ProbeOutput::Color, false, false, Vec2::ONE)
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn clear_target_is_pending_and_valid_flat_outputs_are_accepted() {
        assert!(
            inspect_pixels(
                &data([1000.0, 1000.0, 1000.0, 0.0]),
                ProbeOutput::Color,
                true,
                false,
                Vec2::ONE
            )
            .is_none()
        );
        for (pixel, output) in [
            ([0.0, 0.0, 0.0, 1.0], ProbeOutput::Displacement),
            ([0.0, 1.0, 0.0, 1.0], ProbeOutput::Normal),
            ([0.02, 0.04, 0.05, 1.0], ProbeOutput::Color),
        ] {
            assert_eq!(
                inspect_pixels(&data(pixel), output, true, false, Vec2::ONE),
                Some(Ok(()))
            );
        }
    }
}
