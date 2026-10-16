use crate::*;

#[test]
fn off_grid_tail_contact_seals_record_and_score_at_the_first_fractional_time() {
    let aircraft = AircraftModel::try_new(
        10.0,
        InertiaTensor::diagonal(2.0, 3.0, 4.0).unwrap(),
        1.0,
        0.0,
        -0.5,
        0.5,
        1.0,
        2.0,
    )
    .unwrap();
    let coefficients = StaticPolarCoefficients::try_new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0).unwrap();
    let rows = [-0.5, 0.5].map(|alpha| StaticPolarRow::try_new(alpha, coefficients).unwrap());
    let polar = StaticPolar::try_new(
        &rows,
        ElementReference::try_new(1.0, 1.0, 1.0).unwrap(),
        BodyPoint::origin(),
        PolarMomentAxes::BodyFrd,
        StaticPolarMetadata::try_new(PolarAnalysisMethod::SoftwareFixture, "off-grid-contact", 1)
            .unwrap(),
    )
    .unwrap();
    let sections = [-0.5, 0.5].map(|span| {
        HybridSection::try_new(BodyPoint::try_new(0.0, span, 0.0).unwrap(), 1.0).unwrap()
    });
    let geometry = HybridSurfaceGeometry::try_new(
        HybridSurfaceRole::HorizontalTail,
        &sections,
        PlanformSymmetry::MirrorSpan,
    )
    .unwrap();
    let proxies = [HybridProxy::try_new(
        geometry,
        [-0.5, 0.5],
        ElementOrientation::IDENTITY,
        HybridAnchor::try_new(0.0, 0.0).unwrap(),
    )
    .unwrap()];
    let surfaces = [HybridSurface::try_new(geometry, &proxies).unwrap()];
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(polar, &surfaces).unwrap(),
        1.225,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let profile = TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap();
    let config = TailFlightTickConfig::new(
        ControlMode::Manual,
        profile,
        TailPilotPositionMapping::try_new(&aircraft, 0.0).unwrap(),
        Gravity::try_new(0.0).unwrap(),
    );
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let points = [BodyPoint::try_new(0.0, 0.0, 1.0).unwrap()];
    for rate in [2.0, 10.0] {
        for penetration in [0.0, 1.0e-7] {
            let start_angle = -0.33 * rate * PHYSICS_DT_SECONDS;
            let launch = CompositeCgLaunchConditions::try_new(
                NedPoint::try_new(0.0, 0.0, -1.0 + penetration).unwrap(),
                NedVector::try_new(10.0, 0.0, 0.0).unwrap(),
                UnitQuaternion::try_new(
                    libm::cos(start_angle * 0.5),
                    0.0,
                    libm::sin(start_angle * 0.5),
                    0.0,
                )
                .unwrap(),
                BodyVector::try_new(0.0, rate, 0.0).unwrap(),
                0.0,
                0.0,
            )
            .unwrap();
            let parameters = TailFlightScenarioParameters::try_new(
                aircraft,
                launch,
                TailIncidence::neutral(),
                Gravity::try_new(0.0).unwrap(),
                &points,
                CourseAxis::try_new(1.0, 0.0).unwrap(),
            )
            .unwrap();
            let scenario = TailFlightScenario::try_new(parameters, loads, profile).unwrap();
            let previous = scenario.initial_state();
            let endpoint =
                advance_tail_flight_tick(&aircraft, previous, config, input, &loads).unwrap();
            let end = endpoint.flight_state().attitude_body_to_ned().components();
            let end_angle = 2.0 * libm::atan2(end[2], end[0]);
            let boundary_angle = -libm::acos(1.0 - penetration);
            let expected_fraction = if rate == 10.0 {
                (boundary_angle - start_angle) / (end_angle - start_angle)
            } else {
                let numerator = libm::sin((boundary_angle - start_angle) * 0.5);
                numerator / (numerator + libm::sin((end_angle - boundary_angle) * 0.5))
            };
            let identity = SessionScenarioIdentity {
                catalog_version: 1,
                scenario_id: 207,
                scenario_version: 1,
                aircraft_model_version: 1,
                environment_version: 1,
                controller_profile_version: 1,
                seed: 207,
            };
            let mut session = GameSession::new();
            session.open_setup().unwrap();
            session
                .prepare_flight(
                    GameSessionConfiguration::try_new_tail(
                        scenario,
                        ControlMode::Manual,
                        10,
                        identity,
                    )
                    .unwrap(),
                )
                .unwrap();
            session.mark_briefing_ready().unwrap();
            session.start_countdown(1).unwrap();
            session
                .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
                .unwrap();
            let result = session
                .advance_tail_flight_tick(input)
                .unwrap()
                .result()
                .unwrap();
            let SessionTerminalState::TailWaterContact(contact) = result.state else {
                panic!("expected off-grid contact: rate={rate}, penetration={penetration}");
            };
            assert_eq!(result.reason, SessionEndReason::WaterContact);
            assert_eq!(result.failure, None);
            assert!((contact.fraction() - expected_fraction).abs() < 2.0e-9);
            assert_eq!(contact.interval_start_tick(), 0);
            assert_eq!(contact.incidence(), TailIncidence::neutral());
            let time = contact.fraction() * PHYSICS_DT_SECONDS;
            assert!((result.score.unwrap().course_parallel_m() - 10.0 * time).abs() < 1.0e-14);
            let record = session.flight_record().unwrap();
            assert_eq!(record.sample_count(), 2);
            assert_eq!(record.duration_seconds(), Ok(time));
            let finalization = record.finalization().unwrap();
            assert_eq!(finalization.terminal_tick, 0);
            assert_eq!(finalization.terminal_fraction, contact.fraction());
            assert_eq!(finalization.score, result.score);
            let terminal = *record.samples().last().unwrap();
            assert_eq!(terminal.flight_state, contact.flight_state());
            assert_eq!(terminal.fraction, contact.fraction());
            assert_eq!(
                session.advance_tail_flight_tick(input),
                Err(GameSessionError::InvalidTransition)
            );
            assert_eq!(
                *session.flight_record().unwrap().samples().last().unwrap(),
                terminal
            );
        }
    }
}
