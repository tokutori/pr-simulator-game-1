#![doc = "Static tail presentation derived from registered physical sections; local metres use right/up/aft axes."]

use birdman_game_core::{HybridMockDefinition, HybridSurfaceGeometry, HybridSurfaceRole};
use birdman_game_format::{RenderGeometryUnavailable, RenderTailGeometry, RenderTailGeometryValue};
use std::sync::OnceLock;

use crate::airframe_shape::tail_section_point as section_point;
use crate::wing_geometry::{COVERED_FRACTION, FOAM_COLOR, RIB_COLOR, RIB_RADIUS};

static RENDER_GEOMETRY: OnceLock<Result<RenderTailGeometryValue, &'static str>> = OnceLock::new();

/// Resolves small presentation metadata without constructing tail meshes per frame.
pub fn render_geometry(identity: Option<(&str, u32)>) -> Result<RenderTailGeometry, &'static str> {
    if identity
        != Some((
            HybridMockDefinition::CONFIGURATION_ID,
            HybridMockDefinition::MODEL_VERSION,
        ))
    {
        return Ok(RenderTailGeometry::Unavailable {
            reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
        });
    }
    RENDER_GEOMETRY
        .get_or_init(|| {
            let definition =
                HybridMockDefinition::try_new().map_err(|_| "Invalid registered aircraft")?;
            let surfaces = definition
                .surfaces()
                .map_err(|_| "Invalid registered surfaces")?;
            let geometry = surfaces
                .iter()
                .find(|surface| surface.geometry().role() == HybridSurfaceRole::HorizontalTail)
                .ok_or("Missing registered horizontal tail")?
                .geometry();
            let [first, last] = geometry.sections() else {
                return Err("Registered tail requires two sections");
            };
            Ok(RenderTailGeometryValue {
                horizontal_tail_arm_meters: -(first.quarter_chord().components()[0]
                    + last.quarter_chord().components()[0])
                    / 2.0,
            })
        })
        .as_ref()
        .copied()
        .map(|value| RenderTailGeometry::Available { value })
        .map_err(|cause| *cause)
}

#[doc = "Tail palette, reusing the Web indigo and film and shared wing structural colours."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TailMaterial {
    #[doc = "Opaque indigo cover."]
    Cover,
    #[doc = "Transparent trailing film."]
    Film,
    #[doc = "Light-blue foam over the forward part of each exposed rib."]
    Foam,
    #[doc = "Pale balsa rib."]
    Rib,
}

impl TailMaterial {
    #[doc = "Stable bulk palette order."]
    pub const ALL: [Self; 4] = [Self::Cover, Self::Film, Self::Foam, Self::Rib];

    #[doc = "sRGB 0xRRGGBB colour from the existing airframe palette."]
    pub const fn color(self) -> u32 {
        match self {
            Self::Cover => 0x304c6b,
            Self::Film => 0xd5ddd9,
            Self::Foam => FOAM_COLOR,
            Self::Rib => RIB_COLOR,
        }
    }

    #[doc = "Existing wing film opacity; opaque parts retain depth and shadow writes."]
    pub const fn opacity(self) -> f32 {
        if matches!(self, Self::Film) {
            0.22
        } else {
            1.0
        }
    }
}

#[doc = "One static quadrilateral, using the adapter's existing rectangle topology."]
pub struct TailPatch {
    #[doc = "Right/up/aft coordinates relative to the physical tail pivot."]
    pub vertices: [[f32; 3]; 4],
    #[doc = "Shared palette meaning."]
    pub material: TailMaterial,
}

#[doc = "Registered tail outline, fixed material patches and exposed structural anchors."]
pub struct TailGeometry {
    #[doc = "Physical quarter-chord midpoint in aircraft right/up/aft coordinates."]
    pub origin: [f64; 3],
    #[doc = "Opaque cover, trailing film and the visible foam strips, in that order."]
    pub patches: Vec<TailPatch>,
    #[doc = "Rib endpoints, relative to the same physical pivot as every patch."]
    pub ribs: Vec<[[f64; 3]; 2]>,
}

impl TailGeometry {
    #[doc = "Builds horizontal then vertical tails from the same registered physical model."]
    pub fn registered() -> Result<[Self; 2], &'static str> {
        let definition =
            HybridMockDefinition::try_new().map_err(|_| "Invalid registered aircraft")?;
        let surfaces = definition
            .surfaces()
            .map_err(|_| "Invalid registered surfaces")?;
        let tail = |role| {
            let geometry = surfaces
                .iter()
                .find(|surface| surface.geometry().role() == role)
                .ok_or("Missing registered tail")?
                .geometry();
            Self::from_surface(geometry)
        };
        Ok([
            tail(HybridSurfaceRole::HorizontalTail)?,
            tail(HybridSurfaceRole::VerticalTail)?,
        ])
    }

    #[doc = "Projects two physical sections without changing their outline or incidence pivot."]
    pub fn from_surface(geometry: HybridSurfaceGeometry<'_>) -> Result<Self, &'static str> {
        let intervals = match geometry.role() {
            HybridSurfaceRole::HorizontalTail => 8,
            HybridSurfaceRole::VerticalTail => 4,
            HybridSurfaceRole::MainWing => return Err("Tail geometry requires a tail surface"),
        };
        let [first, last] = geometry.sections() else {
            return Err("Registered tail requires two sections");
        };
        let first_point = first.quarter_chord().components();
        let last_point = last.quarter_chord().components();
        let center = core::array::from_fn(|axis| (first_point[axis] + last_point[axis]) / 2.0);
        let point = |span: f64, chord: f64| {
            let first = section_point(*first, center, chord);
            let last = section_point(*last, center, chord);
            core::array::from_fn(|axis| first[axis] + (last[axis] - first[axis]) * span)
        };
        let patch = |start: f64, end: f64, material| TailPatch {
            vertices: [
                point(0.0, start),
                point(1.0, start),
                point(1.0, end),
                point(0.0, end),
            ]
            .map(|vertex| vertex.map(|value| value as f32)),
            material,
        };
        let mut patches = vec![
            patch(0.0, COVERED_FRACTION, TailMaterial::Cover),
            patch(COVERED_FRACTION, 1.0, TailMaterial::Film),
        ];
        let span_length = first_point
            .iter()
            .zip(last_point)
            .map(|(first, last)| (last - first).powi(2))
            .sum::<f64>()
            .sqrt();
        let half_width = RIB_RADIUS / span_length;
        let normal_axis = if geometry.role() == HybridSurfaceRole::HorizontalTail {
            1
        } else {
            0
        };
        let mut ribs = Vec::with_capacity(intervals - 1);
        for index in 1..intervals {
            let span = index as f64 / intervals as f64;
            ribs.push([point(span, COVERED_FRACTION), point(span, 1.0)]);
            let mut vertices = [
                point(span - half_width, COVERED_FRACTION),
                point(span + half_width, COVERED_FRACTION),
                point(span + half_width, 0.75),
                point(span - half_width, 0.75),
            ];
            for vertex in &mut vertices {
                vertex[normal_axis] += RIB_RADIUS / 4.0;
            }
            patches.push(TailPatch {
                vertices: vertices.map(|vertex| vertex.map(|value| value as f32)),
                material: TailMaterial::Foam,
            });
        }
        Ok(Self {
            origin: [center[1], -center[2], -center[0]],
            patches,
            ribs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_metadata_uses_physical_pivot_and_rejects_unknown_model_names_or_versions() {
        let registered = Some((
            HybridMockDefinition::CONFIGURATION_ID,
            HybridMockDefinition::MODEL_VERSION,
        ));
        let RenderTailGeometry::Available { value } = render_geometry(registered).unwrap() else {
            panic!("registered physical geometry must be available");
        };
        let [horizontal, _] = TailGeometry::registered().unwrap();
        assert_eq!(value.horizontal_tail_arm_meters, horizontal.origin[2]);
        for identity in [
            None,
            Some(("unknown-aircraft", HybridMockDefinition::MODEL_VERSION)),
            Some((HybridMockDefinition::CONFIGURATION_ID, u32::MAX)),
        ] {
            assert_eq!(
                render_geometry(identity).unwrap(),
                RenderTailGeometry::Unavailable {
                    reason: RenderGeometryUnavailable::UnregisteredAircraftGeometry,
                }
            );
        }
        assert_eq!(
            render_geometry(registered).unwrap(),
            RenderTailGeometry::Available { value }
        );
    }

    #[test]
    fn registered_tails_preserve_physical_outline_and_join_cover_to_film() {
        let [horizontal, vertical] = TailGeometry::registered().unwrap();
        for (tail, origin, ribs, span, chord) in [
            (horizontal, [0.0, -0.1, 3.6], 7, 3.4, 2.5 / 3.4),
            (vertical, [0.0, 0.1, 1.8], 3, 0.7, 0.5 / 0.7),
        ] {
            for (actual, expected) in tail.origin.into_iter().zip(origin) {
                assert!((actual - expected).abs() < 1.0e-12);
            }
            assert_eq!(tail.ribs.len(), ribs);
            assert_eq!(tail.patches.len(), 2 + ribs);
            let cover = &tail.patches[0].vertices;
            let film = &tail.patches[1].vertices;
            assert_eq!([cover[3], cover[2]], [film[0], film[1]]);
            let span_axis = if ribs == 7 { 0 } else { 1 };
            assert!(
                (f64::from(cover[1][span_axis] - cover[0][span_axis]).abs() - span).abs() < 1.0e-6
            );
            assert!((f64::from(film[3][2] - cover[0][2]) - chord).abs() < 1.0e-6);
            for patch in &tail.patches {
                assert!(
                    patch
                        .vertices
                        .iter()
                        .flatten()
                        .all(|value| value.is_finite())
                );
                assert_ne!(patch.material, TailMaterial::Rib);
            }
            for endpoints in tail.ribs {
                assert!(endpoints[0][2] < endpoints[1][2]);
                assert!(endpoints.iter().flatten().all(|value| value.is_finite()));
            }
        }
        assert_eq!(TailMaterial::Rib.color(), RIB_COLOR);
        assert_eq!(TailMaterial::Foam.color(), FOAM_COLOR);
        assert_eq!(TailMaterial::Film.opacity(), 0.22);
    }
}
