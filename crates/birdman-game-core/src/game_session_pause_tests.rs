#[test]
fn every_pause_set_preserves_other_causes_and_requires_explicit_resume() {
    let causes = [
        PauseReason::Manual,
        PauseReason::DocumentHidden,
        PauseReason::TrackingSuspended,
        PauseReason::ProcessingDelay,
    ];
    for selected_bits in 0u8..16 {
        let mut current = session(100);
        current.start_countdown(1).unwrap();
        current.advance_countdown().unwrap();
        current.launch().unwrap();
        current
            .advance_flight_tick(neutral_input(&current))
            .unwrap();
        let previous_state = current.snapshot().flight_state().unwrap();
        let previous_count = current.flight_record().unwrap().sample_count();
        for (index, cause) in causes.into_iter().enumerate() {
            if selected_bits & (1u8 << index) != 0 {
                current.pause(cause).unwrap();
                let paused = current.snapshot();
                current.pause(cause).unwrap();
                assert_eq!(current.snapshot(), paused);
            }
        }
        if selected_bits == 0 {
            assert_eq!(current.snapshot().phase(), SessionPhase::FlightRunning);
            assert!(!current.can_resume());
            assert_eq!(current.resume(), Err(GameSessionError::NotPaused));
            continue;
        }
        let SessionPhase::FlightPaused { reasons } = current.snapshot().phase() else {
            panic!("a nonempty pause set must keep the flight paused");
        };
        for (index, cause) in causes.into_iter().enumerate() {
            assert_eq!(reasons.contains(cause), selected_bits & (1u8 << index) != 0);
        }
        assert_eq!(current.snapshot().flight_state(), Some(previous_state));
        assert_eq!(current.flight_record().unwrap().sample_count(), previous_count);
        assert_eq!(
            current.advance_flight_tick(neutral_input(&current)),
            Err(GameSessionError::InvalidTransition)
        );
        let mut remaining_external = selected_bits & 14;
        assert_eq!(current.can_resume(), remaining_external == 0);
        if remaining_external == 0 {
            current.resume().unwrap();
        } else {
            assert_eq!(current.resume(), Err(GameSessionError::PauseConditionsRemain));
            let SessionPhase::FlightPaused { reasons } = current.snapshot().phase() else {
                panic!("unresolved external causes must keep the flight paused");
            };
            assert!(!reasons.contains(PauseReason::Manual));
            for (index, cause) in causes.into_iter().enumerate().skip(1) {
                assert_eq!(reasons.contains(cause), remaining_external & (1u8 << index) != 0);
            }
            for (index, cause) in causes.into_iter().enumerate().skip(1) {
                current.clear_pause_reason(cause).unwrap();
                let after_clear = current.snapshot();
                current.clear_pause_reason(cause).unwrap();
                assert_eq!(current.snapshot(), after_clear);
                remaining_external &= !(1u8 << index);
                assert!(matches!(current.snapshot().phase(), SessionPhase::FlightPaused { .. }));
                assert_eq!(current.can_resume(), remaining_external == 0);
                assert_eq!(current.snapshot().flight_state(), Some(previous_state));
                assert_eq!(current.flight_record().unwrap().sample_count(), previous_count);
                if remaining_external != 0 {
                    assert_eq!(current.resume(), Err(GameSessionError::PauseConditionsRemain));
                }
            }
            current.resume().unwrap();
        }
        assert_eq!(current.snapshot().phase(), SessionPhase::FlightRunning);
        assert_eq!(current.snapshot().flight_state(), Some(previous_state));
        assert_eq!(current.flight_record().unwrap().sample_count(), previous_count);
    }
}

#[test]
fn resume_rejects_nonpaused_phases_and_never_clears_terminal_record() {
    let mut current = GameSession::new();
    assert!(!current.can_resume());
    assert_eq!(current.resume(), Err(GameSessionError::NotPaused));
    current.open_setup().unwrap();
    assert!(!current.can_resume());
    assert_eq!(current.resume(), Err(GameSessionError::NotPaused));
    current.prepare_flight(configuration(100)).unwrap();
    current.mark_briefing_ready().unwrap();
    assert!(!current.can_resume());
    assert_eq!(current.resume(), Err(GameSessionError::NotPaused));
    current.start_countdown(1).unwrap();
    assert!(!current.can_resume());
    assert_eq!(current.resume(), Err(GameSessionError::NotPaused));
    current.advance_countdown().unwrap();
    current.launch().unwrap();
    current.pause(PauseReason::ProcessingDelay).unwrap();
    let terminal = current.abort_flight().unwrap();
    let previous_count = current.flight_record().unwrap().sample_count();
    let finalization = current.flight_record().unwrap().finalization();
    assert!(!current.can_resume());
    assert_eq!(current.resume(), Err(GameSessionError::NotPaused));
    assert_eq!(current.snapshot(), terminal);
    assert_eq!(current.flight_record().unwrap().sample_count(), previous_count);
    assert_eq!(current.flight_record().unwrap().finalization(), finalization);
}
