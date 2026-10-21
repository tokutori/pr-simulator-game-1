use super::*;
use crate::{
    AeroError, AerodynamicEvaluationError, AerodynamicStage, BodyPoint, BodyVector,
    CompositeCgLaunchConditions, CourseAxis, DynamicsError, FbwAuthority, Gravity,
    HybridAerodynamicLoad, HybridError, HybridModel, HybridSite, HybridSurface, LoadError,
    NedPoint, NedVector, TailControlProfile, TailFlightScenarioParameters, TailIncidence,
    TailPilotIntent, TailPilotPositionCommand, TailRateTarget, TestAircraft, WindField,
};

const CONTACT_POINTS: [BodyPoint; 1] = [BodyPoint::origin()];

fn identity() -> SessionScenarioIdentity {
    SessionScenarioIdentity {
        catalog_version: 10,
        scenario_id: 6,
        scenario_version: 1,
        aircraft_model_version: 1,
        environment_version: 1,
        controller_profile_version: 1,
        seed: 42,
    }
}

fn configuration<'a>(
    definition: &'a TestAircraft,
    surfaces: &'a [HybridSurface<'a>],
    wind: WindField<'a>,
    cg_down: f64,
    maximum_ticks: u64,
    mode: ControlMode,
) -> GameSessionConfiguration<'a> {
    configuration_for_heading(
        definition,
        surfaces,
        wind,
        cg_down,
        maximum_ticks,
        mode,
        0.0,
    )
}

fn configuration_for_heading<'a>(
    definition: &'a TestAircraft,
    surfaces: &'a [HybridSurface<'a>],
    wind: WindField<'a>,
    cg_down: f64,
    maximum_ticks: u64,
    mode: ControlMode,
    heading_rad: f64,
) -> GameSessionConfiguration<'a> {
    let trim = definition.trim().unwrap();
    let ground = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, cg_down).unwrap(), heading_rad)
        .unwrap();
    let launch = CompositeCgLaunchConditions::try_new(
        NedPoint::try_new(0.0, 0.0, cg_down).unwrap(),
        NedVector::try_new(
            TestAircraft::TRIM_AIRSPEED_MPS * libm::cos(trim.gamma_rad()) * libm::cos(heading_rad),
            TestAircraft::TRIM_AIRSPEED_MPS * libm::cos(trim.gamma_rad()) * libm::sin(heading_rad),
            -TestAircraft::TRIM_AIRSPEED_MPS * libm::sin(trim.gamma_rad()),
        )
        .unwrap(),
        ground.attitude_body_to_ned(),
        BodyVector::zero(),
        trim.pilot_position_m(),
        0.0,
    )
    .unwrap();
    let parameters = TailFlightScenarioParameters::try_new(
        definition.aircraft(),
        launch,
        TailIncidence::neutral(),
        Gravity::try_new(TestAircraft::TRIM_GRAVITY_MPS2).unwrap(),
        &CONTACT_POINTS,
        CourseAxis::try_new(libm::cos(heading_rad), libm::sin(heading_rad)).unwrap(),
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), surfaces).unwrap(),
        TestAircraft::TRIM_AIR_DENSITY_KG_M3,
        wind,
    )
    .unwrap();
    let scenario = TailFlightScenario::try_new(
        parameters,
        load,
        TailControlProfile::try_new(0.5, 0.5, 0.8).unwrap(),
    )
    .unwrap();
    GameSessionConfiguration::try_new_tail(scenario, mode, maximum_ticks, identity()).unwrap()
}

fn launch(configuration: GameSessionConfiguration<'_>) -> GameSession<'_> {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    session
}

fn input() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

#[test]
fn flight_progress_uses_signed_sealed_course_and_datum_despite_pilot_motion() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let heading_rad = 0.37;
    let mut session = launch(configuration_for_heading(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        -10.5,
        10,
        ControlMode::Manual,
        heading_rad,
    ));
    let initial = session.snapshot().tail_flight_state().unwrap();
    let initial_cg = session
        .telemetry()
        .unwrap()
        .unwrap()
        .composite_cg_position_ned_m;
    let initial_datum = initial.flight_state().datum_position_ned();
    let [north, east, down] = initial_datum.components();
    let position = NedPoint::try_new(
        north - 4.0 * libm::cos(heading_rad) - 3.0 * libm::sin(heading_rad),
        east - 4.0 * libm::sin(heading_rad) + 3.0 * libm::cos(heading_rad),
        down,
    )
    .unwrap();
    let physical = crate::FlightState::try_new(
        position,
        initial.flight_state().datum_velocity_ned(),
        initial.flight_state().attitude_body_to_ned(),
        BodyVector::zero(),
        0.2,
        0.0,
    )
    .unwrap();
    let current = crate::TailFlightTickState::try_new(
        &definition.aircraft(),
        1,
        physical,
        initial.incidence(),
        initial.pilot_position_target(),
    )
    .unwrap();
    session.configured_mut().unwrap().phase = ConfiguredPhase::Flight(ActiveFlight {
        state: current,
        execution: FlightExecution::Running,
    });
    let progress = session.flight_progress().unwrap().unwrap();
    assert!((progress.course_parallel_m() + 4.0).abs() < 1.0e-12);
    assert!((progress.cross_track_m() - 3.0).abs() < 1.0e-12);
    assert!((progress.net_horizontal_m() - 5.0).abs() < 1.0e-12);
    let cg_progress = course_distance_score(
        initial_cg,
        session
            .telemetry()
            .unwrap()
            .unwrap()
            .composite_cg_position_ned_m,
        CourseAxis::try_new(libm::cos(heading_rad), libm::sin(heading_rad)).unwrap(),
    )
    .unwrap();
    assert!((cg_progress.course_parallel_m() - progress.course_parallel_m()).abs() > 0.01);
    assert_eq!(session.snapshot().tail_flight_state(), Some(current));
    session.pause(PauseReason::Manual).unwrap();
    assert_eq!(session.flight_progress(), Ok(Some(progress)));
    session.resume().unwrap();
    assert_eq!(session.flight_progress(), Ok(Some(progress)));
}

#[test]
fn accepted_running_ticks_retain_the_latest_record_physical_payload() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        -10.5,
        4,
        ControlMode::Manual,
    ));
    let applied = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.25, -0.125).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    for completed_ticks in 0..=3 {
        let snapshot = session.snapshot();
        assert_eq!(snapshot.phase(), SessionPhase::FlightRunning);
        let current = snapshot.tail_flight_state().unwrap();
        let record = session.flight_record().unwrap();
        let latest = record.samples().last().unwrap();
        assert_eq!(latest.tick_index, completed_ticks);
        assert_eq!(latest.fraction.to_bits(), 0.0_f64.to_bits());
        assert_eq!(latest.flight_state, current.flight_state());
        assert_eq!(latest.controls.incidence, current.incidence());
        assert_eq!(latest.controls.has_input(), completed_ticks > 0);
        if completed_ticks == 1 {
            session.pause(PauseReason::Manual).unwrap();
            session.resume().unwrap();
        }
        if completed_ticks < 3 {
            session.advance_tail_flight_tick(applied).unwrap();
        }
    }
    let snapshot = session.advance_tail_flight_tick(applied).unwrap();
    let result = snapshot.result().unwrap();
    let SessionTerminalState::TailTick(terminal) = result.state else {
        panic!("expected the configured time limit");
    };
    let latest = session.flight_record().unwrap().samples().last().unwrap();
    assert_eq!(result.reason, SessionEndReason::TimeLimit);
    assert_eq!(terminal.tick_index(), 4);
    assert_eq!(latest.flight_state, terminal.flight_state());
    assert_eq!(latest.controls.incidence, terminal.incidence());
}

#[test]
fn tail_session_shares_lifecycle_pause_retry_and_record_contract() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        -10.5,
        2,
        ControlMode::Manual,
    ));
    let initial = session.snapshot();
    let state = initial.tail_flight_state().unwrap();
    session.pause(PauseReason::TrackingSuspended).unwrap();
    assert_eq!(
        session.advance_tail_flight_tick(input()),
        Err(GameSessionError::InvalidTransition)
    );
    assert!(!session.can_resume());
    session
        .clear_pause_reason(PauseReason::TrackingSuspended)
        .unwrap();
    session.resume().unwrap();
    session.advance_tail_flight_tick(input()).unwrap();
    let result = session
        .advance_tail_flight_tick(input())
        .unwrap()
        .result()
        .unwrap();
    assert_eq!(result.reason, SessionEndReason::TimeLimit);
    assert_eq!(result.failure, None);
    assert!(matches!(result.state, SessionTerminalState::TailTick(_)));
    assert_eq!(session.flight_record().unwrap().sample_count(), 3);
    let first_run = session.flight_record().unwrap().samples().to_vec();
    session.retry().unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::BriefingReady);
    assert_eq!(session.configuration_identity(), Some(identity()));
    session.start_countdown(1).unwrap();
    session
        .observe_countdown(session.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    assert_eq!(session.snapshot().tail_flight_state(), Some(state));
    session.advance_tail_flight_tick(input()).unwrap();
    session.advance_tail_flight_tick(input()).unwrap();
    assert_eq!(
        session.flight_record().unwrap().samples(),
        first_run.as_slice()
    );
}

#[test]
fn tail_session_water_contact_record_score_and_playback_share_one_terminal_time() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let mut session = launch(configuration(
            &definition,
            &surfaces,
            WindField::uniform(NedVector::zero()),
            -1.0,
            4_000,
            mode,
        ));
        while session.snapshot().phase() == SessionPhase::FlightRunning {
            session.advance_tail_flight_tick(input()).unwrap();
        }
        let result = session.snapshot().result().unwrap();
        let SessionTerminalState::TailWaterContact(contact) = result.state else {
            panic!("expected fractional hybrid water contact");
        };
        assert_eq!(result.reason, SessionEndReason::WaterContact);
        assert!(contact.fraction() > 0.0);
        let record = session.flight_record().unwrap();
        let terminal = record.samples().last().unwrap();
        assert_eq!(terminal.flight_state, contact.flight_state());
        assert_eq!(terminal.tick_index, contact.interval_start_tick());
        assert_eq!(terminal.fraction, contact.fraction());
        assert_eq!(terminal.controls.actuators(), contact.incidence());
        assert_eq!(record.finalization().unwrap().score, result.score);
        assert_eq!(record.personal_best_candidate_score(), result.score);
        assert_eq!(
            result.score,
            Some(
                course_distance_score(
                    record.samples()[0].flight_state.datum_position_ned(),
                    contact.flight_state().datum_position_ned(),
                    CourseAxis::try_new(1.0, 0.0).unwrap(),
                )
                .unwrap()
            )
        );
        assert_eq!(session.flight_progress(), Ok(None));
        let playback = record
            .sample_at_seconds(record.duration_seconds().unwrap())
            .unwrap();
        assert_eq!(playback.flight_state, contact.flight_state());
        assert_eq!(playback.actuators, terminal.controls.actuators());
        session.enter_replay().unwrap();
        assert_eq!(session.snapshot().phase(), SessionPhase::Replay);
        session.leave_replay().unwrap();
        assert_eq!(session.snapshot().result(), Some(result));
    }
}

#[test]
fn tail_session_failed_stage_retains_last_successful_state_record_and_original_cause() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let winds = [NedVector::zero(); 8];
    let wind = WindField::grid(
        NedPoint::try_new(-5.0, -10.0, -12.0).unwrap(),
        NedVector::try_new(5.14, 20.0, 14.0).unwrap(),
        [2; 3],
        &winds,
    )
    .unwrap();
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        wind,
        -10.5,
        100,
        ControlMode::Automatic,
    ));
    session.advance_tail_flight_tick(input()).unwrap();
    let previous = session.snapshot().tail_flight_state().unwrap();
    let retained = session.flight_record().unwrap().samples().to_vec();
    let error = session.advance_tail_flight_tick(input()).unwrap_err();
    let expected = TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(
            HybridError::new(HybridSite::Datum, AeroError::Wind(WindError::OutsideGrid))
                .with_stage(AerodynamicStage::Second),
        ),
    )));
    assert_eq!(error, GameSessionError::TailTick(expected));
    let result = session.snapshot().result().unwrap();
    assert_eq!(result.reason, SessionEndReason::OutOfValidEnvelope);
    assert_eq!(result.state, SessionTerminalState::TailTick(previous));
    assert_eq!(result.failure, Some(expected));
    let record = session.flight_record().unwrap();
    assert_eq!(record.samples(), retained.as_slice());
    assert_eq!(record.finalization().unwrap().failure, result.failure);
    assert_eq!(
        record.finalization().unwrap().terminal_tick,
        previous.tick_index()
    );
    assert_eq!(record.finalization().unwrap().terminal_fraction, 0.0);
    assert!(record.personal_best_candidate_score().is_none());
    assert_eq!(
        session.advance_tail_flight_tick(input()),
        Err(GameSessionError::InvalidTransition)
    );
}

#[test]
fn tail_session_contact_at_start_does_not_record_unelapsed_input() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let trim = definition.trim().unwrap();
    let probe = trim
        .initial_state_for_ground_launch(NedPoint::origin(), 0.0)
        .unwrap();
    let cg_down = -probe.datum_position_ned().components()[2];
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        cg_down,
        100,
        ControlMode::Manual,
    ));
    let initial = session.snapshot().tail_flight_state().unwrap();
    let initial_sample = *session.flight_record().unwrap().sample(0).unwrap();
    let applied = TailFlightTickInput::new(
        TailPilotIntent::try_new(1.0, 1.0).unwrap(),
        TailRateTarget::try_new(0.2, 0.2).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    session.advance_tail_flight_tick(applied).unwrap();
    let result = session.snapshot().result().unwrap();
    let SessionTerminalState::TailWaterContact(contact) = result.state else {
        panic!("expected contact at start");
    };
    assert_eq!(contact.fraction(), 0.0);
    assert_eq!(contact.flight_state(), initial.flight_state());
    assert_eq!(contact.incidence(), initial.incidence());
    assert_eq!(
        contact.pilot_position_target(),
        initial.pilot_position_target()
    );
    let record = session.flight_record().unwrap();
    assert_eq!(record.sample_count(), 1);
    assert_eq!(record.sample(0).unwrap(), &initial_sample);
    assert_eq!(
        record.sample(0).unwrap().flight_state,
        contact.flight_state()
    );
    assert_eq!(
        record.finalization().unwrap().terminal_tick,
        contact.interval_start_tick()
    );
    assert_eq!(
        record.finalization().unwrap().terminal_fraction,
        contact.fraction()
    );
    assert_eq!(
        record.sample(0).unwrap().controls,
        crate::FlightRecordControls::initial_tail(initial)
    );
}

#[test]
fn envelope_classification_preserves_numerical_failure_distinction() {
    for (cause, reason) in [
        (
            AeroError::OutsideEnvelope,
            SessionEndReason::OutOfValidEnvelope,
        ),
        (
            AeroError::Wind(WindError::OutsideGrid),
            SessionEndReason::OutOfValidEnvelope,
        ),
        (
            AeroError::Wind(WindError::NonFinite),
            SessionEndReason::FatalSimulationError,
        ),
        (AeroError::NonFinite, SessionEndReason::FatalSimulationError),
        (
            AeroError::UndefinedFlowAngle,
            SessionEndReason::FatalSimulationError,
        ),
    ] {
        let error = TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::Hybrid(
                HybridError::new(HybridSite::Datum, cause).with_stage(AerodynamicStage::Fourth),
            ),
        )));
        assert_eq!(error.end_reason(), reason);
    }
    for (cause, reason) in [
        (
            crate::ContactError::Wind(WindError::OutsideGrid),
            SessionEndReason::OutOfValidEnvelope,
        ),
        (
            crate::ContactError::Wind(WindError::NonFinite),
            SessionEndReason::FatalSimulationError,
        ),
        (
            crate::ContactError::ShapeProjection,
            SessionEndReason::FatalSimulationError,
        ),
        (
            crate::ContactError::MissingWingBendingFactor,
            SessionEndReason::FatalSimulationError,
        ),
    ] {
        assert_eq!(TailFlightTickError::Contact(cause).end_reason(), reason);
    }
}

#[test]
fn terminal_owner_commits_record_and_result_for_score_success_and_failure() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let failure = TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(HybridError::new(
            HybridSite::Datum,
            AeroError::OutsideEnvelope,
        )),
    )));
    let score = DistanceScore::try_from_recorded(0.0, 0.0, 0.0).unwrap();
    for failure in [None, Some(failure)] {
        for score_result in [Ok(score), Err(DistanceScoreError::NonFinite)] {
            let mut session = launch(configuration(
                &definition,
                &surfaces,
                WindField::uniform(NedVector::zero()),
                -10.5,
                10,
                ControlMode::Manual,
            ));
            let state = session.snapshot().tail_flight_state().unwrap();
            let samples = session.flight_record().unwrap().samples().to_vec();
            let reason = failure.map_or(
                SessionEndReason::ManualAbort,
                TailFlightTickError::end_reason,
            );
            let outcome = session.configured_mut().unwrap().finalize_terminal(
                reason,
                SessionTerminalState::TailTick(state),
                identity(),
                failure,
                None,
                score_result,
            );
            let expected = if failure.is_some() {
                Ok(())
            } else {
                score_result.map(|_| ()).map_err(GameSessionError::Score)
            };
            assert_eq!(outcome, expected);
            let result = session.snapshot().result().unwrap();
            assert_eq!(result.state, SessionTerminalState::TailTick(state));
            assert_eq!(result.scenario, identity());
            assert_eq!(result.failure, failure);
            assert_eq!(result.score, score_result.ok());
            assert_eq!(
                result.reason,
                if score_result.is_err() && failure.is_none() {
                    SessionEndReason::FatalSimulationError
                } else {
                    reason
                }
            );
            let record = session.flight_record().unwrap();
            assert_eq!(record.samples(), samples.as_slice());
            assert_eq!(record.header().scenario, identity());
            let metadata = record.finalization().unwrap();
            assert_eq!(metadata.reason, result.reason);
            assert_eq!(metadata.failure, result.failure);
            assert_eq!(metadata.score, result.score);
            assert_eq!(metadata.terminal_tick, state.tick_index());
            assert_eq!(metadata.terminal_fraction, 0.0);
        }
    }
}

#[test]
fn terminal_record_rejection_preserves_running_and_existing_result() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        -10.5,
        10,
        ControlMode::Manual,
    ));
    let snapshot = session.snapshot();
    let state = snapshot.tail_flight_state().unwrap();
    let samples = session.flight_record().unwrap().samples().to_vec();
    let aircraft = session.configured_mut().unwrap().configuration.aircraft();
    let mismatched = TailFlightTickState::try_new(
        &aircraft,
        state.tick_index() + 1,
        state.flight_state(),
        state.incidence(),
        state.pilot_position_target(),
    )
    .unwrap();
    assert_eq!(
        session.finalize_tick(SessionEndReason::TimeLimit, mismatched),
        Err(GameSessionError::Record(
            FlightRecordError::FinalizationMismatch
        ))
    );
    assert_eq!(session.snapshot(), snapshot);
    assert_eq!(
        session.flight_record().unwrap().samples(),
        samples.as_slice()
    );
    assert_eq!(session.flight_record().unwrap().finalization(), None);
    session
        .finalize_tick(SessionEndReason::ManualAbort, state)
        .unwrap();
    let result = session.snapshot();
    let metadata = session.flight_record().unwrap().finalization();
    assert_eq!(
        session.finalize_tick(SessionEndReason::TimeLimit, state),
        Err(GameSessionError::Record(
            FlightRecordError::AlreadyFinalized
        ))
    );
    assert_eq!(session.snapshot(), result);
    assert_eq!(
        session.flight_record().unwrap().samples(),
        samples.as_slice()
    );
    assert_eq!(session.flight_record().unwrap().finalization(), metadata);
}

#[test]
fn running_owner_rejects_non_running_phases_before_evaluation() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        -10.5,
        10,
        ControlMode::Manual,
    ));
    let reject = |session: &mut GameSession<'_>| {
        let snapshot = session.snapshot();
        let samples = session
            .flight_record()
            .map(|record| record.samples().to_vec());
        let metadata = session.flight_record().map(FlightRecord::finalization);
        assert_eq!(
            session.advance_tail_flight_tick_with(
                input(),
                |_, _, _| panic!("rejected tick evaluated physics"),
                |_, _| panic!("rejected tick evaluated telemetry"),
                |_, _| panic!("rejected tick evaluated score"),
            ),
            Err(GameSessionError::InvalidTransition)
        );
        assert_eq!(session.snapshot(), snapshot);
        assert_eq!(
            session
                .flight_record()
                .map(|record| record.samples().to_vec()),
            samples
        );
        assert_eq!(
            session.flight_record().map(FlightRecord::finalization),
            metadata
        );
    };
    let mut unconfigured = GameSession::new();
    reject(&mut unconfigured);
    unconfigured.open_setup().unwrap();
    reject(&mut unconfigured);
    session.pause(PauseReason::Manual).unwrap();
    reject(&mut session);
    session.abort_flight().unwrap();
    reject(&mut session);
    session.enter_replay().unwrap();
    reject(&mut session);
    session.leave_replay().unwrap();
    session.retry().unwrap();
    reject(&mut session);
    session.start_countdown(1).unwrap();
    reject(&mut session);
}

#[test]
fn running_owner_preserves_callback_order_and_stage_failure_priority() {
    use core::cell::{Cell, RefCell};
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Stage {
        Advanced,
        Tick,
        Telemetry,
        Append,
        Limit,
    }
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    for stage in [
        Stage::Advanced,
        Stage::Tick,
        Stage::Telemetry,
        Stage::Append,
        Stage::Limit,
    ] {
        for score_fails in [false, true] {
            if stage == Stage::Advanced && score_fails {
                continue;
            }
            let maximum_ticks = if stage == Stage::Limit { 1 } else { 10 };
            let mut session = launch(configuration(
                &definition,
                &surfaces,
                WindField::uniform(NedVector::zero()),
                -10.5,
                maximum_ticks,
                ControlMode::Manual,
            ));
            let previous = session.snapshot().tail_flight_state().unwrap();
            let retained = session.flight_record().unwrap().samples().to_vec();
            let events = RefCell::new(alloc::vec::Vec::new());
            let observed = Cell::new(None);
            let tick_failure = TailFlightTickError::TickOverflow;
            let telemetry_failure = FlightTelemetryError::Wind(crate::WindError::OutsideGrid);
            let outcome = session.advance_tail_flight_tick_with(
                input(),
                |configuration, state, sampled_input| {
                    events.borrow_mut().push("evaluate");
                    assert_eq!(state, previous);
                    assert_eq!(sampled_input, input());
                    assert_eq!(configuration.identity(), identity());
                    assert_eq!(configuration.maximum_flight_ticks, maximum_ticks);
                    if stage == Stage::Tick {
                        return Err(tick_failure);
                    }
                    let report = configuration.scenario.advance_tick_with_contact_report(
                        state,
                        configuration.control_mode,
                        sampled_input,
                    )?;
                    let report = if stage == Stage::Append {
                        let crate::TailFlightTickOutcome::Advanced(next) = report.outcome() else {
                            panic!("unexpected contact in append rejection fixture");
                        };
                        configuration.scenario.advance_tick_with_contact_report(
                            next,
                            configuration.control_mode,
                            sampled_input,
                        )?
                    } else {
                        report
                    };
                    observed.set(Some(report));
                    Ok(report)
                },
                |configuration, state| {
                    events.borrow_mut().push("telemetry");
                    let expected = match observed.get().unwrap().outcome() {
                        crate::TailFlightTickOutcome::Advanced(next) => next.flight_state(),
                        crate::TailFlightTickOutcome::WaterContact(sample) => sample.flight_state(),
                    };
                    assert_eq!(state, expected);
                    if stage == Stage::Telemetry {
                        return Err(telemetry_failure);
                    }
                    configuration.telemetry(state)
                },
                |configuration, terminal| {
                    events.borrow_mut().push("score");
                    let expected = if stage == Stage::Limit {
                        let crate::TailFlightTickOutcome::Advanced(next) =
                            observed.get().unwrap().outcome()
                        else {
                            panic!("unexpected contact in time-limit fixture");
                        };
                        next
                    } else {
                        previous
                    };
                    assert_eq!(terminal, SessionTerminalState::TailTick(expected));
                    if score_fails {
                        return Err(DistanceScoreError::NonFinite);
                    }
                    course_distance_score(
                        configuration
                            .initial_state()
                            .flight_state()
                            .datum_position_ned(),
                        terminal.flight_state().datum_position_ned(),
                        configuration.course_axis(),
                    )
                },
            );
            if stage == Stage::Advanced {
                assert_eq!(outcome, Ok(session.snapshot()));
                assert_eq!(*events.borrow(), ["evaluate", "telemetry"]);
                assert_eq!(
                    session.snapshot().tail_flight_state().unwrap().tick_index(),
                    1
                );
                assert_eq!(session.flight_record().unwrap().sample_count(), 2);
                continue;
            }
            assert_eq!(
                *events.borrow(),
                if stage == Stage::Tick {
                    alloc::vec!["evaluate", "score"]
                } else {
                    alloc::vec!["evaluate", "telemetry", "score"]
                }
            );
            let expected = if stage == Stage::Tick {
                Err(GameSessionError::TailTick(tick_failure))
            } else if score_fails {
                Err(GameSessionError::Score(DistanceScoreError::NonFinite))
            } else {
                match stage {
                    Stage::Telemetry => Err(GameSessionError::Telemetry(telemetry_failure)),
                    Stage::Append => Err(GameSessionError::Record(FlightRecordError::InvalidTime)),
                    Stage::Limit => Ok(()),
                    _ => unreachable!(),
                }
            };
            assert_eq!(
                outcome.map(|snapshot| assert_eq!(snapshot, session.snapshot())),
                expected,
                "{stage:?}, score_fails={score_fails}"
            );
            let result = session.snapshot().result().unwrap();
            assert_eq!(
                result.failure,
                (stage == Stage::Tick).then_some(tick_failure)
            );
            assert_eq!(result.score.is_none(), score_fails);
            assert_eq!(
                result.reason,
                if stage == Stage::Limit && !score_fails {
                    SessionEndReason::TimeLimit
                } else {
                    SessionEndReason::FatalSimulationError
                }
            );
            let record = session.flight_record().unwrap();
            if stage == Stage::Limit {
                assert_eq!(record.sample_count(), 2);
            } else {
                assert_eq!(record.samples(), retained);
            }
            let latest = record.samples().last().unwrap();
            assert_eq!(latest.flight_state, result.state.flight_state());
            let metadata = record.finalization().unwrap();
            assert_eq!(metadata.failure, result.failure);
            assert_eq!(metadata.score, result.score);
            assert_eq!(
                (metadata.terminal_tick, metadata.terminal_fraction),
                result.state.record_time()
            );
        }
    }
}

#[test]
fn running_owner_finalization_rejection_retains_the_previous_owner() {
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let mut session = launch(configuration(
        &definition,
        &surfaces,
        WindField::uniform(NedVector::zero()),
        -10.5,
        10,
        ControlMode::Manual,
    ));
    let previous = session.snapshot().tail_flight_state().unwrap();
    session.abort_flight().unwrap();
    session.configured_mut().unwrap().phase = ConfiguredPhase::Flight(ActiveFlight {
        state: previous,
        execution: FlightExecution::Running,
    });
    let snapshot = session.snapshot();
    let samples = session.flight_record().unwrap().samples().to_vec();
    let metadata = session.flight_record().unwrap().finalization();
    assert_eq!(
        session.advance_tail_flight_tick_with(
            input(),
            |_, _, _| Err(TailFlightTickError::TickOverflow),
            |_, _| panic!("failed tick evaluated telemetry"),
            |_, _| Err(DistanceScoreError::NonFinite),
        ),
        Err(GameSessionError::Record(
            FlightRecordError::AlreadyFinalized
        ))
    );
    assert_eq!(session.snapshot(), snapshot);
    assert_eq!(session.flight_record().unwrap().samples(), samples);
    assert_eq!(session.flight_record().unwrap().finalization(), metadata);
}
