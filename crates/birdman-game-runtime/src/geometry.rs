use crate::assets::ApplicationAssetError;
use std::collections::BTreeMap;

use birdman_game_core::WingBendingFactor;
use birdman_game_format::WaterQuality;
use birdman_game_presentation::camera::{Pose, Quaternion, Vector3, compose_pose};
use birdman_game_presentation::render_pose::{
    FlightRenderPose, RenderTailGeometry, RenderWingDeformation,
};
use birdman_game_scene::airframe_structure::StructuralAttachment;
use birdman_game_scene::lake_water::{LakeWaveProjection, water_quality_profile};
use birdman_game_scene::lake_water_mesh::{PreparedWaterChunk, PreparedWaterGeometry};
use birdman_game_scene::mesh::{area_weighted_normals, write_area_weighted_normals};
use birdman_game_scene::prepared_scene::{PreparedAirframeFrame, registered_scene};
use birdman_game_scene::tail_geometry::TailMaterial;
use birdman_game_scene::wing_geometry::WingMaterial;
use serde::Serialize;

/// Native borrowing view of the same buffer identity copied by the WASM adapter.
pub enum ApplicationBufferSlice<'buffer> {
    /// Byte texels or encoded data.
    U8(&'buffer [u8]),
    /// Native Float32 attribute elements.
    F32(&'buffer [f32]),
    /// Native unsigned triangle indices.
    U32(&'buffer [u32]),
}

impl ApplicationBufferSlice<'_> {
    /// Copies wire bytes in the common little-endian attribute convention.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::U8(values) => values.to_vec(),
            Self::F32(values) => values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
            Self::U32(values) => values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect(),
        }
    }
}

/// Scalar element format of a shared upload buffer.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApplicationBufferType {
    /// Unsigned bytes.
    U8,
    /// Float32 in little-endian wire order.
    F32,
    /// Unsigned 32-bit indices in little-endian wire order.
    U32,
}

/// Complete typed layout for a borrowed or copied GPU upload.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationBufferDescriptor {
    /// Stable identity accepted by the common buffer accessor.
    pub buffer_id: u32,
    /// Scalar GPU element format.
    pub data_type: ApplicationBufferType,
    /// Components per attribute item.
    pub components: u32,
    /// Attribute item count, rather than scalar count.
    pub count: u32,
    /// Contiguous item stride in bytes.
    pub stride_bytes: u32,
}

/// Parent transform selected by shared physical or world placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ApplicationMeshAttachment {
    /// Registered render-world frame.
    World,
    /// Aircraft datum frame.
    Aircraft,
    /// Horizontal-tail mesh in the aircraft frame.
    HorizontalTail,
    /// Vertical-tail mesh in the aircraft frame.
    VerticalTail,
}

/// Engine-independent final material values for a triangle range.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationMeshMaterial {
    /// First triangle index in the shared mesh index buffer.
    pub start: u32,
    /// Number of indices in this range.
    pub count: u32,
    /// Opaque sRGB material colour.
    pub color_srgb: u32,
    /// Material alpha.
    pub opacity: f32,
    /// Direct transparent-pipeline flag.
    pub transparent: bool,
    /// Direct depth-buffer write flag.
    pub depth_write: bool,
    /// Render both triangle orientations.
    pub double_sided: bool,
    /// Apply scene illumination.
    pub lit: bool,
    /// Cast an opaque shadow.
    pub cast_shadow: bool,
    /// Receive scene shadows.
    pub receive_shadow: bool,
    /// Optional shared wing texture identity.
    pub texture_buffer_id: Option<u32>,
}

/// Static topology and final attributes for one scene mesh.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationMeshAsset {
    /// Stable drawing identity.
    pub mesh_id: u32,
    /// Shared diagnostic name.
    pub name: String,
    /// Final transform parent.
    pub attachment: ApplicationMeshAttachment,
    /// Final local transform; positions already use right/up/aft or render-world axes.
    pub local_pose: Pose,
    /// Culling is disabled for articulated or deforming aircraft meshes.
    pub frustum_culled: bool,
    /// Float32 triangle vertices.
    pub position: ApplicationBufferDescriptor,
    /// Final unit normals.
    pub normal: ApplicationBufferDescriptor,
    /// Surface-local UInt32 triangle indices.
    pub index: ApplicationBufferDescriptor,
    /// Optional texture coordinates.
    pub uv: Option<ApplicationBufferDescriptor>,
    /// Optional linear vertex colour.
    pub color: Option<ApplicationBufferDescriptor>,
    /// Contiguous material ranges.
    pub materials: Vec<ApplicationMeshMaterial>,
}

/// Completed static RGBA image including its full linear-light mip chain.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationGeometryTexture {
    /// Concatenated RGBA8 mip levels, largest first.
    pub buffer: ApplicationBufferDescriptor,
    /// Base image texel dimensions.
    pub size: [u32; 2],
    /// Complete mip level count.
    pub mip_level_count: u32,
}

/// Immutable registered meshes and completed wing images.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationGeometryAssets {
    /// Static triangle topology in common GPU attribute form.
    pub meshes: Vec<ApplicationMeshAsset>,
    /// Left and right completed wing textures.
    pub wing_textures: [ApplicationGeometryTexture; 2],
}

/// Completed culling volume in water-local render axes.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationWaterBounds {
    /// Inclusive minimum coordinates.
    pub minimum: [f64; 3],
    /// Inclusive maximum coordinates.
    pub maximum: [f64; 3],
    /// Final bounding-sphere centre.
    pub sphere_center: [f64; 3],
    /// Final bounding-sphere radius.
    pub sphere_radius: f64,
}

/// One complete immutable water mesh and weather-expanded culling volume.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationWaterChunkAsset {
    /// Final render X/0/Z positions.
    pub position: ApplicationBufferDescriptor,
    /// Upward base normals.
    pub normal: ApplicationBufferDescriptor,
    /// Whole-patch texture coordinates.
    pub uv: ApplicationBufferDescriptor,
    /// Shared local spacing consumed by the wave shader.
    pub grid_spacing: ApplicationBufferDescriptor,
    /// Positive-Y triangle indices.
    pub index: ApplicationBufferDescriptor,
    /// Final culling volume including the shared displacement reserve.
    pub bounds: ApplicationWaterBounds,
    /// Whether the engine should apply view-frustum culling.
    pub frustum_culled: bool,
}

/// Complete water geometry selected by the shared quality policy.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationWaterGeometry {
    /// Sixteen near chunks, in row-major order.
    pub near: Vec<ApplicationWaterChunkAsset>,
    /// Distant backing geometry.
    pub far: ApplicationWaterChunkAsset,
    /// Shared horizontal/vertical displacement budgets.
    pub displacement_meters: [f64; 2],
}

/// One replacement attribute pair using retained mutable CPU buffers.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationMeshUpdate {
    /// Static mesh receiving these attributes.
    pub mesh_id: u32,
    /// Current accepted positions.
    pub position: ApplicationBufferDescriptor,
    /// Current completed normals.
    pub normal: ApplicationBufferDescriptor,
}

/// Final local pose for an articulated mesh parent.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationMeshPose {
    /// Shared articulation parent.
    pub attachment: ApplicationMeshAttachment,
    /// Final aircraft-local transform, including the physical pivot.
    pub pose: Pose,
}

/// Mutable attributes and articulation already resolved by the shared owner.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ApplicationGeometryFrame {
    /// Whether registered aircraft geometry is available for this source.
    pub aircraft_visible: bool,
    /// Geometry upload generation, advancing only after a complete accepted update.
    pub revision: u32,
    /// Attribute replacements; empty while the accepted coefficient is unchanged.
    pub updates: Vec<ApplicationMeshUpdate>,
    /// Final horizontal and vertical tail transforms.
    pub part_poses: [ApplicationMeshPose; 2],
}

enum Buffer {
    BorrowedF32(&'static [f32]),
    BorrowedU32(&'static [u32]),
    BorrowedBytes(&'static [u8]),
    F32(Vec<f32>),
    U32(Vec<u32>),
    Wing(usize),
    Structure(usize),
    Normal(usize),
}

#[derive(Clone, Copy)]
enum DynamicSource {
    Wing(usize),
    Structure(usize),
}

struct DynamicMesh {
    source: DynamicSource,
    update: ApplicationMeshUpdate,
    sums: Vec<[f64; 3]>,
    normals: Vec<[f32; 3]>,
    indices: &'static [u32],
}

pub(super) struct GeometryOwner {
    tail_pivots: [Vector3; 2],
    assets: ApplicationGeometryAssets,
    buffers: BTreeMap<u32, Buffer>,
    frame: PreparedAirframeFrame<'static>,
    dynamic: Vec<DynamicMesh>,
    revision: u32,
    published: bool,
    water: BTreeMap<u32, ApplicationWaterGeometry>,
}

type GeometryResult<Value> = Result<Value, ApplicationAssetError>;

impl GeometryOwner {
    pub(super) fn prepare(next_id: &mut u32) -> GeometryResult<Self> {
        let source = registered_scene().map_err(ApplicationAssetError::ScenePreparation)?;
        let frame = source
            .frame(
                WingBendingFactor::try_new(0.0).map_err(ApplicationAssetError::FrameDeformation)?,
            )
            .map_err(|cause| {
                ApplicationAssetError::ScenePreparation(
                    birdman_game_scene::prepared_scene::ScenePreparationError::Structure(cause),
                )
            })?;
        let mut owner = Self {
            tail_pivots: std::array::from_fn(|index| {
                let [x, y, z] = source.tails()[index].origin;
                Vector3 { x, y, z }
            }),
            assets: ApplicationGeometryAssets {
                meshes: Vec::new(),
                wing_textures: std::array::from_fn(|_| ApplicationGeometryTexture {
                    buffer: ApplicationBufferDescriptor {
                        buffer_id: 0,
                        data_type: ApplicationBufferType::U8,
                        components: 4,
                        count: 0,
                        stride_bytes: 4,
                    },
                    size: [0, 0],
                    mip_level_count: 0,
                }),
            },
            buffers: BTreeMap::new(),
            frame,
            dynamic: Vec::new(),
            revision: 0,
            published: false,
            water: BTreeMap::new(),
        };
        for (side, bytes) in source.wing_textures().mipmaps().iter().enumerate() {
            let buffer = owner.insert(
                next_id,
                Buffer::BorrowedBytes(bytes),
                ApplicationBufferType::U8,
                4,
                bytes.len() / 4,
            )?;
            owner.assets.wing_textures[side] = ApplicationGeometryTexture {
                buffer,
                size: source.wing_textures().size(),
                mip_level_count: source.wing_textures().mip_level_count(),
            };
        }
        for (index, surface) in source.wing().surfaces().iter().enumerate() {
            let materials = surface
                .material_ranges()
                .iter()
                .map(|range| {
                    let (color, alpha, texture) = match range.material {
                        WingMaterial::Cover => (
                            0xffffff,
                            1.0,
                            Some(owner.assets.wing_textures[index / 2].buffer.buffer_id),
                        ),
                        WingMaterial::Underside => (0xffffff, 1.0, None),
                        WingMaterial::Film => (0xd5ddd9, 0.22, None),
                    };
                    material(range.start, range.count, color, alpha, true, texture)
                })
                .collect();
            let mesh = owner.mesh(
                next_id,
                format!("wing-{index}"),
                ApplicationMeshAttachment::Aircraft,
                Buffer::BorrowedF32(surface.positions().as_flattened()),
                Buffer::BorrowedU32(surface.indices()),
                surface.indices(),
                materials,
            )?;
            let uv = owner.insert(
                next_id,
                Buffer::BorrowedF32(surface.uvs().as_flattened()),
                ApplicationBufferType::F32,
                2,
                surface.uvs().len(),
            )?;
            owner.assets.meshes[mesh].uv = Some(uv);
            owner.dynamic(next_id, mesh, DynamicSource::Wing(index), surface.indices())?;
        }
        for (index, surface) in source.cockpit().surfaces.iter().enumerate() {
            owner.mesh(
                next_id,
                format!("cockpit-{index}"),
                ApplicationMeshAttachment::Aircraft,
                Buffer::BorrowedF32(surface.positions.as_flattened()),
                Buffer::BorrowedU32(&surface.indices),
                &surface.indices,
                vec![material(
                    0,
                    surface.indices.len(),
                    surface.material.color(),
                    surface.material.opacity(),
                    true,
                    None,
                )],
            )?;
        }
        for (index, tail) in source.tails().iter().enumerate() {
            for kind in TailMaterial::ALL {
                let mut positions = Vec::new();
                let mut indices = Vec::new();
                for patch in tail.patches.iter().filter(|patch| patch.material == kind) {
                    let base = positions.len() as u32;
                    positions.extend(
                        patch
                            .vertices
                            .map(|point| tail_vertex(point, owner.tail_pivots[index])),
                    );
                    indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                }
                if positions.is_empty() {
                    continue;
                }
                let position_buffer = Buffer::F32(positions.into_flattened());
                let index_buffer = Buffer::U32(indices.clone());
                owner.mesh(
                    next_id,
                    format!("tail-{index}-{kind:?}"),
                    tail_attachment(index),
                    position_buffer,
                    index_buffer,
                    &indices,
                    vec![material(
                        0,
                        indices.len(),
                        kind.color(),
                        kind.opacity(),
                        true,
                        None,
                    )],
                )?;
            }
        }
        for (index, mesh) in source.structure().meshes().iter().enumerate() {
            let (attachment, position_buffer) = match mesh.attachment() {
                StructuralAttachment::HorizontalTail => (
                    ApplicationMeshAttachment::HorizontalTail,
                    Buffer::F32(
                        mesh.positions()
                            .iter()
                            .flat_map(|point| tail_vertex(*point, owner.tail_pivots[0]))
                            .collect::<Vec<_>>(),
                    ),
                ),
                StructuralAttachment::VerticalTail => (
                    ApplicationMeshAttachment::VerticalTail,
                    Buffer::F32(
                        mesh.positions()
                            .iter()
                            .flat_map(|point| tail_vertex(*point, owner.tail_pivots[1]))
                            .collect::<Vec<_>>(),
                    ),
                ),
                _ => (
                    ApplicationMeshAttachment::Aircraft,
                    Buffer::BorrowedF32(mesh.positions().as_flattened()),
                ),
            };
            let mesh_index = owner.mesh(
                next_id,
                format!("structure-{index}"),
                attachment,
                position_buffer,
                Buffer::BorrowedU32(mesh.indices()),
                mesh.indices(),
                vec![material(
                    0,
                    mesh.indices().len(),
                    mesh.color(),
                    1.0,
                    true,
                    None,
                )],
            )?;
            if mesh.attachment() == StructuralAttachment::MainWing {
                owner.dynamic(
                    next_id,
                    mesh_index,
                    DynamicSource::Structure(index),
                    mesh.indices(),
                )?;
            }
        }
        for mesh in source.terrain() {
            let positions: Vec<_> = mesh.positions_ned.iter().copied().map(ned).collect();
            let normals: Vec<_> = mesh.normals_ned.iter().copied().map(ned).collect();
            let indices = &mesh.indices;
            let mesh_index = owner.mesh_ready(
                next_id,
                mesh.name.clone(),
                ApplicationMeshAttachment::World,
                Buffer::F32(positions.as_flattened().to_vec()),
                positions.len(),
                Buffer::BorrowedU32(indices),
                indices.len(),
                vec![material(0, indices.len(), 0xffffff, 1.0, true, None)],
                normals,
            )?;
            let color = owner.insert(
                next_id,
                Buffer::BorrowedF32(mesh.colors_linear.as_flattened()),
                ApplicationBufferType::F32,
                3,
                mesh.colors_linear.len(),
            )?;
            owner.assets.meshes[mesh_index].color = Some(color);
            owner.assets.meshes[mesh_index].materials[0].cast_shadow = false;
            owner.assets.meshes[mesh_index].materials[0].receive_shadow = false;
        }
        for mesh in source.shore_structures() {
            if mesh.positions_ned.is_empty() {
                continue;
            }
            let positions: Vec<_> = mesh.positions_ned.iter().copied().map(ned).collect();
            owner.mesh(
                next_id,
                mesh.kind.name().to_owned(),
                ApplicationMeshAttachment::World,
                Buffer::F32(positions.into_flattened()),
                Buffer::BorrowedU32(&mesh.indices),
                &mesh.indices,
                vec![material(
                    0,
                    mesh.indices.len(),
                    mesh.kind.color_srgb(),
                    1.0,
                    true,
                    None,
                )],
            )?;
        }
        for (name, mesh, double_sided) in [
            ("launch-deck", &source.launch_platform().deck, true),
            ("launch-supports", &source.launch_platform().supports, false),
        ] {
            let positions: Vec<_> = mesh.positions_ned.iter().copied().map(ned).collect();
            let mesh_index = owner.mesh(
                next_id,
                name.to_owned(),
                ApplicationMeshAttachment::World,
                Buffer::F32(positions.into_flattened()),
                Buffer::BorrowedU32(&mesh.indices),
                &mesh.indices,
                vec![material(
                    0,
                    mesh.indices.len(),
                    mesh.color_srgb,
                    1.0,
                    false,
                    None,
                )],
            )?;
            owner.assets.meshes[mesh_index].materials[0].double_sided = double_sided;
        }
        Ok(owner)
    }

    fn insert(
        &mut self,
        next_id: &mut u32,
        buffer: Buffer,
        data_type: ApplicationBufferType,
        components: u32,
        count: usize,
    ) -> GeometryResult<ApplicationBufferDescriptor> {
        let count = u32::try_from(count).map_err(|_| {
            ApplicationAssetError::Geometry("Drawing buffer count exhausted".to_owned())
        })?;
        *next_id = next_id
            .checked_add(1)
            .ok_or(ApplicationAssetError::IdentifiersExhausted)?;
        self.buffers.insert(*next_id, buffer);
        Ok(ApplicationBufferDescriptor {
            buffer_id: *next_id,
            data_type,
            components,
            count,
            stride_bytes: components
                * if matches!(data_type, ApplicationBufferType::U8) {
                    1
                } else {
                    4
                },
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn mesh(
        &mut self,
        next_id: &mut u32,
        name: String,
        attachment: ApplicationMeshAttachment,
        position_buffer: Buffer,
        index_buffer: Buffer,
        indices: &[u32],
        materials: Vec<ApplicationMeshMaterial>,
    ) -> GeometryResult<usize> {
        let values: &[f32] = match &position_buffer {
            Buffer::BorrowedF32(values) => values,
            Buffer::F32(values) => values,
            _ => {
                return Err(ApplicationAssetError::Geometry(format!(
                    "{name}: Float32 positions required"
                )));
            }
        };
        let (positions, remainder) = values.as_chunks::<3>();
        if !remainder.is_empty() {
            return Err(ApplicationAssetError::Geometry(format!(
                "{name}: incomplete position"
            )));
        }
        let count = positions.len();
        let normals = area_weighted_normals(positions, indices).map_err(|cause| {
            ApplicationAssetError::MeshProjection {
                mesh: Some(name.clone()),
                cause,
            }
        })?;
        self.mesh_ready(
            next_id,
            name,
            attachment,
            position_buffer,
            count,
            index_buffer,
            indices.len(),
            materials,
            normals,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn mesh_ready(
        &mut self,
        next_id: &mut u32,
        name: String,
        attachment: ApplicationMeshAttachment,
        position_buffer: Buffer,
        position_count: usize,
        index_buffer: Buffer,
        index_count: usize,
        materials: Vec<ApplicationMeshMaterial>,
        normals: Vec<[f32; 3]>,
    ) -> GeometryResult<usize> {
        let position = self.insert(
            next_id,
            position_buffer,
            ApplicationBufferType::F32,
            3,
            position_count,
        )?;
        let normal = self.insert(
            next_id,
            Buffer::F32(normals.as_flattened().to_vec()),
            ApplicationBufferType::F32,
            3,
            normals.len(),
        )?;
        let index = self.insert(
            next_id,
            index_buffer,
            ApplicationBufferType::U32,
            1,
            index_count,
        )?;
        let mesh_index = self.assets.meshes.len();
        self.assets.meshes.push(ApplicationMeshAsset {
            frustum_culled: attachment == ApplicationMeshAttachment::World,
            mesh_id: mesh_index as u32 + 1,
            name,
            attachment,
            local_pose: Pose::IDENTITY,
            position,
            normal,
            index,
            uv: None,
            color: None,
            materials,
        });
        Ok(mesh_index)
    }

    fn dynamic(
        &mut self,
        next_id: &mut u32,
        mesh_index: usize,
        source: DynamicSource,
        indices: &'static [u32],
    ) -> GeometryResult<()> {
        let binding = self.dynamic.len();
        let count = self.assets.meshes[mesh_index].position.count as usize;
        let buffer = match source {
            DynamicSource::Wing(index) => Buffer::Wing(index),
            DynamicSource::Structure(index) => Buffer::Structure(index),
        };
        let position = self.insert(next_id, buffer, ApplicationBufferType::F32, 3, count)?;
        let normal = self.insert(
            next_id,
            Buffer::Normal(binding),
            ApplicationBufferType::F32,
            3,
            count,
        )?;
        self.dynamic.push(DynamicMesh {
            source,
            update: ApplicationMeshUpdate {
                mesh_id: self.assets.meshes[mesh_index].mesh_id,
                position,
                normal,
            },
            sums: vec![[0.0; 3]; count],
            normals: vec![[0.0; 3]; count],
            indices,
        });
        Ok(())
    }

    pub(super) fn assets(&self) -> &ApplicationGeometryAssets {
        &self.assets
    }

    pub(super) fn rebind(&mut self) {
        self.published = false;
    }

    pub(super) fn water(
        &mut self,
        next_id: &mut u32,
        quality: WaterQuality,
        projection: &LakeWaveProjection,
    ) -> GeometryResult<ApplicationWaterGeometry> {
        let prepared = PreparedWaterGeometry::try_new(quality, projection)
            .map_err(ApplicationAssetError::Water)?;
        let key = water_quality_profile(quality).mesh_segments();
        if !self.water.contains_key(&key) {
            let near = prepared
                .near
                .iter()
                .map(|chunk| self.water_chunk(next_id, chunk))
                .collect::<Result<_, _>>()?;
            let far = self.water_chunk(next_id, &prepared.far)?;
            self.water.insert(
                key,
                ApplicationWaterGeometry {
                    near,
                    far,
                    displacement_meters: prepared.displacement_meters,
                },
            );
        }
        let mut output = self.water[&key].clone();
        for (chunk, source) in output.near.iter_mut().zip(&prepared.near) {
            chunk.bounds = water_bounds(source);
        }
        output.far.bounds = water_bounds(&prepared.far);
        output.displacement_meters = prepared.displacement_meters;
        Ok(output)
    }

    fn water_chunk(
        &mut self,
        next_id: &mut u32,
        chunk: &PreparedWaterChunk,
    ) -> GeometryResult<ApplicationWaterChunkAsset> {
        let mesh = chunk.mesh;
        Ok(ApplicationWaterChunkAsset {
            position: self.insert(
                next_id,
                Buffer::BorrowedF32(mesh.positions.as_flattened()),
                ApplicationBufferType::F32,
                3,
                mesh.positions.len(),
            )?,
            normal: self.insert(
                next_id,
                Buffer::BorrowedF32(mesh.normals.as_flattened()),
                ApplicationBufferType::F32,
                3,
                mesh.normals.len(),
            )?,
            uv: self.insert(
                next_id,
                Buffer::BorrowedF32(mesh.uv.as_flattened()),
                ApplicationBufferType::F32,
                2,
                mesh.uv.len(),
            )?,
            grid_spacing: self.insert(
                next_id,
                Buffer::BorrowedF32(&mesh.grid_spacing),
                ApplicationBufferType::F32,
                1,
                mesh.grid_spacing.len(),
            )?,
            index: self.insert(
                next_id,
                Buffer::BorrowedU32(&mesh.indices),
                ApplicationBufferType::U32,
                1,
                mesh.indices.len(),
            )?,
            bounds: water_bounds(chunk),
            frustum_culled: chunk.frustum_culled,
        })
    }

    pub(super) fn project(
        &mut self,
        pose: Option<FlightRenderPose>,
    ) -> GeometryResult<ApplicationGeometryFrame> {
        let factor = match pose.map(|pose| pose.wing_deformation) {
            None => Some(0.0),
            Some(RenderWingDeformation::Accepted { factor }) => Some(factor),
            _ => None,
        };
        let available = factor.is_some()
            && pose.is_none_or(|pose| {
                matches!(pose.tail_geometry, RenderTailGeometry::Available { .. })
            });
        let mut updates = Vec::new();
        if let Some(factor) = factor
            && available
        {
            let changed = self
                .frame
                .update(
                    WingBendingFactor::try_new(factor)
                        .map_err(ApplicationAssetError::FrameDeformation)?,
                )
                .map_err(|cause| {
                    ApplicationAssetError::ScenePreparation(
                        birdman_game_scene::prepared_scene::ScenePreparationError::Structure(cause),
                    )
                })?;
            if changed || !self.published {
                self.published = false;
                for mesh in &mut self.dynamic {
                    let positions = match mesh.source {
                        DynamicSource::Wing(index) => &self
                            .frame
                            .wing()
                            .ok_or_else(|| {
                                ApplicationAssetError::Geometry("Unaccepted wing frame".to_owned())
                            })?
                            .surface_positions()[index],
                        DynamicSource::Structure(index) => {
                            self.frame.structure_positions(index).ok_or_else(|| {
                                ApplicationAssetError::Geometry(
                                    "Unaccepted structural frame".to_owned(),
                                )
                            })?
                        }
                    };
                    write_area_weighted_normals(
                        positions,
                        mesh.indices,
                        &mut mesh.sums,
                        &mut mesh.normals,
                    )
                    .map_err(|cause| ApplicationAssetError::MeshProjection { mesh: None, cause })?;
                    updates.push(mesh.update.clone());
                }
                self.revision = self.revision.checked_add(1).ok_or_else(|| {
                    ApplicationAssetError::Geometry(
                        "Drawing frame generations exhausted".to_owned(),
                    )
                })?;
                self.published = true;
            }
        }
        let incidence = pose.map(|pose| pose.controls.physical_incidence);
        let part_pose = |index: usize| -> GeometryResult<ApplicationMeshPose> {
            let angle = incidence.map_or(0.0, |angles| {
                if index == 0 {
                    angles.horizontal_tail_radians
                } else {
                    -angles.vertical_tail_radians
                }
            }) / 2.0;
            let pivot = self.tail_pivots[index];
            let pose = compose_pose(
                Pose {
                    position: pivot,
                    orientation: Quaternion {
                        w: angle.cos(),
                        x: if index == 0 { angle.sin() } else { 0.0 },
                        y: if index == 1 { angle.sin() } else { 0.0 },
                        z: 0.0,
                    },
                },
                Pose {
                    position: Vector3 {
                        x: -pivot.x,
                        y: -pivot.y,
                        z: -pivot.z,
                    },
                    ..Pose::IDENTITY
                },
            )
            .map_err(ApplicationAssetError::TailArticulation)?;
            Ok(ApplicationMeshPose {
                attachment: tail_attachment(index),
                pose,
            })
        };
        let part_poses = [part_pose(0)?, part_pose(1)?];
        Ok(ApplicationGeometryFrame {
            aircraft_visible: available,
            revision: self.revision,
            updates,
            part_poses,
        })
    }

    pub(super) fn buffer(&self, buffer_id: u32) -> Option<ApplicationBufferSlice<'_>> {
        Some(match self.buffers.get(&buffer_id)? {
            Buffer::BorrowedBytes(bytes) => ApplicationBufferSlice::U8(bytes),
            Buffer::BorrowedF32(values) => ApplicationBufferSlice::F32(values),
            Buffer::BorrowedU32(values) => ApplicationBufferSlice::U32(values),
            Buffer::F32(values) => ApplicationBufferSlice::F32(values),
            Buffer::U32(values) => ApplicationBufferSlice::U32(values),
            Buffer::Wing(index) => ApplicationBufferSlice::F32(
                self.frame.wing()?.surface_positions()[*index].as_flattened(),
            ),
            Buffer::Structure(index) => {
                ApplicationBufferSlice::F32(self.frame.structure_positions(*index)?.as_flattened())
            }
            Buffer::Normal(index) => {
                ApplicationBufferSlice::F32(self.dynamic[*index].normals.as_flattened())
            }
        })
    }
}

fn material(
    start: usize,
    count: usize,
    color_srgb: u32,
    opacity: f32,
    lit: bool,
    texture_buffer_id: Option<u32>,
) -> ApplicationMeshMaterial {
    ApplicationMeshMaterial {
        transparent: opacity < 1.0,
        depth_write: opacity >= 1.0,
        start: start as u32,
        count: count as u32,
        color_srgb,
        opacity,
        double_sided: true,
        lit,
        cast_shadow: opacity >= 1.0,
        receive_shadow: lit,
        texture_buffer_id,
    }
}

fn tail_attachment(index: usize) -> ApplicationMeshAttachment {
    if index == 0 {
        ApplicationMeshAttachment::HorizontalTail
    } else {
        ApplicationMeshAttachment::VerticalTail
    }
}

fn tail_vertex(point: [f32; 3], pivot: Vector3) -> [f32; 3] {
    [
        (f64::from(point[0]) + pivot.x) as f32,
        (f64::from(point[1]) + pivot.y) as f32,
        (f64::from(point[2]) + pivot.z) as f32,
    ]
}

fn ned([north, east, down]: [f32; 3]) -> [f32; 3] {
    [east, -down, -north]
}

fn water_bounds(chunk: &PreparedWaterChunk) -> ApplicationWaterBounds {
    ApplicationWaterBounds {
        minimum: chunk.bounds.minimum,
        maximum: chunk.bounds.maximum,
        sphere_center: chunk.bounds.sphere_center,
        sphere_radius: chunk.bounds.sphere_radius,
    }
}
