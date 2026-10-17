#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;
#[path = "../crates/birdman-game-core/src/flight_execution.rs"]
mod flight_execution;
#[path = "../crates/birdman-game-core/src/countdown.rs"]
mod countdown;

mod configured_phase {
    include!("../crates/birdman-game-core/src/configured_phase.rs");

    #[verus_spec]
    fn verify_result_replay_roundtrip(result: u64, clock: u32) {
        let mut phase = ConfiguredPhase::<u64, u64, u32>::Result { result };
        proof! { assert(phase.is_result()); }
        let entered = phase.enter_replay(clock);
        proof! {
            assert(entered.is_ok());
            assert(phase.is_replay());
            assert(phase.result_payload() == Some(result));
            assert(phase.clock_payload() == Some(clock));
        }
        let left = phase.leave_replay();
        proof! {
            assert(left.is_ok());
            assert(phase.is_result());
            assert(phase.result_payload() == Some(result));
        }
    }

    #[verus_spec(outcome =>
        ensures match outcome {
            Ok(()) => old(phase).is_result() && final(phase).is_replay()
                && final(phase).result_payload() == old(phase).result_payload()
                && final(phase).clock_payload() == Some(clock),
            Err(()) => !old(phase).is_result() && *final(phase) == *old(phase),
        },
    )]
    fn verify_enter_replay<State, Payload: Copy, Clock>(
        phase: &mut ConfiguredPhase<State, Payload, Clock>,
        clock: Clock,
    ) -> Result<(), ()> {
        phase.enter_replay(clock)
    }

    #[verus_spec(outcome =>
        ensures match outcome {
            Ok(()) => old(phase).is_replay() && final(phase).is_result()
                && final(phase).result_payload() == old(phase).result_payload(),
            Err(()) => !old(phase).is_replay() && *final(phase) == *old(phase),
        },
    )]
    fn verify_leave_replay<State, Payload: Copy, Clock>(
        phase: &mut ConfiguredPhase<State, Payload, Clock>,
    ) -> Result<(), ()> {
        phase.leave_replay()
    }

    #[verus_spec(outcome =>
        ensures outcome == match phase {
            ConfiguredPhase::Flight(ActiveFlight { state, execution: FlightExecution::Running }) => Some(*state),
            _ => None,
        },
    )]
    fn verify_running_admission<State: Copy, Payload, Clock>(
        phase: &ConfiguredPhase<State, Payload, Clock>,
    ) -> Option<State> {
        phase.running_state()
    }
}
