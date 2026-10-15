#[path = "../crates/birdman-game-core/src/pause_reasons.rs"]
mod pause_reasons;

mod flight_execution {
    include!("../crates/birdman-game-core/src/flight_execution.rs");

    #[verus_spec]
    fn verify_pause_execution(mut execution: FlightExecution, reason: PauseReason) {
        let before = execution;
        execution.pause(reason);
        proof! {
            assert(!execution.running());
            assert(execution.pause_bits() == (before.pause_bits() | reason.mask()));
        }
    }

    #[verus_spec]
    fn verify_clear_execution(reasons: PauseReasons, reason: PauseReason) {
        let mut execution = FlightExecution::Paused { reasons };
        proof! { assert(!execution.running()); }
        let outcome = execution.clear_pause_reason(reason);
        match outcome {
            Ok(()) => {
                proof! {
                    assert(!execution.running());
                    assert(execution.pause_bits() == (reasons.bits() & !reason.mask()));
                }
            }
            Err(_) => {
                proof! { assert(false); }
            }
        }
    }

    #[verus_spec]
    fn verify_resume_execution(reasons: PauseReasons) {
        let mut execution = FlightExecution::Paused { reasons };
        let outcome = execution.resume();
        match outcome {
            Ok(()) => {
                proof! {
                    assert(execution.running());
                    assert(reasons.bits() & !PauseReason::Manual.mask() == 0);
                }
            }
            Err(FlightExecutionError::PauseConditionsRemain) => {
                proof! {
                    assert(!execution.running());
                    assert(execution.pause_bits() == (reasons.bits() & !PauseReason::Manual.mask()));
                    assert(execution.pause_bits() != 0);
                }
            }
            Err(FlightExecutionError::NotPaused) => {
                proof! { assert(false); }
            }
        }
    }

    #[verus_spec]
    fn verify_running_rejection(reason: PauseReason) {
        let mut execution = FlightExecution::Running;
        let clear = execution.clear_pause_reason(reason);
        proof! {
            assert(clear == Err(FlightExecutionError::NotPaused));
            assert(execution.running());
        }
        let resume = execution.resume();
        proof! {
            assert(resume == Err(FlightExecutionError::NotPaused));
            assert(execution.running());
        }
    }
}
