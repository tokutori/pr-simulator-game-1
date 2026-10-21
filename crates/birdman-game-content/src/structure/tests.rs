use super::*;
use crate::playable_aircraft;

fn rotate_tail(
    point: [f64; 3],
    pivot: [f64; 3],
    attachment: StructuralAttachment,
    angle: f64,
) -> [f64; 3] {
    let relative = subtract(point, pivot);
    let (cosine, sine) = (libm::cos(angle), libm::sin(angle));
    let rotated = match attachment {
        StructuralAttachment::HorizontalTail => [
            cosine * relative[0] + sine * relative[2],
            relative[1],
            cosine * relative[2] - sine * relative[0],
        ],
        StructuralAttachment::VerticalTail => [
            cosine * relative[0] - sine * relative[1],
            sine * relative[0] + cosine * relative[1],
            relative[2],
        ],
        _ => relative,
    };
    core::array::from_fn(|axis| pivot[axis] + rotated[axis])
}

#[test]
fn all_structural_parts_share_contact_vertices_and_complete_local_topology() {
    let definition = playable_aircraft().unwrap();
    let structure = definition.structural_geometry();
    let contact = definition.contact_points();
    assert_eq!(
        &contact[contact.len() - structure.neutral_vertices().len()..],
        structure.neutral_vertices()
    );
    for (kind, count) in [
        (StructuralPartKind::Spar, 24),
        (StructuralPartKind::Rib, 98),
        (StructuralPartKind::Stringer, 48),
        (StructuralPartKind::Foam, 54),
        (StructuralPartKind::MainBeam, 1),
        (StructuralPartKind::CockpitFrame, 18),
        (StructuralPartKind::CanopyRim, 16),
    ] {
        assert_eq!(
            structure
                .parts()
                .iter()
                .filter(|part| part.kind() == kind)
                .count(),
            count
        );
    }
    let mut end = 0;
    for part in structure.parts() {
        let range = part.vertex_range();
        assert_eq!(range.start, end);
        assert!(!range.is_empty());
        assert!(!part.indices().is_empty() && part.indices().len().is_multiple_of(3));
        assert!(
            part.indices()
                .iter()
                .all(|index| (*index as usize) < range.len())
        );
        end = range.end;
    }
    assert_eq!(end, structure.neutral_vertices().len());
}

#[test]
fn adopted_members_preserve_tube_radii_tail_rotation_and_foam_station_displacement() {
    let definition = playable_aircraft().unwrap();
    let shape = definition.shape();
    let structure = definition.structural_geometry();
    let mut output = structure.neutral_vertices().to_vec();
    let mut contact = definition.contact_points().to_vec();
    let pointer = output.as_ptr();
    let capacity = output.capacity();
    let incidence = TailIncidence::try_new(0.07, -0.05).unwrap();
    let wing_displacement = |span: f64, coefficient: f64| {
        let [first, last] = shape.surface(HybridSurfaceRole::MainWing).span_bounds_m();
        let length = if span < 0.0 { -first } else { last };
        let span = span.abs();
        let document = definition.document();
        let load = (document.mass.airframe_kg + document.pilot.mass_kg)
            * document.trim.gravity_mps2
            * coefficient
            / (last - first);
        -load / (24.0 * document.structure.wing_flexural_rigidity_nm2)
            * (6.0 * length * length * span * span - 4.0 * length * span.powi(3) + span.powi(4))
    };
    for coefficient in [-0.25, 0.0, 0.3, 1.0, 1.6] {
        let factor = WingBendingFactor::try_new(coefficient).unwrap();
        definition
            .write_structural_vertices(factor, incidence, &mut output)
            .unwrap();
        definition
            .write_deformed_contact_points_with_factor(factor, incidence, &mut contact)
            .unwrap();
        assert_eq!(&contact[contact.len() - output.len()..], output);
        for part in structure.parts() {
            let actual = &output[part.vertex_range()];
            let neutral = &structure.neutral_vertices()[part.vertex_range()];
            let transform = |mut point: [f64; 3]| {
                match part.attachment() {
                    StructuralAttachment::MainWing => {
                        point[2] += wing_displacement(point[1], coefficient)
                    }
                    StructuralAttachment::HorizontalTail | StructuralAttachment::VerticalTail => {
                        let (role, angle) =
                            if part.attachment() == StructuralAttachment::HorizontalTail {
                                (HybridSurfaceRole::HorizontalTail, incidence.elevator_rad())
                            } else {
                                (HybridSurfaceRole::VerticalTail, incidence.rudder_rad())
                            };
                        point = rotate_tail(
                            point,
                            shape.surface(role).incidence_pivot().unwrap().components(),
                            part.attachment(),
                            angle,
                        );
                    }
                    StructuralAttachment::Rigid => {}
                }
                point
            };
            if let MemberForm::Tube {
                endpoints,
                radius_m,
                ..
            } = part.form
            {
                let endpoints = endpoints.map(transform);
                let axis = subtract(endpoints[1], endpoints[0]);
                for (ring, center) in actual.chunks_exact(structure.circle.len()).zip(endpoints) {
                    let average: [f64; 3] = core::array::from_fn(|coordinate| {
                        ring.iter()
                            .map(|point| point.components()[coordinate])
                            .sum::<f64>()
                            / ring.len() as f64
                    });
                    for coordinate in 0..3 {
                        assert!((average[coordinate] - center[coordinate]).abs() < 3.0e-14);
                    }
                    for point in ring {
                        let radial = subtract(point.components(), center);
                        assert!((dot(radial, radial) - radius_m * radius_m).abs() < 3.0e-15);
                        assert!(dot(radial, axis).abs() < 3.0e-14);
                    }
                }
            } else {
                for (source, actual) in neutral.iter().zip(actual) {
                    let expected = if let MemberForm::WingFoam { station_m } = part.form {
                        let mut point = source.components();
                        point[2] += wing_displacement(station_m, coefficient);
                        point
                    } else {
                        transform(source.components())
                    };
                    for (actual, expected) in actual.components().into_iter().zip(expected) {
                        assert!((actual - expected).abs() < 3.0e-14);
                    }
                }
            }
        }
        assert_eq!(output.as_ptr(), pointer);
        assert_eq!(output.capacity(), capacity);
    }
    assert!(
        definition
            .write_structural_vertices(
                WingBendingFactor::try_new(f64::MAX).unwrap(),
                incidence,
                &mut output
            )
            .is_err()
    );
    definition
        .write_structural_vertices(
            WingBendingFactor::try_new(1.6).unwrap(),
            incidence,
            &mut output,
        )
        .unwrap();
    assert_eq!(&contact[contact.len() - output.len()..], output);
    let before = output.clone();
    assert_eq!(
        definition.write_structural_vertices(
            WingBendingFactor::try_new(1.0).unwrap(),
            incidence,
            &mut output[..1]
        ),
        Err(ShapeError::MismatchedVertexCount)
    );
    assert_eq!(output, before);
}

#[test]
fn neutral_radial_transport_is_continuous_across_coordinate_axis_ties() {
    let frame = tube_frame([[0.0; 3], [1.0; 3]]).unwrap();
    let first_axis = unit([1.0 - 1.0e-8, 1.0, 1.0]).unwrap();
    let last_axis = unit([1.0 + 1.0e-8, 1.0, 1.0]).unwrap();
    let first = transported_radial(frame, first_axis).unwrap();
    let last = transported_radial(frame, last_axis).unwrap();
    assert!(dot(first, last) > 1.0 - 8.0 * f64::EPSILON);
    assert!(dot(first, first_axis).abs() < 8.0 * f64::EPSILON);
    assert!(dot(last, last_axis).abs() < 8.0 * f64::EPSILON);
    assert_eq!(
        transported_radial(frame, frame[0].map(|coordinate| -coordinate)),
        Err(ShapeError::DegenerateMember)
    );
    assert_eq!(tube_frame([[0.0; 3]; 2]), Err(ShapeError::DegenerateMember));
}

#[test]
fn beam_attachments_follow_source_fractions_and_three_dimensional_radius() {
    for endpoints in [
        [[0.1375, 0.25, 0.73], [-4.75, 0.9, 0.57]],
        [[0.1375, 0.25, -0.7], [0.1374999999999998, 0.25, 2.0]],
    ] {
        let original = playable_aircraft().unwrap();
        let mut document = original.document().clone();
        document.structure.main_beam_endpoints_frd_m = endpoints;
        document.structure.members.cockpit.beam_attachment_fractions = [0.02, 0.12];
        let shape = AircraftShape::new(&document);
        let direction =
            core::array::from_fn::<_, 3, _>(|axis| endpoints[1][axis] - endpoints[0][axis]);
        for (point, fraction) in beam_attachments(shape)
            .unwrap()
            .into_iter()
            .zip([0.02, 0.12])
        {
            let offset = core::array::from_fn::<_, 3, _>(|axis| {
                point[axis] - endpoints[0][axis] - fraction * direction[axis]
            });
            assert!(point[1] > 0.1);
            assert!(
                (dot(offset, offset) - document.structure.main_beam_radius_m.powi(2)).abs()
                    < 1.0e-15
            );
            assert!(dot(offset, direction).abs() < 1.0e-14);
        }
    }
}

#[test]
fn source_radii_foam_width_and_tail_offset_change_the_shared_outer_vertices() {
    let original = playable_aircraft().unwrap();
    let mut document = original.document().clone();
    document.structure.members.wing.rib_radius_m *= 2.0;
    document.structure.members.wing.foam_half_width_m *= 2.0;
    document
        .structure
        .members
        .horizontal_tail
        .foam_normal_offset_m *= 2.0;
    document.structure.main_beam_radius_m *= 1.5;
    let changed = crate::AircraftDefinition::from_document(document).unwrap();
    let original_geometry = original.structural_geometry();
    let changed_geometry = changed.structural_geometry();
    for (kind, attachment) in [
        (StructuralPartKind::Rib, StructuralAttachment::MainWing),
        (StructuralPartKind::Foam, StructuralAttachment::MainWing),
        (
            StructuralPartKind::Foam,
            StructuralAttachment::HorizontalTail,
        ),
        (StructuralPartKind::MainBeam, StructuralAttachment::Rigid),
    ] {
        let index = original_geometry
            .parts()
            .iter()
            .position(|part| part.kind() == kind && part.attachment() == attachment)
            .unwrap();
        let range = original_geometry.parts()[index].vertex_range();
        assert_ne!(
            original_geometry.neutral_vertices()[range.clone()],
            changed_geometry.neutral_vertices()[range]
        );
    }
}
