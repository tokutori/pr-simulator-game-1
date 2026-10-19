#[test]
fn manual_pause_freezes_the_record_until_explicit_resume() {
    let mut current = session(100);
    current.start_countdown(1).unwrap();
    current
        .observe_countdown(current.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    current
        .advance_tail_flight_tick(neutral_input(&current))
        .unwrap();
    let previous_state = current.snapshot().tail_flight_state().unwrap();
    let previous_samples = current.flight_record().unwrap().samples().to_vec();

    current.pause(PauseReason::Manual).unwrap();
    assert!(current.can_resume());
    assert_eq!(
        current.advance_tail_flight_tick(neutral_input(&current)),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(current.snapshot().tail_flight_state(), Some(previous_state));
    assert_eq!(current.flight_record().unwrap().samples(), previous_samples);

    current.resume().unwrap();
    assert_eq!(current.snapshot().phase(), SessionPhase::FlightRunning);
    assert_eq!(current.snapshot().tail_flight_state(), Some(previous_state));
    assert_eq!(current.flight_record().unwrap().samples(), previous_samples);
    current
        .advance_tail_flight_tick(neutral_input(&current))
        .unwrap();
    assert_eq!(
        current.flight_record().unwrap().sample_count(),
        previous_samples.len() + 1
    );
}

#[test]
fn external_pause_requires_clear_and_explicit_resume_without_advancing_the_record() {
    let mut current = session(100);
    current.start_countdown(1).unwrap();
    current
        .observe_countdown(current.countdown_token().unwrap(), 1.0, true)
        .unwrap();
    current
        .advance_tail_flight_tick(neutral_input(&current))
        .unwrap();
    let previous_state = current.snapshot().tail_flight_state().unwrap();
    let previous_samples = current.flight_record().unwrap().samples().to_vec();

    current.pause(PauseReason::Manual).unwrap();
    current.pause(PauseReason::DocumentHidden).unwrap();
    assert!(!current.can_resume());
    assert_eq!(current.resume(), Err(GameSessionError::PauseConditionsRemain));
    let SessionPhase::FlightPaused { reasons } = current.snapshot().phase() else {
        panic!("an external pause must prevent resume");
    };
    assert!(!reasons.contains(PauseReason::Manual));
    assert!(reasons.contains(PauseReason::DocumentHidden));

    current.clear_pause_reason(PauseReason::DocumentHidden).unwrap();
    assert!(matches!(current.snapshot().phase(), SessionPhase::FlightPaused { .. }));
    assert!(current.can_resume());
    assert_eq!(
        current.advance_tail_flight_tick(neutral_input(&current)),
        Err(GameSessionError::InvalidTransition)
    );
    assert_eq!(current.snapshot().tail_flight_state(), Some(previous_state));
    assert_eq!(current.flight_record().unwrap().samples(), previous_samples);

    current.resume().unwrap();
    assert_eq!(current.snapshot().phase(), SessionPhase::FlightRunning);
    assert_eq!(current.snapshot().tail_flight_state(), Some(previous_state));
    assert_eq!(current.flight_record().unwrap().samples(), previous_samples);
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
    current.observe_countdown(current.countdown_token().unwrap(), 1.0, true).unwrap();
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
