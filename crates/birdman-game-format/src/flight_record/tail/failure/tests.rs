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
        for (site, limit) in [
            (HybridSite::Datum, HybridLimit::UndefinedReference),
            (HybridSite::Datum, HybridLimit::GlobalBeta),
            (HybridSite::StaticPolar, HybridLimit::StaticAlpha),
        ] {
            let error = HybridError::try_from_recorded(
                site,
                AeroError::OutsideEnvelope,
                Some(limit),
                stage,
            )
            .unwrap();
            round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
            )));
        }
        for surface in [
            HybridSurfaceRole::MainWing,
            HybridSurfaceRole::HorizontalTail,
            HybridSurfaceRole::VerticalTail,
        ] {
            let site = HybridSite::Proxy { surface, index: 17 };
            for limit in [
                HybridLimit::UndefinedReference,
                HybridLimit::LocalAlphaDifference,
                HybridLimit::LocalSpanAngle(HybridFlowKind::Actual),
                HybridLimit::LocalSpanAngle(HybridFlowKind::Reference),
                HybridLimit::LocalForward(HybridFlowKind::Actual),
                HybridLimit::LocalForward(HybridFlowKind::Reference),
                HybridLimit::LocalSpeed,
            ] {
                let error = HybridError::try_from_recorded(
                    site,
                    AeroError::OutsideEnvelope,
                    Some(limit),
                    stage,
                )
                .unwrap();
                round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                    LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
                )));
            }
            if surface != HybridSurfaceRole::MainWing {
                let error = HybridError::try_from_recorded(
                    site,
                    AeroError::OutsideEnvelope,
                    Some(HybridLimit::ControlledAlphaDifference),
                    stage,
                )
                .unwrap();
                round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                    LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
                )));
            }
            for cause in [
                AeroError::NonFinite,
                AeroError::Wind(WindError::OutsideGrid),
                AeroError::Wind(WindError::NonFinite),
            ] {
                for wind_site in [site, HybridSite::Datum] {
                    let error =
                        HybridError::try_from_recorded(wind_site, cause, None, stage).unwrap();
                    round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                        LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
                    )));
                }
            }
        }
        for site in [HybridSite::StaticPolar, HybridSite::Aggregate] {
            let error =
                HybridError::try_from_recorded(site, AeroError::NonFinite, None, stage).unwrap();
            round_trip(TailFlightTickError::Dynamics(DynamicsError::Load(
                LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)),
            )));
        }
    }
    for (elevator, rudder) in [(f64::NAN, 0.0), (0.21, 0.0), (0.0, -0.21)] {
        let cause = birdman_game_core::TailIncidence::try_new(elevator, rudder).unwrap_err();
        round_trip(TailFlightTickError::Control(TailControlError::Incidence(
            cause,
        )));
    }
    let surface = HybridSite::Surface(HybridSurfaceRole::HorizontalTail);
    let cause = HybridError::try_from_recorded(
        surface,
        AeroError::OutsideEnvelope,
        Some(HybridLimit::ControlledAlphaDifference),
        None,
    )
    .unwrap();
    assert_eq!(
        HybridFailureDocument::from_core(cause)
            .unwrap()
            .to_core()
            .unwrap(),
        cause
    );
}

#[test]
fn contradictory_hybrid_site_limit_cause_and_control_stage_are_rejected() {
    for (site, cause, limit) in [
        (
            HybridSite::StaticPolar,
            AeroError::OutsideEnvelope,
            Some(HybridLimit::GlobalBeta),
        ),
        (
            HybridSite::Datum,
            AeroError::OutsideEnvelope,
            Some(HybridLimit::StaticAlpha),
        ),
        (
            HybridSite::Datum,
            AeroError::OutsideEnvelope,
            Some(HybridLimit::LocalSpeed),
        ),
        (
            HybridSite::Aggregate,
            AeroError::OutsideEnvelope,
            Some(HybridLimit::LocalAlphaDifference),
        ),
        (
            HybridSite::TailIncidence,
            AeroError::OutsideEnvelope,
            Some(HybridLimit::GlobalBeta),
        ),
        (
            HybridSite::Proxy {
                surface: HybridSurfaceRole::MainWing,
                index: 1,
            },
            AeroError::OutsideEnvelope,
            Some(HybridLimit::ControlledAlphaDifference),
        ),
        (HybridSite::StaticPolar, AeroError::OutsideEnvelope, None),
        (
            HybridSite::TailIncidence,
            AeroError::NonFinite,
            Some(HybridLimit::RudderIncidence),
        ),
    ] {
        assert_eq!(
            HybridError::try_from_recorded(site, cause, limit, None),
            Err(AeroError::InvalidEnvelope)
        );
    }
    for (site, cause, limit, stage) in [
        (
            HybridSiteDocument::StaticPolar,
            AeroFailureDocument::OutsideEnvelope,
            Some(HybridLimitDocument::GlobalBeta),
            Some(AerodynamicStageDocument::Fourth),
        ),
        (
            HybridSiteDocument::TailIncidence,
            AeroFailureDocument::OutsideEnvelope,
            Some(HybridLimitDocument::ElevatorIncidence),
            Some(AerodynamicStageDocument::First),
        ),
        (
            HybridSiteDocument::Datum,
            AeroFailureDocument::NonFinite,
            None,
            None,
        ),
        (
            HybridSiteDocument::TailIncidence,
            AeroFailureDocument::Wind(WindFailureDocument::OutsideGrid),
            None,
            None,
        ),
        (
            HybridSiteDocument::TailIncidence,
            AeroFailureDocument::UnsupportedControl,
            None,
            None,
        ),
    ] {
        let document = TailTickFailureDocument::Control(TailControlFailureDocument::Incidence(
            HybridFailureDocument {
                site,
                cause,
                limit,
                stage,
            },
        ));
        assert_eq!(
            document.to_core(),
            Err(FlightRecordFormatError::InvalidRecord)
        );
    }
    let invalid = HybridError::try_from_recorded(
        HybridSite::TailIncidence,
        AeroError::OutsideEnvelope,
        Some(HybridLimit::ElevatorIncidence),
        Some(AerodynamicStage::Fourth),
    )
    .unwrap();
    assert_eq!(
        TailTickFailureDocument::from_core(TailFlightTickError::Control(
            TailControlError::Incidence(invalid)
        )),
        Err(FlightRecordFormatError::InvalidRecord)
    );
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
