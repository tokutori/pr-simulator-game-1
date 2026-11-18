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

#[test]
fn ground_effect_factor_matches_wieselsberger_values() {
    let factor = |height: f64, span: f64| {
        GroundEffect::from_height_above_surface(height, span)
            .unwrap()
            .factor()
    };
    // At the surface the factor is zero; negative heights clamp to it.
    assert_eq!(factor(0.0, 4.0), 0.0);
    assert_eq!(factor(-2.0, 4.0), 0.0);
    // h/b = 1/16 gives (16 h/b)^2 = 1 and factor 1/2, exactly.
    assert_eq!(factor(0.25, 4.0), 0.5);
    assert_eq!(factor(1.0, 16.0), 0.5);
    // h/b = 0.1 gives 2.56 / 3.56; h/b = 0.25 gives 16 / 17.
    let near = |actual: f64, expected: f64| assert!((actual - expected).abs() < 1.0e-15);
    near(factor(0.4, 4.0), 2.56 / 3.56);
    near(factor(1.0, 4.0), 16.0 / 17.0);
    // The factor increases strictly with height and approaches one.
    let mut previous = 0.0;
    for height in [0.1, 0.5, 1.0, 2.0, 4.0, 8.0] {
        let value = factor(height, 4.0);
        assert!(previous < value && value < 1.0);
        previous = value;
    }
    assert!(factor(4.0e6, 4.0) > 1.0 - 1.0e-12);
    assert_eq!(GroundEffect::FREE_AIR.factor(), 1.0);
    // Only the ratio h/b matters.
    near(factor(2.0, 8.0), factor(1.0, 4.0));
}

#[test]
fn ground_effect_rejects_nonfinite_height_and_nonpositive_span() {
    for height in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            GroundEffect::from_height_above_surface(height, 4.0),
            Err(AeroError::NonFinite)
        );
    }
    for span in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            GroundEffect::from_height_above_surface(1.0, span),
            Err(AeroError::InvalidReferenceGeometry)
        );
    }
}
