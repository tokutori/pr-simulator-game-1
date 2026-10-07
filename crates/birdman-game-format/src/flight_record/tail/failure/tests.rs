use super::*;

fn round_trip(error: TailFlightTickError) {
    let document = TailTickFailureDocument::from_core(error).unwrap();
    let bytes = serde_json::to_vec(&document).unwrap();
    let restored: TailTickFailureDocument = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.to_core().unwrap(), error);
}

#[test]
fn every_nested_failure_preserves_its_typed_leaf() {
    round_trip(TailFlightTickError::TickOverflow);
    for cause in [
        TailControlError::NonFinite,
        TailControlError::InvalidPilotIntent,
        TailControlError::InvalidRateTarget,
    ] {
        round_trip(TailFlightTickError::Control(cause));
    }
    for cause in [
        ActuatorError::NonFinite,
        ActuatorError::InvalidAuthority,
        ActuatorError::InvalidFeedbackGain,
        ActuatorError::InvalidLimit,
        ActuatorError::InvalidTimeStep,
        ActuatorError::InvalidInterpolationFraction,
        ActuatorError::DeflectionOutOfRange,
    ] {
        round_trip(TailFlightTickError::Control(TailControlError::Actuator(
            cause,
        )));
        round_trip(TailFlightTickError::Contact(ContactError::Actuator(cause)));
    }
    for cause in [
        MathError::NonFinite,
        MathError::InvalidQuaternion,
        MathError::InvalidInterpolationFraction,
        MathError::AsymmetricTensor,
        MathError::NonPositiveDefiniteTensor,
        MathError::NonPhysicalInertiaTensor,
    ] {
        round_trip(TailFlightTickError::Dynamics(
            DynamicsError::InvalidMathValue(cause),
        ));
        round_trip(TailFlightTickError::Contact(ContactError::Math(cause)));
    }
    for cause in [
        DynamicsError::NonFinite,
        DynamicsError::InvalidMass,
        DynamicsError::InvalidGravity,
        DynamicsError::InvalidPilotLimits,
        DynamicsError::PilotOutOfRange,
        DynamicsError::PilotMotionUnrecoverable,
        DynamicsError::PilotMotionOutsidePolicyDomain,
        DynamicsError::InvalidTimeStep,
        DynamicsError::SingularMassMatrix,
        DynamicsError::Load(LoadError::OutsideDomain),
        DynamicsError::Load(LoadError::Unavailable),
    ] {
        round_trip(TailFlightTickError::Dynamics(cause));
        round_trip(TailFlightTickError::Contact(ContactError::Dynamics(cause)));
    }
    round_trip(TailFlightTickError::Contact(ContactError::EmptyGeometry));
    round_trip(TailFlightTickError::Contact(ContactError::NonAdjacentTicks));
    for cause in [
        AeroError::NonFinite,
        AeroError::InvalidMathValue(MathError::InvalidQuaternion),
        AeroError::InvalidAirDensity,
        AeroError::InvalidReferenceGeometry,
        AeroError::InvalidOrientation,
        AeroError::InvalidEnvelope,
        AeroError::IncompatibleControlEnvelope,
        AeroError::NegativeDragCoefficient,
        AeroError::InvalidElementSet,
        AeroError::InvalidPolarTable,
        AeroError::InvalidPolarMetadata,
        AeroError::InvalidHybridGeometry,
        AeroError::InvalidHybridAnchor,
        AeroError::InvalidHybridProxySet,
        AeroError::UnsupportedControl,
        AeroError::UndefinedFlowAngle,
        AeroError::OutsideEnvelope,
    ] {
        for evaluation in [
            AerodynamicEvaluationError::StaticPolar { cause },
            AerodynamicEvaluationError::Aggregate { cause },
        ] {
            round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                LoadError::Aerodynamic(evaluation),
            )));
        }
        for role in [
            AerodynamicRole::LeftWing,
            AerodynamicRole::RightWing,
            AerodynamicRole::HorizontalTail,
            AerodynamicRole::VerticalTail,
            AerodynamicRole::Fuselage,
        ] {
            round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                LoadError::Aerodynamic(AerodynamicEvaluationError::Element { role, cause }),
            )));
        }
    }
    for cause in [
        WindError::NonFinite,
        WindError::InvalidGridDimensions,
        WindError::InvalidGridSpacing,
        WindError::InvalidGridDomain,
        WindError::GridLengthMismatch,
        WindError::OutsideGrid,
    ] {
        round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
            LoadError::Aerodynamic(AerodynamicEvaluationError::StaticPolar {
                cause: AeroError::Wind(cause),
            }),
        )));
    }
}

#[test]
fn hybrid_stage_site_limit_and_cause_round_trip_without_reclassification() {
    for stage in [
        None,
        Some(AerodynamicStage::First),
        Some(AerodynamicStage::Second),
        Some(AerodynamicStage::Third),
        Some(AerodynamicStage::Fourth),
    ] {
        for surface in [
            HybridSurfaceRole::MainWing,
            HybridSurfaceRole::HorizontalTail,
            HybridSurfaceRole::VerticalTail,
        ] {
            for site in [
                HybridSite::Datum,
                HybridSite::StaticPolar,
                HybridSite::Surface(surface),
                HybridSite::Proxy { surface, index: 17 },
                HybridSite::TailIncidence,
                HybridSite::Aggregate,
            ] {
                for limit in [
                    None,
                    Some(HybridLimit::StaticAlpha),
                    Some(HybridLimit::UndefinedReference),
                    Some(HybridLimit::GlobalBeta),
                    Some(HybridLimit::ElevatorIncidence),
                    Some(HybridLimit::RudderIncidence),
                    Some(HybridLimit::LocalAlphaDifference),
                    Some(HybridLimit::ControlledAlphaDifference),
                    Some(HybridLimit::LocalSpanAngle(HybridFlowKind::Actual)),
                    Some(HybridLimit::LocalSpanAngle(HybridFlowKind::Reference)),
                    Some(HybridLimit::LocalForward(HybridFlowKind::Actual)),
                    Some(HybridLimit::LocalForward(HybridFlowKind::Reference)),
                    Some(HybridLimit::LocalSpeed),
                ] {
                    let error = HybridError::try_from_recorded(
                        site,
                        AeroError::OutsideEnvelope,
                        limit,
                        stage,
                    )
                    .unwrap();
                    round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                        LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
                    )));
                    round_trip(TailFlightTickError::Control(TailControlError::Incidence(
                        error,
                    )));
                }
                for cause in [
                    AeroError::NonFinite,
                    AeroError::Wind(WindError::OutsideGrid),
                    AeroError::Wind(WindError::NonFinite),
                ] {
                    let error = HybridError::try_from_recorded(site, cause, None, stage).unwrap();
                    round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                        LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
                    )));
                }
            }
        }
    }
}

#[test]
fn unknown_tags_surplus_fields_and_inconsistent_limits_are_rejected() {
    for input in [
        r#"{"unknown":null}"#,
        r#"{"control":"unknown"}"#,
        r#"{"dynamics":{"load":{"aerodynamic":{"hybrid":{"site":"datum","cause":"outside_envelope","limit":"unknown","stage":"second"}}}}}"#,
        r#"{"dynamics":{"load":{"aerodynamic":{"hybrid":{"site":{"proxy":{"surface":"main_wing","index":3,"extra":0}},"cause":"outside_envelope","limit":"local_speed","stage":"second"}}}}}"#,
        r#"{"tick_overflow":{"cause":"non_finite"}}"#,
        r#"{"tick_overflow":null,"contact":"empty_geometry"}"#,
    ] {
        assert!(
            serde_json::from_str::<TailTickFailureDocument>(input).is_err(),
            "{input}"
        );
    }
    let invalid = HybridFailureDocument {
        site: HybridSiteDocument::Datum,
        cause: AeroFailureDocument::NonFinite,
        limit: Some(HybridLimitDocument::LocalSpeed),
        stage: Some(AerodynamicStageDocument::Second),
    };
    assert_eq!(
        invalid.to_core(),
        Err(FlightRecordFormatError::InvalidRecord)
    );
    assert_eq!(
        HybridError::try_from_recorded(
            HybridSite::Datum,
            AeroError::NonFinite,
            Some(HybridLimit::LocalSpeed),
            None
        ),
        Err(AeroError::InvalidEnvelope)
    );
}
