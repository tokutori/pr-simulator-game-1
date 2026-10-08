use super::*;
use birdman_game_core::{
    FbwAuthority, FlightRecordControls, GameSession, SessionEndReason, SessionPhase,
    TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand, TailPilotPositionIntent,
    TailRateTarget,
};
use birdman_game_format::{
    AssistanceLevel, DifficultySettings, InformationLevel, TailFlightRecordDocument, WeatherClass,
};

fn launch(configuration: GameSessionConfiguration<'static>) -> GameSession<'static> {
    let mut session = GameSession::new();
    session.open_setup().unwrap();
    session.prepare_flight(configuration).unwrap();
    session.mark_briefing_ready().unwrap();
    session.start_countdown(1).unwrap();
    session.advance_countdown().unwrap();
    session.launch().unwrap();
    session
}

fn neutral_input() -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    )
}

#[test]
fn cached_hybrid_owners_survive_preparation_moves_and_share_environment_telemetry() {
    let definition = cached_definition().unwrap();
    let surfaces = cached_surfaces().unwrap();
    assert!(std::ptr::eq(definition, cached_definition().unwrap()));
    assert!(std::ptr::eq(surfaces, cached_surfaces().unwrap()));
    let (configuration, control_identity) =
        HybridSessionPreparation::try_new(ControlMode::Manual, 2, 42)
            .unwrap()
            .into_parts();
    assert_eq!(
        control_identity.aircraft_configuration_id,
        HybridMockConfiguration::Playable.configuration_id()
    );
    assert_eq!(
        control_identity.controller_profile_id,
        CONTROLLER_PROFILE_ID
    );
    let identity = configuration.identity();
    assert_eq!(identity.catalog_version, CATALOG_VERSION);
    assert_eq!(identity.scenario_version, 3);
    assert_eq!(identity.environment_version, 6);
    assert_eq!(identity.aircraft_model_version, 2);
    assert_eq!(
        identity.controller_profile_version,
        CONTROLLER_PROFILE_VERSION
    );
    assert_eq!(identity.seed, 42);
    let mut session = launch(configuration);
    let initial = session.snapshot().tail_flight_state().unwrap();
    assert_eq!(initial.incidence(), TailIncidence::neutral());
    assert_eq!(
        initial.pilot_position_target().position_m(),
        HybridMockTrim::try_new(definition)
            .unwrap()
            .pilot_position_m()
    );
    let telemetry = session.telemetry().unwrap().unwrap();
    assert_eq!(
        telemetry.wind_velocity_ned_mps,
        bundled_environment()
            .unwrap()
            .wind_field()
            .unwrap()
            .velocity_at(telemetry.composite_cg_position_ned_m)
            .unwrap()
    );
    assert!((telemetry.airspeed_mps - HybridMockTrim::AIRSPEED_MPS).abs() < 1.0e-12);
    assert!((telemetry.altitude_m - 10.5).abs() < 1.0e-12);
    session.advance_tail_flight_tick(neutral_input()).unwrap();
    session.advance_tail_flight_tick(neutral_input()).unwrap();
    assert_eq!(session.snapshot().phase(), SessionPhase::Result);
    let record = session.flight_record().unwrap();
    assert!(matches!(
        record.sample(0).unwrap().controls,
        FlightRecordControls::TailIncidence { .. }
    ));
    let document = TailFlightRecordDocument::from_record(
        record,
        DifficultySettings::custom(
            InformationLevel::Full,
            AssistanceLevel::Manual,
            WeatherClass::Typical,
        ),
        control_identity,
    )
    .unwrap();
    assert_eq!(document.header.environment_version, 6);
    assert_eq!(document.header.seed, 42);
    assert_eq!(
        document.control_identity.controller_profile_id,
        CONTROLLER_PROFILE_ID
    );
    assert_eq!(document.samples.len(), 3);
}

#[test]
fn repeated_hybrid_preparation_is_deterministic_and_rejects_invalid_tick_limit() {
    let mut first = launch(
        HybridSessionPreparation::try_new(ControlMode::Automatic, 2, 7)
            .unwrap()
            .into_parts()
            .0,
    );
    let mut second = launch(
        HybridSessionPreparation::try_new(ControlMode::Automatic, 2, 7)
            .unwrap()
            .into_parts()
            .0,
    );
    for _ in 0..2 {
        assert_eq!(
            first.advance_tail_flight_tick(neutral_input()).unwrap(),
            second.advance_tail_flight_tick(neutral_input()).unwrap()
        );
    }
    assert_eq!(
        first.flight_record().unwrap().samples(),
        second.flight_record().unwrap().samples()
    );
    assert!(matches!(
        HybridSessionPreparation::try_new(ControlMode::Manual, 0, 42),
        Err(HybridSessionPreparationError::Session(
            GameSessionError::InvalidFlightTickLimit
        ))
    ));
}

#[test]
fn default_configuration_matches_public_web_selection() {
    let preparation = HybridSessionPreparation::try_default().unwrap();
    let (configuration, record_identity) = preparation.into_parts();
    let expected = HybridSessionPreparation::try_new_for_weather(
        ControlMode::Manual,
        4_000,
        (0x5f98_u64 << 32) | 0x55aa,
        WeatherClass::Typical,
    )
    .unwrap()
    .into_parts();
    assert_eq!(configuration.identity(), expected.0.identity());
    assert_eq!(record_identity, expected.1);
    let session = launch(configuration);
    let expected_session = launch(expected.0);
    assert_eq!(session.snapshot(), expected_session.snapshot());
    assert_eq!(
        session.telemetry().unwrap(),
        expected_session.telemetry().unwrap()
    );
}

#[test]
fn registered_weather_keeps_catalog_identity_and_provider_values() {
    for weather in [
        WeatherClass::Calm,
        WeatherClass::Mild,
        WeatherClass::Typical,
        WeatherClass::Challenging,
        WeatherClass::NearLimit,
    ] {
        let selection = HybridSessionPreparation::select_scenario(weather, 42).unwrap();
        let preparation =
            HybridSessionPreparation::try_new_for_weather(ControlMode::Manual, 2, 42, weather)
                .unwrap();
        let session = launch(preparation.into_parts().0);
        assert_eq!(
            session.configuration_identity(),
            Some(identity_for_selection(selection))
        );
        let telemetry = session.telemetry().unwrap().unwrap();
        let expected = if selection.environment_version == 6 {
            bundled_environment()
                .unwrap()
                .wind_field()
                .unwrap()
                .velocity_at(telemetry.composite_cg_position_ned_m)
                .unwrap()
                .components()
        } else {
            legacy_wind_for_version(selection.environment_version).unwrap()
        };
        assert_eq!(telemetry.wind_velocity_ned_mps.components(), expected);
        assert!((telemetry.airspeed_mps - HybridMockTrim::AIRSPEED_MPS).abs() < 1.0e-12);
    }
}

#[test]
fn every_hybrid_selection_launches_and_scores_along_the_shared_northwest_bearing() {
    let platform = launch_venue().unwrap().platform;
    let direction = platform.horizontal_direction_ned();
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(birdman_game_core::FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        for weather in [
            WeatherClass::Calm,
            WeatherClass::Mild,
            WeatherClass::Typical,
            WeatherClass::Challenging,
            WeatherClass::NearLimit,
        ] {
            let preparation =
                HybridSessionPreparation::try_new_for_weather(mode, 2, 42, weather).unwrap();
            let axis = preparation.course_axis();
            assert!((axis.components()[0] - direction[0]).abs() < 1.0e-15);
            assert!((axis.components()[1] - direction[1]).abs() < 1.0e-15);
            let session = launch(preparation.into_parts().0);
            let state = session
                .snapshot()
                .tail_flight_state()
                .unwrap()
                .flight_state();
            let telemetry = session.telemetry().unwrap().unwrap();
            let ground = state.datum_velocity_ned().components();
            let wind = telemetry.wind_velocity_ned_mps.components();
            let air = core::array::from_fn::<_, 3, _>(|index| ground[index] - wind[index]);
            let horizontal_speed = air[0].hypot(air[1]);
            assert!((air[0] / horizontal_speed - direction[0]).abs() < 1.0e-14);
            assert!((air[1] / horizontal_speed - direction[1]).abs() < 1.0e-14);
            assert!(air[0] > 0.0 && air[1] < 0.0);
            let forward = state
                .attitude_body_to_ned()
                .body_to_ned(BodyVector::try_new(1.0, 0.0, 0.0).unwrap())
                .unwrap()
                .components();
            let horizontal_forward = forward[0].hypot(forward[1]);
            assert!((forward[0] / horizontal_forward - direction[0]).abs() < 1.0e-14);
            assert!((forward[1] / horizontal_forward - direction[1]).abs() < 1.0e-14);
            let expected_wind = if weather == WeatherClass::Typical {
                bundled_environment()
                    .unwrap()
                    .wind_field()
                    .unwrap()
                    .velocity_at(telemetry.composite_cg_position_ned_m)
                    .unwrap()
                    .components()
            } else {
                legacy_wind_for_version(
                    session
                        .configuration_identity()
                        .unwrap()
                        .environment_version,
                )
                .unwrap()
            };
            assert_eq!(wind, expected_wind);
            let score = birdman_game_core::course_distance_score(
                NedPoint::origin(),
                NedPoint::try_new(direction[0] * 100.0, direction[1] * 100.0, 0.0).unwrap(),
                axis,
            )
            .unwrap();
            assert!((score.course_parallel_m() - 100.0).abs() < 1.0e-12);
            assert!(score.cross_track_m().abs() < 1.0e-12);
            assert!((score.net_horizontal_m() - 100.0).abs() < 1.0e-12);
            assert_eq!(
                session.configuration_identity().unwrap().scenario_version,
                3
            );
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PlayabilitySequence {
    Neutral,
    SmallNoseUp,
    SmallNoseDown,
    ShortFullNoseUp,
    PilotForward,
    PilotBackward,
    FullNoseUp,
    FullNoseDown,
    PilotForwardEndpoint,
    PilotBackwardEndpoint,
    OneSecondNoseUp,
}

impl PlayabilitySequence {
    fn input(self, tick: u64) -> TailFlightTickInput {
        let pitch = match self {
            Self::SmallNoseUp => 0.05,
            Self::SmallNoseDown => -0.05,
            Self::FullNoseUp => 1.0,
            Self::FullNoseDown => -1.0,
            Self::ShortFullNoseUp if (100..110).contains(&tick) => 1.0,
            Self::OneSecondNoseUp if (100..200).contains(&tick) => 1.0,
            _ => 0.0,
        };
        let position = match self {
            Self::PilotForward | Self::PilotBackward if (100..110).contains(&tick) => {
                let sign = if matches!(self, Self::PilotForward) {
                    1.0
                } else {
                    -1.0
                };
                TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(sign * (tick - 99) as f64 * 0.01).unwrap(),
                )
            }
            Self::PilotForwardEndpoint | Self::PilotBackwardEndpoint
                if (100..200).contains(&tick) =>
            {
                let sign = if matches!(self, Self::PilotForwardEndpoint) {
                    1.0
                } else {
                    -1.0
                };
                TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(sign * (tick - 99) as f64 * 0.01).unwrap(),
                )
            }
            _ => TailPilotPositionCommand::Hold,
        };
        TailFlightTickInput::new(
            TailPilotIntent::try_new(pitch, 0.0).unwrap(),
            TailRateTarget::try_new(pitch * TailRateTarget::limits_rad_per_second()[0], 0.0)
                .unwrap(),
            position,
        )
    }
}

#[test]
fn playable_normal_input_sequences_remain_inside_all_load_boundaries_until_natural_termination() {
    for sequence in [
        PlayabilitySequence::Neutral,
        PlayabilitySequence::SmallNoseUp,
        PlayabilitySequence::SmallNoseDown,
        PlayabilitySequence::ShortFullNoseUp,
        PlayabilitySequence::PilotForward,
        PlayabilitySequence::PilotBackward,
    ] {
        assert_playable_sequence(sequence);
    }
}

#[test]
fn playable_held_pitch_pulse_and_pilot_endpoint_requests_use_guarded_physics_and_preserved_record_inputs()
 {
    for sequence in [
        PlayabilitySequence::FullNoseUp,
        PlayabilitySequence::FullNoseDown,
        PlayabilitySequence::PilotForwardEndpoint,
        PlayabilitySequence::PilotBackwardEndpoint,
        PlayabilitySequence::OneSecondNoseUp,
    ] {
        assert_playable_sequence(sequence);
    }
}

fn assert_playable_sequence(sequence: PlayabilitySequence) {
    let definition = cached_definition().unwrap();
    let surfaces = cached_surfaces().unwrap();
    let wind = bundled_environment().unwrap().wind_field().unwrap();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        wind,
    )
    .unwrap();
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let preparation = HybridSessionPreparation::try_new_for_weather(
            mode,
            DEFAULT_MAXIMUM_FLIGHT_TICKS,
            DEFAULT_SESSION_SEED,
            DEFAULT_WEATHER,
        )
        .unwrap();
        let guard = preparation
            .controller_profile()
            .angle_of_attack_guard()
            .unwrap();
        assert_eq!(guard.alpha_interval_rad(), [-0.09, 0.09]);
        assert_eq!(
            guard.trim_alpha_rad(),
            HybridMockTrim::try_new(definition).unwrap().alpha_rad()
        );
        let mut session = launch(preparation.into_parts().0);
        for tick in 1..=DEFAULT_MAXIMUM_FLIGHT_TICKS {
            assert!(
                session
                    .advance_tail_flight_tick(sequence.input(tick))
                    .is_ok(),
                "{mode:?} {sequence:?} tick={tick} {:?}",
                session.snapshot().result()
            );
            if session.snapshot().phase() == SessionPhase::Result {
                break;
            }
        }
        let snapshot = session.snapshot();
        let result = snapshot.result().unwrap();
        assert!(
            matches!(
                result.reason,
                SessionEndReason::WaterContact | SessionEndReason::TimeLimit
            ),
            "{mode:?} {sequence:?}: {result:?}"
        );
        assert_eq!(result.failure, None);
        let record = session.flight_record().unwrap();
        assert_eq!(record.finalization().unwrap().reason, result.reason);
        assert_eq!(record.finalization().unwrap().failure, result.failure);
        assert_eq!(
            record.samples().last().unwrap().flight_state,
            result.state.flight_state()
        );
        for sample in record.samples() {
            let FlightRecordControls::TailIncidence {
                incidence,
                input_from_previous,
            } = sample.controls
            else {
                panic!("expected the public two-tail record layout");
            };
            loads
                .evaluate_hybrid(&sample.flight_state, incidence)
                .unwrap();
            let relative = sample
                .flight_state
                .datum_velocity_ned()
                .minus(
                    wind.velocity_at(sample.flight_state.datum_position_ned())
                        .unwrap(),
                )
                .unwrap();
            let velocity = sample
                .flight_state
                .attitude_body_to_ned()
                .ned_to_body(relative)
                .unwrap()
                .components();
            let alpha = velocity[2].atan2(velocity[0]);
            assert!((-0.18..=0.18).contains(&alpha));
            if let Some(input) = input_from_previous {
                let input_tick = sample.tick_index + u64::from(sample.fraction > 0.0);
                let expected = sequence.input(input_tick);
                assert_eq!(input.manual_intent(), expected.manual_intent());
                assert_eq!(input.desired_body_rate(), expected.desired_body_rate());
                assert_eq!(
                    input.pilot_position_command(),
                    expected.pilot_position_command()
                );
                assert_eq!(
                    input.manual_incidence_target(),
                    expected.manual_intent().incidence().unwrap()
                );
            }
        }
    }
}
