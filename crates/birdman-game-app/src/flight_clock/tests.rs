use super::*;

fn advance(clock: &mut FlightFrameClock, timestamp_ms: f64) -> FlightFrameAdvance {
    clock.advance_frame(timestamp_ms, || true).unwrap()
}

#[test]
fn first_and_resumed_frames_start_at_zero_and_keep_a_common_remainder() {
    let mut clock = FlightFrameClock::default();
    assert_eq!(
        advance(&mut clock, 100.0),
        FlightFrameAdvance::Ticks {
            count: 0,
            interpolation_fraction: 0.0,
        }
    );
    assert_eq!(
        advance(&mut clock, 125.0),
        FlightFrameAdvance::Ticks {
            count: 2,
            interpolation_fraction: 0.5,
        }
    );
    clock.suspend();
    assert_eq!(
        advance(&mut clock, 200.0),
        FlightFrameAdvance::Ticks {
            count: 0,
            interpolation_fraction: 0.0,
        }
    );
    clock.resume();
    assert_eq!(
        advance(&mut clock, 10_000.0),
        FlightFrameAdvance::Ticks {
            count: 0,
            interpolation_fraction: 0.0,
        }
    );
    assert_eq!(
        advance(&mut clock, 10_010.0),
        FlightFrameAdvance::Ticks {
            count: 1,
            interpolation_fraction: 0.0,
        }
    );
}

#[test]
fn the_frame_gap_boundary_and_recovery_retire_time_before_explicit_resume() {
    let maximum = f64::from(crate::MAXIMUM_FLIGHT_FRAME_GAP_MILLISECONDS);
    for gap in [maximum - 0.001, maximum, maximum + 0.001] {
        let mut clock = FlightFrameClock::default();
        advance(&mut clock, 0.0);
        let mut callbacks = 0;
        let outcome = clock
            .advance_frame(gap, || {
                callbacks += 1;
                true
            })
            .unwrap();
        if gap <= maximum {
            assert_eq!(callbacks, if gap == maximum { 15 } else { 14 });
            assert!(matches!(outcome, FlightFrameAdvance::Ticks { .. }));
        } else {
            assert_eq!(callbacks, 0);
            assert_eq!(
                outcome,
                FlightFrameAdvance::ProcessingDelay { blocked: true }
            );
            assert!(!clock.resume());
            assert_eq!(
                advance(&mut clock, gap + 10.0),
                FlightFrameAdvance::ProcessingDelay { blocked: false }
            );
            assert_eq!(
                advance(&mut clock, gap + 20.0),
                FlightFrameAdvance::Ticks {
                    count: 0,
                    interpolation_fraction: 0.0,
                }
            );
            clock.resume();
            advance(&mut clock, gap + 30.0);
            assert!(matches!(
                advance(&mut clock, gap + 40.0),
                FlightFrameAdvance::Ticks { count: 1, .. }
            ));
        }
    }
}

#[test]
fn a_stopped_tick_retires_the_rest_of_a_multi_tick_frame() {
    let mut clock = FlightFrameClock::default();
    advance(&mut clock, 0.0);
    let mut attempts = 0;
    assert_eq!(
        clock
            .advance_frame(50.0, || {
                attempts += 1;
                attempts < 2
            })
            .unwrap(),
        FlightFrameAdvance::Ticks {
            count: 2,
            interpolation_fraction: 0.0,
        }
    );
    assert_eq!(attempts, 2);
    assert!(matches!(
        clock
            .advance_frame(60.0, || {
                attempts += 1;
                true
            })
            .unwrap(),
        FlightFrameAdvance::Ticks { count: 0, .. }
    ));
    assert_eq!(attempts, 2);
    clock.resume();
    advance(&mut clock, 100.0);
    assert!(matches!(
        advance(&mut clock, 110.0),
        FlightFrameAdvance::Ticks { count: 1, .. }
    ));
}

#[test]
fn invalid_timestamps_preserve_the_last_observation_and_reset_starts_a_new_clock() {
    let mut clock = FlightFrameClock::default();
    advance(&mut clock, 20.0);
    for timestamp in [f64::NAN, f64::INFINITY, -1.0, 19.0] {
        let expected = if timestamp == 19.0 {
            FlightClockError::NonMonotonicTimestamp
        } else {
            FlightClockError::InvalidTimestamp
        };
        assert_eq!(clock.advance_frame(timestamp, || true), Err(expected));
    }
    assert!(matches!(
        advance(&mut clock, 30.0),
        FlightFrameAdvance::Ticks { count: 1, .. }
    ));
    clock.reset();
    assert_eq!(
        advance(&mut clock, 0.0),
        FlightFrameAdvance::Ticks {
            count: 0,
            interpolation_fraction: 0.0,
        }
    );
}
