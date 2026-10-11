use super::*;

#[test]
fn orientation_validates_proper_rotation_and_preserves_axis_roundtrip() {
    let forward = BodyVector::try_new(0.0, 1.0, 0.0).unwrap();
    let right = BodyVector::try_new(-1.0, 0.0, 0.0).unwrap();
    let down = BodyVector::try_new(0.0, 0.0, 1.0).unwrap();
    let orientation = ElementOrientation::try_new(forward, right, down).unwrap();
    let local = [2.0, 3.0, 4.0];
    let body = orientation.local_to_body_vector(local).unwrap();
    assert_eq!(body.components(), [-3.0, 2.0, 4.0]);
    assert_eq!(orientation.body_to_local_vector(body).unwrap(), local);
    assert_eq!(
        ElementOrientation::try_new(forward, forward, down),
        Err(AeroError::InvalidOrientation)
    );
    assert_eq!(
        ElementOrientation::try_new(forward, right, BodyVector::try_new(0.0, 0.0, -1.0).unwrap()),
        Err(AeroError::InvalidOrientation)
    );
}

#[test]
fn reference_and_air_reject_nonpositive_and_nonfinite_dimensions() {
    for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(ElementReference::try_new(value, 1.0, 1.0).is_err());
        assert!(ElementReference::try_new(1.0, value, 1.0).is_err());
        assert!(ElementReference::try_new(1.0, 1.0, value).is_err());
        assert!(UniformAir::try_new(NedVector::zero(), value).is_err());
    }
    assert!(UniformAir::try_new(NedVector::zero(), 1.225).is_ok());
}
