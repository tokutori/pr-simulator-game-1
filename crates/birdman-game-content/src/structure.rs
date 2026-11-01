use alloc::vec::Vec;
use birdman_game_core::{BodyPoint, HybridSurfaceRole, TailIncidence, WingBendingFactor};
use core::ops::Range;

use crate::{AircraftShape, AirfoilFace, ContactSampling, ShapeError, SurfaceShape};

/// Physical attachment controlling the motion of a structural part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralAttachment {
    /// Fixed to the aircraft datum.
    Rigid,
    /// Main-wing elastic deformation.
    MainWing,
    /// Horizontal-tail incidence about its physical pivot.
    HorizontalTail,
    /// Vertical-tail incidence about its physical pivot.
    VerticalTail,
}

/// Structural meaning used by contact diagnostics and presentation materials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralPartKind {
    /// Carbon main-wing spar segment.
    Spar,
    /// Chordwise rib tube.
    Rib,
    /// Spanwise stringer tube.
    Stringer,
    /// Extruded wing profile or raised tail patch.
    Foam,
    /// Longitudinal carbon beam.
    MainBeam,
    /// Support joining the vertical-tail root, pivot axis and main beam.
    TailSupport,
    /// Cockpit frame, brace or beam attachment.
    CockpitFrame,
    /// Canopy-opening rim segment.
    CanopyRim,
}

enum MemberForm {
    Tube {
        endpoints: [[f64; 3]; 2],
        radius_m: f64,
        frame: [[f64; 3]; 3],
    },
    WingFoam {
        station_m: f64,
    },
    Fixed,
}

/// Fixed topology and identity of one physical structural member.
pub struct StructuralPart {
    kind: StructuralPartKind,
    attachment: StructuralAttachment,
    vertices: Range<usize>,
    indices: Vec<u32>,
    form: MemberForm,
}

impl StructuralPart {
    /// Returns the physical member's meaning.
    pub const fn kind(&self) -> StructuralPartKind {
        self.kind
    }

    /// Returns the member's physical attachment.
    pub const fn attachment(&self) -> StructuralAttachment {
        self.attachment
    }

    /// Returns its range in the assembly's fixed-order vertex buffer.
    pub fn vertex_range(&self) -> Range<usize> {
        self.vertices.clone()
    }

    /// Returns outward triangle indices relative to this part's first vertex.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

/// Prepared FRD outer vertices and topology shared by contact and rendering.
pub struct StructuralGeometry {
    vertices: Vec<BodyPoint>,
    parts: Vec<StructuralPart>,
    circle: Vec<[f64; 2]>,
}

impl StructuralGeometry {
    /// Prepares all members from the admitted document and one canonical ring sampling.
    pub(crate) fn from_shape(
        shape: AircraftShape<'_>,
        sampling: ContactSampling,
    ) -> Result<Self, ShapeError> {
        let mut geometry = Self {
            vertices: Vec::new(),
            parts: Vec::new(),
            circle: (0..sampling.circumference_steps())
                .map(|step| {
                    let angle = core::f64::consts::TAU * f64::from(step)
                        / f64::from(sampling.circumference_steps());
                    [libm::cos(angle), libm::sin(angle)]
                })
                .collect(),
        };
        geometry.add_wing(shape)?;
        geometry.add_tail(shape, HybridSurfaceRole::HorizontalTail)?;
        geometry.add_tail(shape, HybridSurfaceRole::VerticalTail)?;
        let structure = shape.structure();
        geometry.add_tube(
            StructuralPartKind::MainBeam,
            StructuralAttachment::Rigid,
            structure.main_beam_endpoints_frd_m,
            structure.main_beam_radius_m,
        )?;
        let fin = shape.surface(HybridSurfaceRole::VerticalTail);
        let fin_root = mean_point(fin, fin.span_bounds_m()[1], 0.25)?;
        let pivot = fin.incidence_pivot()?.components();
        let axis_root = [pivot[0], pivot[1], fin_root[2]];
        let [beam_start, beam_end] = structure.main_beam_endpoints_frd_m;
        let beam_direction = subtract(beam_end, beam_start);
        let fraction = (dot(subtract(axis_root, beam_start), beam_direction)
            / dot(beam_direction, beam_direction))
        .clamp(0.0, 1.0);
        let beam_root =
            core::array::from_fn(|axis| beam_start[axis] + fraction * beam_direction[axis]);
        if axis_root != beam_root {
            geometry.add_tube(
                StructuralPartKind::TailSupport,
                StructuralAttachment::Rigid,
                [axis_root, beam_root],
                structure.members.tail_support_radius_m,
            )?;
        }
        if fin_root != axis_root {
            geometry.add_tube(
                StructuralPartKind::TailSupport,
                StructuralAttachment::VerticalTail,
                [axis_root, fin_root],
                structure.members.tail_support_radius_m,
            )?;
        }
        geometry.add_cockpit(shape, sampling)?;
        Ok(geometry)
    }

    /// Borrows the immutable neutral FRD vertices of all parts.
    pub fn neutral_vertices(&self) -> &[BodyPoint] {
        &self.vertices
    }

    /// Borrows fixed part identities, topology and vertex ranges.
    pub fn parts(&self) -> &[StructuralPart] {
        &self.parts
    }

    /// Writes every member from one accepted coefficient and held physical incidence.
    /// Callers publish the buffer after the complete operation succeeds.
    pub(crate) fn write_vertices(
        &self,
        shape: AircraftShape<'_>,
        factor: WingBendingFactor,
        incidence: TailIncidence,
        output: &mut [BodyPoint],
    ) -> Result<(), ShapeError> {
        if output.len() != self.vertices.len() {
            return Err(ShapeError::MismatchedVertexCount);
        }
        let horizontal = shape
            .surface(HybridSurfaceRole::HorizontalTail)
            .actuation(incidence)?;
        let vertical = shape
            .surface(HybridSurfaceRole::VerticalTail)
            .actuation(incidence)?;
        for part in &self.parts {
            let target = &mut output[part.vertices.clone()];
            let neutral = &self.vertices[part.vertices.clone()];
            match part.attachment {
                StructuralAttachment::Rigid => target.copy_from_slice(neutral),
                StructuralAttachment::HorizontalTail | StructuralAttachment::VerticalTail => {
                    let actuation = if part.attachment == StructuralAttachment::HorizontalTail {
                        &horizontal
                    } else {
                        &vertical
                    };
                    for (source, target) in neutral.iter().zip(target) {
                        *target = actuation.apply(*source)?;
                    }
                }
                StructuralAttachment::MainWing if factor.value() == 0.0 => {
                    target.copy_from_slice(neutral);
                }
                StructuralAttachment::MainWing => match part.form {
                    MemberForm::Tube {
                        endpoints,
                        radius_m,
                        frame,
                    } => {
                        let deformed = endpoints.map(|point| {
                            shape
                                .wing_displacement_with_factor(point[1], factor)
                                .and_then(|offset| {
                                    body_point(point)?
                                        .translated(offset)
                                        .map_err(ShapeError::Math)
                                })
                        });
                        let endpoints = [deformed[0]?, deformed[1]?].map(BodyPoint::components);
                        let axis = unit(subtract(endpoints[1], endpoints[0]))?;
                        let radial = transported_radial(frame, axis)?;
                        write_tube(
                            endpoints,
                            radius_m,
                            [axis, radial, cross(axis, radial)],
                            &self.circle,
                            target,
                        )?;
                    }
                    MemberForm::WingFoam { station_m } => {
                        let offset = shape.wing_displacement_with_factor(station_m, factor)?;
                        for (source, target) in neutral.iter().zip(target) {
                            *target = source.translated(offset).map_err(ShapeError::Math)?;
                        }
                    }
                    MemberForm::Fixed => return Err(ShapeError::DegenerateMember),
                },
            }
        }
        Ok(())
    }

    fn add_tube(
        &mut self,
        kind: StructuralPartKind,
        attachment: StructuralAttachment,
        endpoints: [[f64; 3]; 2],
        radius_m: f64,
    ) -> Result<(), ShapeError> {
        let frame = tube_frame(endpoints)?;
        let count = self.circle.len() as u32;
        let mut vertices = alloc::vec![BodyPoint::origin(); 2 * self.circle.len()];
        write_tube(endpoints, radius_m, frame, &self.circle, &mut vertices)?;
        let mut indices = Vec::new();
        for current in 0..count {
            let next = (current + 1) % count;
            indices.extend([
                current,
                next,
                count + current,
                next,
                count + next,
                count + current,
            ]);
        }
        for current in 1..count - 1 {
            indices.extend([
                0,
                current + 1,
                current,
                count,
                count + current,
                count + current + 1,
            ]);
        }
        self.add_part(
            kind,
            attachment,
            vertices,
            indices,
            MemberForm::Tube {
                endpoints,
                radius_m,
                frame,
            },
        );
        Ok(())
    }

    fn add_part(
        &mut self,
        kind: StructuralPartKind,
        attachment: StructuralAttachment,
        vertices: Vec<BodyPoint>,
        indices: Vec<u32>,
        form: MemberForm,
    ) {
        let start = self.vertices.len();
        self.vertices.extend(vertices);
        self.parts.push(StructuralPart {
            kind,
            attachment,
            vertices: start..self.vertices.len(),
            indices,
            form,
        });
    }

    fn add_wing(&mut self, shape: AircraftShape<'_>) -> Result<(), ShapeError> {
        let source = &shape.structure().members.wing;
        let wing = shape.surface(HybridSurfaceRole::MainWing);
        let bounds = wing.span_bounds_m();
        for tip in bounds {
            for segment in 0..source.span_segments {
                let stations = [segment, segment + 1]
                    .map(|step| tip * f64::from(step) / f64::from(source.span_segments));
                let radius = source.spar_radii_m[0]
                    + (source.spar_radii_m[1] - source.spar_radii_m[0])
                        * (f64::from(segment) + 0.5)
                        / f64::from(source.span_segments);
                let endpoints = [
                    wing.section(stations[0])?.quarter_chord_frd_m,
                    wing.section(stations[1])?.quarter_chord_frd_m,
                ];
                self.add_tube(
                    StructuralPartKind::Spar,
                    StructuralAttachment::MainWing,
                    endpoints,
                    radius,
                )?;
                for chord in &source.stringer_chord_fractions {
                    self.add_tube(
                        StructuralPartKind::Stringer,
                        StructuralAttachment::MainWing,
                        [
                            mean_point(wing, stations[0], *chord)?,
                            mean_point(wing, stations[1], *chord)?,
                        ],
                        source.stringer_radius_m,
                    )?;
                }
            }
            for rib in 1..=source.rib_count {
                let station = tip * f64::from(rib) / (f64::from(source.rib_count) + 1.0);
                for chords in source.rib_chord_knots.windows(2) {
                    self.add_tube(
                        StructuralPartKind::Rib,
                        StructuralAttachment::MainWing,
                        [
                            mean_point(wing, station, chords[0])?,
                            mean_point(wing, station, chords[1])?,
                        ],
                        source.rib_radius_m,
                    )?;
                }
                let mut outline = Vec::new();
                for (face, reverse) in [(AirfoilFace::Upper, false), (AirfoilFace::Lower, true)] {
                    for index in 0..source.foam_chord_fractions.len() {
                        let index = if reverse {
                            source.foam_chord_fractions.len() - 1 - index
                        } else {
                            index
                        };
                        outline.push(
                            wing.point(station, source.foam_chord_fractions[index], face)?
                                .components(),
                        );
                    }
                }
                let mut vertices = Vec::new();
                for lateral in [-source.foam_half_width_m, source.foam_half_width_m] {
                    for point in &outline {
                        vertices.push(body_point([point[0], station + lateral, point[2]])?);
                    }
                }
                self.add_part(
                    StructuralPartKind::Foam,
                    StructuralAttachment::MainWing,
                    vertices,
                    prism_indices(outline.len() as u32),
                    MemberForm::WingFoam { station_m: station },
                );
            }
        }
        Ok(())
    }

    fn add_tail(
        &mut self,
        shape: AircraftShape<'_>,
        role: HybridSurfaceRole,
    ) -> Result<(), ShapeError> {
        let (source, attachment) = match role {
            HybridSurfaceRole::HorizontalTail => (
                &shape.structure().members.horizontal_tail,
                StructuralAttachment::HorizontalTail,
            ),
            HybridSurfaceRole::VerticalTail => (
                &shape.structure().members.vertical_tail,
                StructuralAttachment::VerticalTail,
            ),
            HybridSurfaceRole::MainWing => return Err(ShapeError::DegenerateMember),
        };
        let surface = shape.surface(role);
        let [first, last] = surface.span_bounds_m();
        for rib in 1..source.rib_intervals {
            let station = first + (last - first) * f64::from(rib) / f64::from(source.rib_intervals);
            self.add_tube(
                StructuralPartKind::Rib,
                attachment,
                [
                    mean_point(surface, station, source.rib_chord_bounds[0])?,
                    mean_point(surface, station, source.rib_chord_bounds[1])?,
                ],
                source.rib_radius_m,
            )?;
            let span = [
                (station - source.foam_half_width_m).max(first),
                (station + source.foam_half_width_m).min(last),
            ];
            let chords = source.foam_chord_bounds;
            let vertices = [
                (span[0], chords[0]),
                (span[1], chords[0]),
                (span[1], chords[1]),
                (span[0], chords[1]),
            ]
            .into_iter()
            .map(|(span, chord)| {
                let mut point = surface.point(span, chord, AirfoilFace::Upper)?.components();
                match role {
                    HybridSurfaceRole::HorizontalTail => point[2] -= source.foam_normal_offset_m,
                    HybridSurfaceRole::VerticalTail => point[1] += source.foam_normal_offset_m,
                    HybridSurfaceRole::MainWing => return Err(ShapeError::DegenerateMember),
                }
                body_point(point)
            })
            .collect::<Result<Vec<_>, _>>()?;
            self.add_part(
                StructuralPartKind::Foam,
                attachment,
                vertices,
                alloc::vec![0, 1, 2, 0, 2, 3],
                MemberForm::Fixed,
            );
        }
        Ok(())
    }

    fn add_cockpit(
        &mut self,
        shape: AircraftShape<'_>,
        sampling: ContactSampling,
    ) -> Result<(), ShapeError> {
        let structure = shape.structure();
        let source = &structure.members.cockpit;
        let stations = &structure.cockpit_stations;
        let forward = |fraction| {
            stations[0].forward_m
                + fraction * (stations[stations.len() - 1].forward_m - stations[0].forward_m)
        };
        let frame_radius = structure.cockpit_frame_radius_m;
        let nose = inset_shell_point(
            shape,
            stations[source.nose_station as usize].forward_m,
            [0.0, -1.0],
            frame_radius,
            sampling,
        )?;
        let lower = [-1.0, 1.0].map(|side| {
            source.lower_station_fractions.map(|fraction| {
                inset_shell_point(
                    shape,
                    forward(fraction),
                    [side, -1.0],
                    frame_radius,
                    sampling,
                )
            })
        });
        let attachments = beam_attachments(shape)?;
        for (side_index, side) in [-1.0, 1.0].into_iter().enumerate() {
            let [upper_front, upper_rear] = source.upper_station_fractions.map(|fraction| {
                inset_shell_point(
                    shape,
                    forward(fraction),
                    [side, 1.0],
                    frame_radius,
                    sampling,
                )
            });
            let [upper_front, upper_rear] = [upper_front?, upper_rear?];
            let [lower_front, lower_rear] = [lower[side_index][0]?, lower[side_index][1]?];
            for (first, last, radius) in [
                (nose, upper_front, frame_radius),
                (upper_front, upper_rear, frame_radius),
                (lower_front, lower_rear, frame_radius),
                (upper_front, lower_front, source.cross_radius_m),
                (upper_rear, lower_rear, source.cross_radius_m),
                (lower_front, upper_rear, source.diagonal_radius_m),
            ] {
                self.add_tube(
                    StructuralPartKind::CockpitFrame,
                    StructuralAttachment::Rigid,
                    [first, last].map(BodyPoint::components),
                    radius,
                )?;
            }
            for attachment in attachments {
                let first = upper_front.components();
                let last = upper_rear.components();
                let fraction = (attachment[0] - first[0]) / (last[0] - first[0]);
                let frame_point = core::array::from_fn(|axis| {
                    first[axis] + fraction * (last[axis] - first[axis])
                });
                self.add_tube(
                    StructuralPartKind::CockpitFrame,
                    StructuralAttachment::Rigid,
                    [attachment, frame_point],
                    source.attachment_radius_m,
                )?;
            }
        }
        for (left, right) in lower[0].into_iter().zip(lower[1]) {
            self.add_tube(
                StructuralPartKind::CockpitFrame,
                StructuralAttachment::Rigid,
                [left?, right?].map(BodyPoint::components),
                source.cross_radius_m,
            )?;
        }
        let station = stations[source.rim_station as usize];
        for step in 0..source.rim_segments {
            let endpoint = |step| {
                let angle =
                    core::f64::consts::TAU * f64::from(step) / f64::from(source.rim_segments);
                let point = shape.cockpit_point(station.forward_m, -angle)?.components();
                inset_shell_point(
                    shape,
                    station.forward_m,
                    [
                        point[1] / station.half_width_m,
                        (station.center_down_m - point[2]) / station.half_height_m,
                    ],
                    source.rim_radius_m,
                    sampling,
                )
            };
            self.add_tube(
                StructuralPartKind::CanopyRim,
                StructuralAttachment::Rigid,
                [endpoint(step)?, endpoint(step + 1)?].map(BodyPoint::components),
                source.rim_radius_m,
            )?;
        }
        Ok(())
    }
}

fn body_point(point: [f64; 3]) -> Result<BodyPoint, ShapeError> {
    BodyPoint::try_new(point[0], point[1], point[2]).map_err(ShapeError::Math)
}

fn subtract(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    core::array::from_fn(|axis| first[axis] - second[axis])
}

fn dot(first: [f64; 3], second: [f64; 3]) -> f64 {
    first
        .into_iter()
        .zip(second)
        .map(|(first, second)| first * second)
        .sum()
}

fn cross(first: [f64; 3], second: [f64; 3]) -> [f64; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

fn unit(vector: [f64; 3]) -> Result<[f64; 3], ShapeError> {
    let length = libm::hypot(libm::hypot(vector[0], vector[1]), vector[2]);
    if !length.is_finite() || length == 0.0 {
        return Err(ShapeError::DegenerateMember);
    }
    Ok(vector.map(|component| component / length))
}

fn tube_frame(endpoints: [[f64; 3]; 2]) -> Result<[[f64; 3]; 3], ShapeError> {
    let axis = unit(subtract(endpoints[1], endpoints[0]))?;
    let reference_axis = (0..3)
        .min_by(|first, second| axis[*first].abs().total_cmp(&axis[*second].abs()))
        .unwrap();
    let reference = core::array::from_fn(|coordinate| f64::from(coordinate == reference_axis));
    let radial = unit(cross(axis, reference))?;
    Ok([axis, radial, cross(axis, radial)])
}

fn transported_radial(frame: [[f64; 3]; 3], axis: [f64; 3]) -> Result<[f64; 3], ShapeError> {
    let rotation = cross(frame[0], axis);
    let denominator = 1.0 + dot(frame[0], axis);
    if denominator <= 32.0 * f64::EPSILON {
        return Err(ShapeError::DegenerateMember);
    }
    let first = cross(rotation, frame[1]);
    let second = cross(rotation, first);
    let rotated = core::array::from_fn(|coordinate| {
        frame[1][coordinate] + first[coordinate] + second[coordinate] / denominator
    });
    let projection = dot(rotated, axis);
    unit(core::array::from_fn(|coordinate| {
        rotated[coordinate] - projection * axis[coordinate]
    }))
}

fn write_tube(
    endpoints: [[f64; 3]; 2],
    radius: f64,
    frame: [[f64; 3]; 3],
    circle: &[[f64; 2]],
    output: &mut [BodyPoint],
) -> Result<(), ShapeError> {
    for (center, ring) in endpoints
        .into_iter()
        .zip(output.chunks_exact_mut(circle.len()))
    {
        for ([cosine, sine], target) in circle.iter().zip(ring) {
            *target = body_point(core::array::from_fn(|coordinate| {
                center[coordinate]
                    + radius * (frame[1][coordinate] * cosine + frame[2][coordinate] * sine)
            }))?;
        }
    }
    Ok(())
}

fn mean_point(surface: SurfaceShape<'_>, span: f64, chord: f64) -> Result<[f64; 3], ShapeError> {
    let upper = surface.point(span, chord, AirfoilFace::Upper)?.components();
    let lower = surface.point(span, chord, AirfoilFace::Lower)?.components();
    Ok(core::array::from_fn(|axis| {
        0.5 * upper[axis] + 0.5 * lower[axis]
    }))
}

fn prism_indices(count: u32) -> Vec<u32> {
    let mut indices = Vec::new();
    for current in 1..count - 1 {
        indices.extend([
            0,
            current + 1,
            current,
            count,
            count + current,
            count + current + 1,
        ]);
    }
    for current in 0..count {
        let next = (current + 1) % count;
        indices.extend([
            current,
            next,
            count + current,
            next,
            count + next,
            count + current,
        ]);
    }
    indices
}

fn beam_attachments(shape: AircraftShape<'_>) -> Result<[[f64; 3]; 2], ShapeError> {
    let structure = shape.structure();
    let [first, last] = structure.main_beam_endpoints_frd_m;
    let [_, radial, normal] = shape.beam_frame()?.map(|vector| vector.components());
    let projected =
        core::array::from_fn(|axis| radial[axis] * radial[2] + normal[axis] * normal[2]);
    let underside = if dot(projected, projected) <= f64::EPSILON * f64::EPSILON {
        radial
    } else {
        unit(projected)?
    };
    Ok(structure
        .members
        .cockpit
        .beam_attachment_fractions
        .map(|fraction| {
            core::array::from_fn(|axis| {
                first[axis]
                    + fraction * (last[axis] - first[axis])
                    + structure.main_beam_radius_m * underside[axis]
            })
        }))
}

fn inset_shell_point(
    shape: AircraftShape<'_>,
    forward: f64,
    direction: [f64; 2],
    radius: f64,
    sampling: ContactSampling,
) -> Result<BodyPoint, ShapeError> {
    let section = shape.cockpit_station(forward)?;
    let slope = shape
        .structure()
        .cockpit_stations
        .windows(2)
        .map(|pair| {
            libm::hypot(
                pair[1].half_width_m - pair[0].half_width_m,
                (pair[1].center_down_m - pair[0].center_down_m).abs()
                    + (pair[1].half_height_m - pair[0].half_height_m).abs(),
            ) / (pair[0].forward_m - pair[1].forward_m)
        })
        .fold(0.0, f64::max);
    let inset = radius * (1.0 + slope);
    let desired = [
        direction[0] * section.half_width_m,
        direction[1] * section.half_height_m,
    ];
    let sample = |step| {
        let angle =
            -core::f64::consts::TAU * f64::from(step) / f64::from(sampling.circumference_steps());
        shape.cockpit_point(forward, angle).map(|point| {
            [
                point.components()[1],
                section.center_down_m - point.components()[2],
            ]
        })
    };
    let mut scale: f64 = 1.0;
    for step in 0..sampling.circumference_steps() {
        let first = sample(step)?;
        let last = sample(step + 1)?;
        let edge = [last[0] - first[0], last[1] - first[1]];
        let length = libm::hypot(edge[0], edge[1]);
        let normal = [edge[1] / length, -edge[0] / length];
        let projection = normal[0] * desired[0] + normal[1] * desired[1];
        if projection > 0.0 {
            scale = scale.min((normal[0] * first[0] + normal[1] * first[1] - inset) / projection);
        }
    }
    if scale < 0.0 {
        return Err(ShapeError::DegenerateMember);
    }
    body_point([
        forward,
        desired[0] * scale,
        section.center_down_m - desired[1] * scale,
    ])
}

#[cfg(test)]
mod tests;
