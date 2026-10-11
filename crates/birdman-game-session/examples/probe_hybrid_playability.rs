#![doc = "Deterministic public synthetic-hybrid input diagnostics with separate datum and CG alpha."]

use birdman_game_core::{
    ControlMode, FbwAuthority, FlightRecordSample, FlightState, GameSession, SessionPhase,
    SessionTerminalState, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionIntent, TailRateTarget, WindField,
};
use birdman_game_session::{
    DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, DEFAULT_WEATHER, HybridSessionPreparation,
    bundled_environment, initialize_bundled_environment,
};

#[derive(Clone, Copy, Debug)]
enum InputSequence {
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
    ContinuousPilotForwardEndpoint,
    ContinuousPilotBackwardEndpoint,
}

impl InputSequence {
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
            Self::PilotForwardEndpoint
            | Self::PilotBackwardEndpoint
            | Self::ContinuousPilotForwardEndpoint
            | Self::ContinuousPilotBackwardEndpoint
                if (100..200).contains(&tick) =>
            {
                let sign = if matches!(
                    self,
                    Self::PilotForwardEndpoint | Self::ContinuousPilotForwardEndpoint
                ) {
                    1.0
                } else {
                    -1.0
                };
                TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(sign * (tick - 99) as f64 * 0.01).unwrap(),
                )
            }
            Self::ContinuousPilotForwardEndpoint | Self::ContinuousPilotBackwardEndpoint
                if tick >= 200 =>
            {
                let normalized = if matches!(self, Self::ContinuousPilotForwardEndpoint) {
                    1.0
                } else {
                    -1.0
                };
                TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(normalized).unwrap())
            }
            _ => TailPilotPositionCommand::Hold,
        };
        let limits = TailRateTarget::limits_rad_per_second();
        TailFlightTickInput::new(
            TailPilotIntent::try_new(pitch, 0.0).unwrap(),
            TailRateTarget::try_new(pitch * limits[0], 0.0).unwrap(),
            position,
        )
    }
}

fn datum_alpha(state: FlightState, wind: WindField<'_>) -> Option<f64> {
    let relative_velocity = state
        .datum_velocity_ned()
        .minus(wind.velocity_at(state.datum_position_ned()).unwrap())
        .unwrap();
    let velocity = state
        .attitude_body_to_ned()
        .ned_to_body(relative_velocity)
        .unwrap()
        .components();
    (velocity[0] != 0.0 || velocity[2] != 0.0).then(|| velocity[2].atan2(velocity[0]))
}

fn print_sample(
    mode: &str,
    sequence: InputSequence,
    sample: &FlightRecordSample,
    wind: WindField<'_>,
) {
    let incidence = sample.controls.actuators();
    let datum_alpha = datum_alpha(sample.flight_state, wind);
    println!(
        "sample,{mode},{sequence:?},{},{:.9},{datum_alpha:?},{:?},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:?}",
        sample.tick_index,
        sample.fraction,
        sample.telemetry.angle_of_attack_rad,
        sample.flight_state.angular_velocity_body().components()[1],
        sample.telemetry.airspeed_mps,
        sample.telemetry.altitude_m,
        incidence.elevator_rad(),
        incidence.rudder_rad(),
        sample.flight_state.pilot_position_m(),
        sample.controls.pilot_position_target_m(),
    );
}

fn main() {
    initialize_bundled_environment().unwrap();
    let wind = bundled_environment().unwrap().wind_field().unwrap();
    println!(
        "kind,mode,sequence,tick,fraction,datum_alpha_rad,cg_alpha_rad,q_rad_per_second,cg_airspeed_mps,cg_altitude_m,elevator_rad,rudder_rad,pilot_position_m,held_target_m"
    );
    for (mode, label) in [
        (ControlMode::Manual, "Manual"),
        (
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            "Shared",
        ),
        (ControlMode::Automatic, "Automatic"),
    ] {
        for sequence in [
            InputSequence::Neutral,
            InputSequence::SmallNoseUp,
            InputSequence::SmallNoseDown,
            InputSequence::ShortFullNoseUp,
            InputSequence::PilotForward,
            InputSequence::PilotBackward,
            InputSequence::FullNoseUp,
            InputSequence::FullNoseDown,
            InputSequence::PilotForwardEndpoint,
            InputSequence::PilotBackwardEndpoint,
            InputSequence::OneSecondNoseUp,
            InputSequence::ContinuousPilotForwardEndpoint,
            InputSequence::ContinuousPilotBackwardEndpoint,
        ] {
            let (configuration, _) = HybridSessionPreparation::try_new_for_weather(
                mode,
                DEFAULT_MAXIMUM_FLIGHT_TICKS,
                DEFAULT_SESSION_SEED,
                DEFAULT_WEATHER,
            )
            .unwrap()
            .into_parts();
            let identity = configuration.identity();
            assert_eq!(identity.environment_version, 6);
            let mut session = GameSession::new();
            session.open_setup().unwrap();
            session.prepare_flight(configuration).unwrap();
            session.mark_briefing_ready().unwrap();
            session.start_countdown(1).unwrap();
            session.advance_countdown().unwrap();
            let initial = session.launch().unwrap().tail_flight_state().unwrap();
            let initial_alpha = datum_alpha(initial.flight_state(), wind);
            println!(
                "initial,{label},{sequence:?},identity={identity:?},datum_alpha={initial_alpha:?},cg_alpha={:?}",
                session.telemetry().unwrap().unwrap().angle_of_attack_rad
            );
            let mut last_valid = initial;
            let mut returned_error = None;
            for tick in 1..=DEFAULT_MAXIMUM_FLIGHT_TICKS {
                returned_error = session.advance_tail_flight_tick(sequence.input(tick)).err();
                if session.snapshot().phase() == SessionPhase::Result {
                    break;
                }
                assert!(returned_error.is_none());
                last_valid = session.snapshot().tail_flight_state().unwrap();
                if tick % 100 == 0 {
                    print_sample(
                        label,
                        sequence,
                        session.flight_record().unwrap().samples().last().unwrap(),
                        wind,
                    );
                }
            }
            let result = session.snapshot().result().unwrap();
            let record = session.flight_record().unwrap();
            let finalization = record.finalization().unwrap();
            let last_sample = record.samples().last().unwrap();
            assert_eq!(finalization.reason, result.reason);
            assert_eq!(finalization.failure, result.failure);
            assert_eq!(finalization.score, result.score);
            assert_eq!(last_sample.flight_state, result.state.flight_state());
            if result.failure.is_some() {
                assert_eq!(result.state, SessionTerminalState::TailTick(last_valid));
                assert!(returned_error.is_some());
            }
            let mut minimum_alpha = f64::INFINITY;
            let mut maximum_alpha = f64::NEG_INFINITY;
            for sample in record.samples() {
                if let Some(alpha) = datum_alpha(sample.flight_state, wind) {
                    minimum_alpha = minimum_alpha.min(alpha);
                    maximum_alpha = maximum_alpha.max(alpha);
                }
            }
            println!(
                "terminal,{label},{sequence:?},time_seconds={:.9},reason={:?},distance_m={:?},datum_alpha_range=[{minimum_alpha:.9},{maximum_alpha:.9}],last_valid_integer_tick={},failure={:?},returned_error={returned_error:?}",
                (finalization.terminal_tick as f64 + finalization.terminal_fraction)
                    / f64::from(birdman_game_core::PHYSICS_HZ),
                result.reason,
                result.score.map(|score| score.course_parallel_m()),
                last_valid.tick_index(),
                result.failure,
            );
            for sample in &record.samples()[record.sample_count().saturating_sub(50)..] {
                print_sample(label, sequence, sample, wind);
            }
        }
    }
}
