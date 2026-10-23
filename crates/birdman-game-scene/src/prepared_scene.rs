//! Immutable registered scene assets and caller-owned deformation buffers.

use std::{fmt, sync::OnceLock};

use birdman_game_app::LaunchVenueError;
use birdman_game_content::{AircraftDefinitionError, ShapeError};
use birdman_game_core::WingBendingFactor;

use crate::{
    airframe_structure::{AirframeStructure, PreparedStructure, StructuralFrame},
    cockpit_geometry::CockpitGeometry,
    mesh::{MeshProjectionError, area_weighted_normals},
    tail_geometry::TailGeometry,
    venue_structures::{ShoreStructureMesh, VenueStructureError, registered_structures},
    venue_terrain::{VenueTerrainMesh, registered_terrain},
    wing_geometry::{WingFrame, WingGeometry},
    wing_texture::{self, WingTextureError},
};

static SCENE: OnceLock<Result<PreparedScene, ScenePreparationError>> = OnceLock::new();

/// Original CPU asset-generation failures, before any GPU resources are created.
#[derive(Clone, Debug)]
pub enum ScenePreparationError {
    /// The registered physical aircraft failed admission.
    Aircraft(AircraftDefinitionError),
    /// Structural positions cannot be represented by the shared mesh format.
    Structure(ShapeError),
    /// A registered tail cannot form its static skin.
    Tail(&'static str),
    /// Registered terrain generation failed.
    Terrain(String),
    /// Registered shoreline structure generation failed.
    VenueStructures(VenueStructureError),
    /// The registered launch platform cannot form its drawing meshes.
    LaunchPlatform(LaunchVenueError),
    /// Final mesh attributes cannot be represented by a valid shared projection.
    Mesh(MeshProjectionError),
    /// The bundled decal could not be decoded as a PNG.
    Decal(String),
    /// Registered texture composition or linear-light mip generation failed.
    WingTexture(WingTextureError),
}

impl fmt::Display for ScenePreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scene preparation: {self:?}")
    }
}

impl std::error::Error for ScenePreparationError {}

/// One static launch-platform mesh in world-fixed north/east/down metres.
pub struct LaunchPlatformMesh {
    /// Positions relative to the registered front lip and water datum.
    pub positions_ned: Vec<[f32; 3]>,
    /// Completed area-weighted unit normals in the same NED frame.
    pub normals_ned: Vec<[f32; 3]>,
    /// Outward triangle indices into the complete position and normal buffers.
    pub indices: Vec<u32>,
    /// Shared unlit sRGB 0xRRGGBB colour.
    pub color_srgb: u32,
}

/// Sloped deck and a single batch of four capped eight-sided supports.
pub struct LaunchPlatformGeometry {
    /// Double-sided launch deck using the registered width, length and slope.
    pub deck: LaunchPlatformMesh,
    /// Opaque supports extending from the water datum to each deck corner.
    pub supports: LaunchPlatformMesh,
}

impl LaunchPlatformGeometry {
    fn prepare() -> Result<Self, ScenePreparationError> {
        let platform = birdman_game_app::launch_venue()
            .map_err(ScenePreparationError::LaunchPlatform)?
            .platform;
        let [forward_north, forward_east] = platform.horizontal_direction_ned();
        let right = [-forward_east, forward_north];
        let rear_height = platform.front_lip_above_water_meters
            + platform.length_meters * platform.downward_slope_degrees.to_radians().tan();
        let corners = [
            (0.0, -1.0, platform.front_lip_above_water_meters),
            (0.0, 1.0, platform.front_lip_above_water_meters),
            (-platform.length_meters, -1.0, rear_height),
            (-platform.length_meters, 1.0, rear_height),
        ];
        let mut deck = LaunchPlatformMesh {
            positions_ned: Vec::with_capacity(4),
            normals_ned: Vec::new(),
            indices: vec![0, 2, 1, 1, 2, 3],
            color_srgb: 0x777b72,
        };
        let mut supports = LaunchPlatformMesh {
            positions_ned: Vec::with_capacity(136),
            normals_ned: Vec::new(),
            indices: Vec::with_capacity(384),
            color_srgb: 0x46514c,
        };
        for (along, across, height) in corners {
            let north = along * forward_north + across * platform.width_meters / 2.0 * right[0];
            let east = along * forward_east + across * platform.width_meters / 2.0 * right[1];
            deck.positions_ned
                .push([north as f32, east as f32, -height as f32]);
            let base = supports.positions_ned.len() as u32;
            for (radius, down) in [(0.12, -height), (0.18, 0.0)] {
                for index in 0..8 {
                    let angle = f64::from(index) * core::f64::consts::TAU / 8.0;
                    supports.positions_ned.push([
                        (north - radius * angle.cos()) as f32,
                        (east + radius * angle.sin()) as f32,
                        down as f32,
                    ]);
                }
            }
            for index in 0..8_u32 {
                let next = (index + 1) % 8;
                supports.indices.extend([
                    base + index,
                    base + index + 8,
                    base + next,
                    base + index + 8,
                    base + next + 8,
                    base + next,
                ]);
            }
            for (radius, down, top) in [(0.12, -height, true), (0.18, 0.0, false)] {
                let center = supports.positions_ned.len() as u32;
                supports
                    .positions_ned
                    .push([north as f32, east as f32, down as f32]);
                for index in 0..8 {
                    let angle = f64::from(index) * core::f64::consts::TAU / 8.0;
                    supports.positions_ned.push([
                        (north - radius * angle.cos()) as f32,
                        (east + radius * angle.sin()) as f32,
                        down as f32,
                    ]);
                }
                for index in 0..8_u32 {
                    let first = center + 1 + index;
                    let next = center + 1 + (index + 1) % 8;
                    supports.indices.extend(if top {
                        [center, first, next]
                    } else {
                        [center, next, first]
                    });
                }
            }
        }
        if deck
            .positions_ned
            .iter()
            .chain(&supports.positions_ned)
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(ScenePreparationError::LaunchPlatform(
                LaunchVenueError::InvalidGeometry,
            ));
        }
        deck.normals_ned = area_weighted_normals(&deck.positions_ned, &deck.indices)
            .map_err(ScenePreparationError::Mesh)?;
        supports.normals_ned = area_weighted_normals(&supports.positions_ned, &supports.indices)
            .map_err(ScenePreparationError::Mesh)?;
        Ok(Self { deck, supports })
    }
}

/// Complete sRGB wing images, including the registered decal and every mip level.
pub struct PreparedWingTextures {
    mipmaps: [Vec<u8>; 2],
}

impl PreparedWingTextures {
    /// Returns the base-level width and height for each wing side.
    pub const fn size(&self) -> [u32; 2] {
        [wing_texture::WIDTH, wing_texture::HEIGHT]
    }

    /// Returns the complete mip count, including the base level.
    pub const fn mip_level_count(&self) -> u32 {
        wing_texture::WIDTH.ilog2() + 1
    }

    /// Borrows left then right RGBA8 mip chains, with the base level first.
    pub const fn mipmaps(&self) -> &[Vec<u8>; 2] {
        &self.mipmaps
    }

    fn prepare() -> Result<Self, ScenePreparationError> {
        let decal = image::load_from_memory_with_format(
            include_bytes!("../../../assets/indigo-resist-decal.png"),
            image::ImageFormat::Png,
        )
        .map_err(|cause| ScenePreparationError::Decal(cause.to_string()))?
        .into_rgba8();
        let alpha: Vec<_> = decal.pixels().map(|pixel| pixel[3]).collect();
        let mut pixels = wing_texture::indigo_wing_textures();
        wing_texture::apply_indigo_wing_decal(&mut pixels, decal.width(), decal.height(), &alpha)
            .map_err(ScenePreparationError::WingTexture)?;
        let left = wing_texture::indigo_wing_mipmaps(&pixels[..wing_texture::SIDE_BYTES])
            .map_err(ScenePreparationError::WingTexture)?;
        let right = wing_texture::indigo_wing_mipmaps(&pixels[wing_texture::SIDE_BYTES..])
            .map_err(ScenePreparationError::WingTexture)?;
        Ok(Self {
            mipmaps: [left, right],
        })
    }
}

/// One immutable CPU scene; terrain and shoreline meshes borrow their registered owners.
pub struct PreparedScene {
    wing: WingGeometry,
    cockpit: CockpitGeometry,
    tails: [TailGeometry; 2],
    structure: PreparedStructure<'static>,
    terrain: &'static [VenueTerrainMesh],
    shore_structures: &'static [ShoreStructureMesh; 3],
    wing_textures: PreparedWingTextures,
    launch_platform: LaunchPlatformGeometry,
}

/// Prepares the registered scene once and borrows the same assets on subsequent calls.
pub fn registered_scene() -> Result<&'static PreparedScene, ScenePreparationError> {
    SCENE
        .get_or_init(PreparedScene::prepare)
        .as_ref()
        .map_err(Clone::clone)
}

impl PreparedScene {
    fn prepare() -> Result<Self, ScenePreparationError> {
        let definition =
            birdman_game_app::registered_aircraft().map_err(ScenePreparationError::Aircraft)?;
        let mut structure =
            AirframeStructure::new(definition).map_err(ScenePreparationError::Structure)?;
        structure
            .update_with_factor(WingBendingFactor::try_new(0.0).unwrap())
            .map_err(ScenePreparationError::Structure)?;
        Ok(Self {
            wing: WingGeometry::new(),
            cockpit: CockpitGeometry::new(),
            tails: TailGeometry::registered().map_err(ScenePreparationError::Tail)?,
            structure: structure.into_prepared(),
            terrain: registered_terrain().map_err(ScenePreparationError::Terrain)?,
            shore_structures: registered_structures()
                .map_err(ScenePreparationError::VenueStructures)?,
            wing_textures: PreparedWingTextures::prepare()?,
            launch_platform: LaunchPlatformGeometry::prepare()?,
        })
    }

    /// Borrows neutral main-wing topology, material ranges and UVs.
    pub const fn wing(&self) -> &WingGeometry {
        &self.wing
    }

    /// Borrows the immutable canopy and fairing surfaces.
    pub const fn cockpit(&self) -> &CockpitGeometry {
        &self.cockpit
    }

    /// Borrows horizontal then vertical neutral tail skins and physical pivots.
    pub const fn tails(&self) -> &[TailGeometry; 2] {
        &self.tails
    }

    /// Borrows neutral structural batches, with shared physical topology and materials.
    pub const fn structure(&self) -> &PreparedStructure<'static> {
        &self.structure
    }

    /// Borrows world-fixed NED terrain positions, normals and linear vertex colours.
    pub const fn terrain(&self) -> &'static [VenueTerrainMesh] {
        self.terrain
    }

    /// Borrows the registered pier, quay and breakwater batches.
    pub const fn shore_structures(&self) -> &'static [ShoreStructureMesh; 3] {
        self.shore_structures
    }

    /// Borrows completed wing images for direct upload by any renderer.
    pub const fn wing_textures(&self) -> &PreparedWingTextures {
        &self.wing_textures
    }

    /// Borrows the launch deck and support geometry derived from the registered venue.
    pub const fn launch_platform(&self) -> &LaunchPlatformGeometry {
        &self.launch_platform
    }

    /// Allocates only deformation buffers and adopts an explicit recorded bending factor.
    pub fn frame(
        &self,
        factor: WingBendingFactor,
    ) -> Result<PreparedAirframeFrame<'_>, ShapeError> {
        let mut frame = PreparedAirframeFrame {
            geometry: &self.wing,
            wing: self.wing.frame(),
            structure: self.structure.frame(),
            factor: None,
        };
        frame.update(factor)?;
        Ok(frame)
    }
}

/// Reusable aircraft positions; static topology, rigid members and tail skins remain borrowed.
pub struct PreparedAirframeFrame<'scene> {
    geometry: &'scene WingGeometry,
    wing: WingFrame,
    structure: StructuralFrame<'scene, 'static>,
    factor: Option<WingBendingFactor>,
}

impl PreparedAirframeFrame<'_> {
    /// Updates both wing skin and members; failed projections remain unpublished until recovery.
    pub fn update(&mut self, factor: WingBendingFactor) -> Result<bool, ShapeError> {
        if self.factor == Some(factor) {
            return Ok(false);
        }
        self.factor = None;
        self.geometry
            .update_frame_with_factor(&mut self.wing, factor)?;
        self.structure.update(factor)?;
        self.factor = Some(factor);
        Ok(true)
    }

    /// Returns the factor only after the complete skin and structural projection succeeded.
    pub const fn bending_factor(&self) -> Option<WingBendingFactor> {
        self.factor
    }

    /// Borrows the valid skin buffers in static main-wing surface order.
    pub fn wing(&self) -> Option<&WingFrame> {
        self.factor.map(|_| &self.wing)
    }

    /// Borrows one valid structural batch; rigid and tail batches use the immutable scene buffer.
    pub fn structure_positions(&self, mesh: usize) -> Option<&[[f32; 3]]> {
        self.factor?;
        self.structure.positions(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::airframe_structure::StructuralAttachment;

    #[test]
    fn registered_bundle_reuses_immutable_meshes_and_completed_image_chains() {
        let scene = registered_scene().unwrap();
        assert!(core::ptr::eq(scene, registered_scene().unwrap()));
        assert!(core::ptr::eq(
            scene.terrain(),
            registered_terrain().unwrap()
        ));
        assert!(core::ptr::eq(
            scene.shore_structures(),
            registered_structures().unwrap()
        ));
        for surface in scene.wing().surfaces() {
            assert!(
                surface
                    .indices()
                    .iter()
                    .all(|index| (*index as usize) < surface.positions().len())
            );
            assert_eq!(surface.positions().len(), surface.uvs().len());
        }
        assert_eq!(scene.structure().meshes().len(), 11);
        assert!(!scene.cockpit().surfaces[0].indices.is_empty());
        assert!(scene.tails().iter().all(|tail| !tail.patches.is_empty()));
        let platform = birdman_game_app::launch_venue().unwrap().platform;
        let deck = &scene.launch_platform().deck;
        assert_eq!(deck.positions_ned.len(), 4);
        assert_eq!(deck.indices, [0, 2, 1, 1, 2, 3]);
        let points = &deck.positions_ned;
        let distance = |first: [f32; 3], second: [f32; 3]| {
            f64::from(first[0] - second[0]).hypot(f64::from(first[1] - second[1]))
        };
        assert!((distance(points[0], points[1]) - platform.width_meters).abs() < 1e-5);
        assert!((distance(points[0], points[2]) - platform.length_meters).abs() < 1e-5);
        assert_eq!(points[0][2], -platform.front_lip_above_water_meters as f32);
        let rear_height = platform.front_lip_above_water_meters
            + platform.length_meters * platform.downward_slope_degrees.to_radians().tan();
        assert_eq!(points[2][2], -rear_height as f32);
        let slope = platform.downward_slope_degrees.to_radians();
        let [north, east] = platform.horizontal_direction_ned();
        let expected_normal = [north * slope.sin(), east * slope.sin(), -slope.cos()];
        for normal in &deck.normals_ned {
            for (value, expected) in normal.iter().zip(expected_normal) {
                assert!((f64::from(*value) - expected).abs() < 1e-6);
            }
        }
        let supports = &scene.launch_platform().supports;
        assert_eq!(supports.positions_ned.len(), 136);
        assert_eq!(supports.indices.len(), 384);
        assert!(
            supports
                .indices
                .iter()
                .all(|index| (*index as usize) < supports.positions_ned.len())
        );
        for mesh in [deck, supports] {
            assert_eq!(mesh.positions_ned.len(), mesh.normals_ned.len());
            assert!(mesh.normals_ned.iter().all(|normal| {
                (f64::from(normal[0])
                    .hypot(f64::from(normal[1]))
                    .hypot(f64::from(normal[2]))
                    - 1.0)
                    .abs()
                    < 1e-6
            }));
        }
        let textures = scene.wing_textures();
        let [width, height] = textures.size();
        let bytes: usize = (0..textures.mip_level_count())
            .map(|level| ((width >> level).max(1) * (height >> level).max(1) * 4) as usize)
            .sum();
        for chain in textures.mipmaps() {
            assert_eq!(chain.len(), bytes);
            assert!(chain.chunks_exact(4).all(|pixel| pixel[3] == 255));
        }
        assert_ne!(textures.mipmaps()[0], textures.mipmaps()[1]);
    }

    #[test]
    fn deformation_borrows_static_topology_and_recovers_without_replacing_upload_buffers() {
        use crate::mesh::{MeshProjectionError, write_area_weighted_normals};
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mut sums = [[0.0; 3]; 3];
        let mut normals = [[0.0; 3]; 3];
        write_area_weighted_normals(&positions, &[0, 1, 2], &mut sums, &mut normals).unwrap();
        assert_eq!(normals, [[0.0, 0.0, 1.0]; 3]);
        assert_eq!(
            write_area_weighted_normals(&positions, &[0, 1, 3], &mut sums, &mut normals),
            Err(MeshProjectionError::InvalidTopology)
        );
        assert_eq!(
            write_area_weighted_normals(&positions, &[0, 1], &mut sums, &mut normals),
            Err(MeshProjectionError::InvalidTopology)
        );
        assert_eq!(
            write_area_weighted_normals(&positions, &[0, 1, 2], &mut sums[..2], &mut normals),
            Err(MeshProjectionError::InvalidBuffers)
        );
        assert_eq!(
            area_weighted_normals(&[[0.0; 3]; 3], &[0, 1, 2]),
            Err(MeshProjectionError::DegenerateNormal)
        );
        let scene = registered_scene().unwrap();
        let mut frame = scene
            .frame(WingBendingFactor::try_new(0.0).unwrap())
            .unwrap();
        let wing_pointers = frame
            .wing()
            .unwrap()
            .surface_positions()
            .each_ref()
            .map(|positions| positions.as_ptr());
        let member_pointers: Vec<_> = (0..scene.structure().meshes().len())
            .map(|index| frame.structure_positions(index).unwrap().as_ptr())
            .collect();
        let mut expected =
            AirframeStructure::new(birdman_game_app::registered_aircraft().unwrap()).unwrap();
        for value in [0.3, 1.6, 0.3] {
            let factor = WingBendingFactor::try_new(value).unwrap();
            assert!(frame.update(factor).unwrap());
            assert!(!frame.update(factor).unwrap());
            assert_eq!(frame.bending_factor(), Some(factor));
            expected.update_with_factor(factor).unwrap();
            for (index, mesh) in scene.structure().meshes().iter().enumerate() {
                let positions = frame.structure_positions(index).unwrap();
                assert_eq!(positions, expected.meshes()[index].positions());
                assert_eq!(positions.as_ptr(), member_pointers[index]);
                if mesh.attachment() != StructuralAttachment::MainWing {
                    assert!(core::ptr::eq(positions, mesh.positions()));
                }
            }
            assert_eq!(
                frame
                    .wing()
                    .unwrap()
                    .surface_positions()
                    .each_ref()
                    .map(|positions| positions.as_ptr()),
                wing_pointers
            );
        }
        assert!(
            frame
                .update(WingBendingFactor::try_new(f64::MAX).unwrap())
                .is_err()
        );
        assert_eq!(frame.bending_factor(), None);
        assert!(frame.wing().is_none());
        assert!(frame.structure_positions(0).is_none());
        let factor = WingBendingFactor::try_new(0.3).unwrap();
        assert!(frame.update(factor).unwrap());
        assert_eq!(frame.bending_factor(), Some(factor));
        assert_eq!(
            frame
                .wing()
                .unwrap()
                .surface_positions()
                .each_ref()
                .map(|positions| positions.as_ptr()),
            wing_pointers
        );
    }
}
