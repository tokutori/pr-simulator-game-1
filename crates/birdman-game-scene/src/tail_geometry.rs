#![doc = "Static tail presentation derived from registered physical sections; local metres use right/up/aft axes."]

use birdman_game_content::{AirfoilFace, ContactSampling, SurfaceShape};
use birdman_game_core::HybridSurfaceRole;

use crate::wing_geometry::COVERED_FRACTION;

#[doc = "Engine-independent palette for the tail's indigo cover and transparent film."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TailMaterial {
    #[doc = "Opaque indigo cover."]
    Cover,
    #[doc = "Transparent trailing film."]
    Film,
}

impl TailMaterial {
    #[doc = "Stable bulk palette order."]
    pub const ALL: [Self; 2] = [Self::Cover, Self::Film];

    #[doc = "sRGB 0xRRGGBB colour from the existing airframe palette."]
    pub const fn color(self) -> u32 {
        match self {
            Self::Cover => 0x304c6b,
            Self::Film => 0xd5ddd9,
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

#[doc = "Registered neutral tail skin and fixed material patches around the physical pivot."]
pub struct TailGeometry {
    #[doc = "Physical quarter-chord midpoint in aircraft right/up/aft coordinates."]
    pub origin: [f64; 3],
    #[doc = "Opaque cover and trailing film."]
    pub patches: Vec<TailPatch>,
}

impl TailGeometry {
    #[doc = "Builds horizontal then vertical tails from the same registered physical model."]
    pub fn registered() -> Result<[Self; 2], &'static str> {
        let definition =
            birdman_game_app::registered_aircraft().map_err(|_| "Invalid registered aircraft")?;
        let tail = |role| Self::from_shape(definition.shape().surface(role));
        Ok([
            tail(HybridSurfaceRole::HorizontalTail)?,
            tail(HybridSurfaceRole::VerticalTail)?,
        ])
    }

    #[doc = "Projects the common neutral airfoil, every source section and its incidence pivot."]
    pub fn from_shape(shape: SurfaceShape<'_>) -> Result<Self, &'static str> {
        if shape.role() == HybridSurfaceRole::MainWing {
            return Err("Tail geometry requires a tail surface");
        }
        let [first_span, last_span] = shape.span_bounds_m();
        let center = shape
            .incidence_pivot()
            .map_err(|_| "Invalid tail pivot")?
            .components();
        let point = |span: f64, chord: f64, face| -> Result<[f64; 3], &'static str> {
            let [forward, right, down] = shape
                .point(span, chord, face)
                .map_err(|_| "Invalid tail profile point")?
                .components();
            Ok([right - center[1], center[2] - down, center[0] - forward])
        };
        let profile = shape.airfoil();
        let chords = if profile.camber_ratio == 0.0 && profile.thickness_ratio == 0.0 {
            vec![0.0, COVERED_FRACTION, 1.0]
        } else {
            ContactSampling::DEFAULT.chord_fractions().collect()
        };
        let faces: &[AirfoilFace] = if profile.thickness_ratio == 0.0 {
            &[AirfoilFace::Upper]
        } else {
            &[AirfoilFace::Upper, AirfoilFace::Lower]
        };
        let stations: Vec<_> = shape.span_stations_m().collect();
        let mut patches = Vec::new();
        for face in faces {
            for span in stations.windows(2) {
                for chord in chords.windows(2) {
                    let mut vertices = [
                        point(span[0], chord[0], *face)?,
                        point(span[1], chord[0], *face)?,
                        point(span[1], chord[1], *face)?,
                        point(span[0], chord[1], *face)?,
                    ];
                    if *face == AirfoilFace::Lower {
                        vertices.reverse();
                    }
                    patches.push(profile_patch(vertices, chord[1])?);
                }
            }
        }
        if profile.thickness_ratio > 0.0 {
            for span in [first_span, last_span] {
                for chord in chords.windows(2) {
                    let mut vertices = [
                        point(span, chord[0], AirfoilFace::Upper)?,
                        point(span, chord[1], AirfoilFace::Upper)?,
                        point(span, chord[1], AirfoilFace::Lower)?,
                        point(span, chord[0], AirfoilFace::Lower)?,
                    ];
                    if span == last_span {
                        vertices.reverse();
                    }
                    patches.push(profile_patch(vertices, chord[1])?);
                }
            }
        }
        Ok(Self {
            origin: [center[1], -center[2], -center[0]],
            patches,
        })
    }
}

fn profile_patch(vertices: [[f64; 3]; 4], chord_end: f64) -> Result<TailPatch, &'static str> {
    let vertices = vertices.map(|point| point.map(|component| component as f32));
    if vertices
        .iter()
        .flatten()
        .any(|component| !component.is_finite())
    {
        return Err("Tail profile exceeds render coordinate range");
    }
    Ok(TailPatch {
        vertices,
        material: if chord_end <= COVERED_FRACTION {
            TailMaterial::Cover
        } else {
            TailMaterial::Film
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_tails_preserve_physical_outline_and_join_cover_to_film() {
        let [horizontal, vertical] = TailGeometry::registered().unwrap();
        for (tail, origin, span_axis, span, chord) in [
            (horizontal, [0.0, -0.1, 3.6], 0, 3.4, 2.5 / 3.4),
            (vertical, [0.0, 0.1, 3.6], 1, 0.7, 0.5 / 0.7),
        ] {
            for (actual, expected) in tail.origin.into_iter().zip(origin) {
                assert!((actual - expected).abs() < 1.0e-12);
            }
            assert_eq!(tail.patches.len(), 2);
            let cover = &tail.patches[0].vertices;
            let film = &tail.patches[1].vertices;
            assert_eq!([cover[3], cover[2]], [film[0], film[1]]);
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
            }
        }
        assert_eq!(TailMaterial::Film.opacity(), 0.22);
    }

    #[test]
    fn nonrectangular_tail_keeps_every_sweep_chord_and_dihedral_break() {
        use birdman_game_content::AircraftDefinition;
        use birdman_game_format::aircraft::AircraftSectionDocument;

        let sections: Vec<_> = [
            ([-4.0, -2.0, 0.2], 0.4),
            ([-3.5, 0.0, -0.1], 1.2),
            ([-4.0, 2.0, 0.2], 0.4),
        ]
        .into_iter()
        .map(|(point, chord)| AircraftSectionDocument {
            quarter_chord_frd_m: point,
            chord_m: chord,
        })
        .collect();
        let mut document = birdman_game_app::registered_aircraft()
            .unwrap()
            .document()
            .clone();
        let source = document
            .surfaces
            .iter_mut()
            .find(|surface| surface.role.core_role() == HybridSurfaceRole::HorizontalTail)
            .unwrap();
        source.sections.clone_from(&sections);
        let definition = AircraftDefinition::from_document(document).unwrap();
        let tail = TailGeometry::from_shape(
            definition
                .shape()
                .surface(HybridSurfaceRole::HorizontalTail),
        )
        .unwrap();
        for section in sections {
            let [forward, right, down] = section.quarter_chord_frd_m;
            for chord in [0.0, COVERED_FRACTION, 1.0] {
                let expected = [right, -down, -forward + (chord - 0.25) * section.chord_m];
                assert!(
                    tail.patches
                        .iter()
                        .flat_map(|patch| patch.vertices)
                        .any(|point| {
                            point.into_iter().zip(tail.origin).zip(expected).all(
                                |((actual, origin), expected)| {
                                    (f64::from(actual) + origin - expected).abs() < 2.0e-7
                                },
                            )
                        })
                );
            }
        }
    }

    #[test]
    fn cambered_thick_tails_share_contact_profile_vertices_and_form_a_closed_skin() {
        use birdman_game_content::AircraftDefinition;
        use birdman_game_format::aircraft::AircraftAirfoilDocument;
        use std::collections::BTreeMap;

        let mut document = birdman_game_app::registered_aircraft()
            .unwrap()
            .document()
            .clone();
        for surface in &mut document.surfaces {
            if surface.role.core_role() != HybridSurfaceRole::MainWing {
                surface.airfoil = AircraftAirfoilDocument {
                    camber_ratio: 0.04,
                    camber_position_ratio: 0.4,
                    thickness_ratio: 0.12,
                };
            }
        }
        let definition = AircraftDefinition::from_document(document).unwrap();
        for role in [
            HybridSurfaceRole::HorizontalTail,
            HybridSurfaceRole::VerticalTail,
        ] {
            let shape = definition.shape().surface(role);
            let tail = TailGeometry::from_shape(shape).unwrap();
            let skin = &tail.patches;
            for span in shape.span_stations_m() {
                for chord in ContactSampling::DEFAULT.chord_fractions() {
                    for face in [AirfoilFace::Upper, AirfoilFace::Lower] {
                        let [forward, right, down] =
                            shape.point(span, chord, face).unwrap().components();
                        let expected = [right, -down, -forward];
                        assert!(skin.iter().flat_map(|patch| patch.vertices).any(|point| {
                            point.into_iter().zip(tail.origin).zip(expected).all(
                                |((actual, origin), expected)| {
                                    (f64::from(actual) + origin - expected).abs()
                                        <= f64::from(f32::EPSILON) * expected.abs().max(1.0)
                                },
                            )
                        }));
                    }
                }
            }
            let mut edges = BTreeMap::new();
            for patch in skin {
                for index in 0..4 {
                    let first = patch.vertices[index].map(|component| {
                        if component == 0.0 {
                            0
                        } else {
                            component.to_bits()
                        }
                    });
                    let second = patch.vertices[(index + 1) % 4].map(|component| {
                        if component == 0.0 {
                            0
                        } else {
                            component.to_bits()
                        }
                    });
                    if first != second {
                        let edge = if first < second {
                            [first, second]
                        } else {
                            [second, first]
                        };
                        *edges.entry(edge).or_insert(0) += 1;
                    }
                }
            }
            assert!(edges.values().all(|count| *count == 2));
        }
    }
}
