use super::*;
use crate::{
    AeroError, AerodynamicEvaluationError, AerodynamicStage, BodyPoint, BodyVector,
    CompositeCgLaunchConditions, CourseAxis, DynamicsError, FbwAuthority, Gravity,
    HybridAerodynamicLoad, HybridError, HybridMockDefinition, HybridMockTrim, HybridModel,
    HybridSite, HybridSurface, LoadError, NedPoint, NedVector, TailControlProfile,
    TailFlightScenarioParameters, TailIncidence, TailPilotIntent, TailPilotPositionCommand,
    TailRateTarget, WindField,
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
    definition: &'a HybridMockDefinition,
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
    definition: &'a HybridMockDefinition,
    surfaces: &'a [HybridSurface<'a>],
    wind: WindField<'a>,
    cg_down: f64,
    maximum_ticks: u64,
    mode: ControlMode,
    heading_rad: f64,
) -> GameSessionConfiguration<'a> {
    let trim = HybridMockTrim::try_new(definition).unwrap();
    let ground = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, cg_down).unwrap(), heading_rad)
        .unwrap();
    let launch = CompositeCgLaunchConditions::try_new(
        NedPoint::try_new(0.0, 0.0, cg_down).unwrap(),
        NedVector::try_new(
            HybridMockTrim::AIRSPEED_MPS * libm::cos(trim.gamma_rad()) * libm::cos(heading_rad),
            HybridMockTrim::AIRSPEED_MPS * libm::cos(trim.gamma_rad()) * libm::sin(heading_rad),
            -HybridMockTrim::AIRSPEED_MPS * libm::sin(trim.gamma_rad()),
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
        Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        &CONTACT_POINTS,
        CourseAxis::try_new(libm::cos(heading_rad), libm::sin(heading_rad)).unwrap(),
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
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
    session.advance_countdown().unwrap();
    session.launch().unwrap();
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
    let definition = HybridMockDefinition::try_new().unwrap();
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
    session.configuration.as_mut().unwrap().set_state(current);
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
fn tail_session_shares_lifecycle_pause_retry_and_record_contract() {
    let definition = HybridMockDefinition::try_new().unwrap();
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
    session.advance_countdown().unwrap();
    session.launch().unwrap();
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
    let definition = HybridMockDefinition::try_new().unwrap();
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
    let definition = HybridMockDefinition::try_new().unwrap();
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
            HybridError::new(
                HybridSite::Proxy {
                    surface: crate::HybridSurfaceRole::MainWing,
                    index: 0,
                },
                AeroError::Wind(WindError::OutsideGrid),
            )
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
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
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
    assert_eq!(contact.incidence(), initial.incidence());
    let record = session.flight_record().unwrap();
    assert_eq!(record.sample_count(), 1);
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
}
