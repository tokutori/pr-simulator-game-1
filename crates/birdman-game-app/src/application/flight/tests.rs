use super::*;
use crate::application::{ApplicationError, BackendReadiness, SessionCommand};
use crate::flight_input::FlightControl;
use birdman_game_core::{
    ControlMode, CountdownProgress, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionIntent, TailRateTarget,
};
use birdman_game_format::WeatherClass;

fn execute(application: &mut ApplicationSession, command: SessionCommand) {
    application
        .execute(command, BackendReadiness::Ready)
        .unwrap();
}

fn launch(maximum_ticks: u64) -> ApplicationSession {
    let mut application = ApplicationSession::new(ControlMode::Manual, maximum_ticks, 42);
    execute(&mut application, SessionCommand::OpenSetup {});
    execute(
        &mut application,
        SessionCommand::SelectWeather {
            weather: WeatherClass::Calm,
        },
    );
    execute(&mut application, SessionCommand::Prepare {});
    execute(&mut application, SessionCommand::StartCountdown {});
    let token = application.core_session().countdown_token().unwrap();
    assert_eq!(
        application.observe_countdown(token, 3.0, true),
        Ok(CountdownProgress::Launched)
    );
    application
}

fn key(application: &mut ApplicationSession, control: FlightControl, pressed: bool, repeat: bool) {
    application.observe_flight_key(FlightKeyObservation {
        control,
        pressed,
        repeat,
    });
}

fn tick_count(report: FlightFrameReport) -> u32 {
    match report.advance {
        FlightFrameAdvance::Ticks { count, .. } => count,
        other => panic!("expected admitted ticks, got {other:?}"),
    }
}

#[test]
fn frame_ticks_integrate_against_each_confirmed_target_and_keep_the_last_render_interval() {
    let mut framed = launch(100);
    let mut direct = launch(100);
    key(&mut framed, FlightControl::PilotIncrease, true, false);
    assert_eq!(
        tick_count(framed.advance_flight_frame(1000.0, &[], true).unwrap()),
        0
    );
    let report = framed.advance_flight_frame(1035.0, &[], true).unwrap();
    assert_eq!(tick_count(report), 3);
    assert_eq!(report.previous_tick.unwrap().tick_index(), 2);
    assert!((framed.flight_interpolation_fraction() - 0.5).abs() < 1e-12);
    for _tick in 0..3 {
        let before = direct
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap();
        let held = direct
            .core_session()
            .tail_pilot_position_mapping()
            .unwrap()
            .normalized_target(before.pilot_position_target())
            .unwrap()
            .value();
        direct
            .advance_tail_flight_tick(TailFlightTickInput::new(
                TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                TailRateTarget::try_new(0.0, 0.0).unwrap(),
                TailPilotPositionCommand::Set(
                    TailPilotPositionIntent::try_new(held + 0.01).unwrap(),
                ),
            ))
            .unwrap();
    }
    assert_eq!(
        framed.core_session().snapshot(),
        direct.core_session().snapshot()
    );
    assert_eq!(
        framed.core_session().flight_record().unwrap().samples(),
        direct.core_session().flight_record().unwrap().samples()
    );
}

#[test]
fn pause_resume_between_frames_retires_time_and_requires_a_fresh_key_press() {
    let mut application = launch(100);
    key(&mut application, FlightControl::PilotIncrease, true, false);
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    application.advance_flight_frame(1015.0, &[], true).unwrap();
    let held = application
        .core_session()
        .snapshot()
        .tail_flight_state()
        .unwrap()
        .pilot_position_target();
    execute(&mut application, SessionCommand::Pause {});
    key(&mut application, FlightControl::PilotIncrease, true, true);
    execute(&mut application, SessionCommand::Resume {});
    key(&mut application, FlightControl::PilotIncrease, true, true);
    assert_eq!(
        tick_count(application.advance_flight_frame(6000.0, &[], true).unwrap()),
        0
    );
    assert_eq!(
        tick_count(application.advance_flight_frame(6010.0, &[], true).unwrap()),
        1
    );
    assert_eq!(
        application
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap()
            .pilot_position_target(),
        held
    );
    key(&mut application, FlightControl::PilotIncrease, false, false);
    key(&mut application, FlightControl::PilotIncrease, true, false);
    application.advance_flight_frame(6020.0, &[], true).unwrap();
    assert!(
        application
            .core_session()
            .snapshot()
            .tail_flight_state()
            .unwrap()
            .pilot_position_target()
            .position_m()
            > held.position_m()
    );
}

#[test]
fn processing_delay_recovery_clears_only_its_cause_and_explicit_resume_restarts_at_zero() {
    let mut application = launch(100);
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    assert_eq!(
        tick_count(application.advance_flight_frame(1150.0, &[], true).unwrap()),
        15
    );
    let retained = application
        .core_session()
        .flight_record()
        .unwrap()
        .samples()
        .to_vec();
    assert_eq!(
        application
            .advance_flight_frame(1300.001, &[], true)
            .unwrap()
            .advance,
        FlightFrameAdvance::ProcessingDelay { blocked: true }
    );
    assert!(!application.core_session().can_resume());
    assert_eq!(
        application.execute(SessionCommand::Resume {}, BackendReadiness::Ready),
        Err(ApplicationError::Session(
            GameSessionError::PauseConditionsRemain
        ))
    );
    application
        .observe_availability(ExternalPauseReason::DocumentHidden, false)
        .unwrap();
    assert_eq!(
        application
            .advance_flight_frame(1310.001, &[], false)
            .unwrap()
            .advance,
        FlightFrameAdvance::ProcessingDelay { blocked: false }
    );
    assert!(!application.core_session().can_resume());
    application
        .observe_availability(ExternalPauseReason::DocumentHidden, true)
        .unwrap();
    assert!(application.core_session().can_resume());
    application
        .advance_flight_frame(1320.001, &[], true)
        .unwrap();
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .samples(),
        retained
    );
    execute(&mut application, SessionCommand::Resume {});
    assert_eq!(
        tick_count(application.advance_flight_frame(9000.0, &[], true).unwrap()),
        0
    );
    assert_eq!(
        tick_count(application.advance_flight_frame(9010.0, &[], true).unwrap()),
        1
    );
}

#[test]
fn terminal_tick_retires_the_rest_of_a_multi_tick_budget() {
    let mut application = launch(2);
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    let report = application
        .advance_flight_frame(1149.999, &[], true)
        .unwrap();
    assert_eq!(tick_count(report), 2);
    assert_eq!(report.previous_tick.unwrap().tick_index(), 1);
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::Result
    );
    assert_eq!(application.flight_interpolation_fraction(), 0.0);
    let retained = application
        .core_session()
        .flight_record()
        .unwrap()
        .samples()
        .to_vec();
    assert_eq!(
        tick_count(
            application
                .advance_flight_frame(1159.999, &[], true)
                .unwrap()
        ),
        0
    );
    execute(&mut application, SessionCommand::EnterReplay {});
    assert_eq!(
        tick_count(
            application
                .advance_flight_frame(1169.999, &[], true)
                .unwrap()
        ),
        0
    );
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .samples(),
        retained
    );
}

#[test]
fn driver_failure_retains_the_original_cause_until_a_new_flight_is_prepared() {
    let mut application = launch(100);
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    let retained = application
        .core_session()
        .flight_record()
        .unwrap()
        .samples()
        .to_vec();
    let error = FlightFrameError::Clock(FlightClockError::NonMonotonicTimestamp);
    assert_eq!(
        application.advance_flight_frame(999.0, &[], true),
        Err(error)
    );
    assert_eq!(
        application.advance_flight_frame(1010.0, &[], true),
        Err(error)
    );
    assert_eq!(
        application.execute(SessionCommand::Resume {}, BackendReadiness::Ready),
        Err(ApplicationError::Flight(error))
    );
    assert_eq!(application.flight_failure(), Some(error));
    assert!(!application.can_resume());
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .samples(),
        retained
    );
    execute(&mut application, SessionCommand::Abort {});
    assert_eq!(application.flight_failure(), Some(error));
    assert_eq!(
        tick_count(application.advance_flight_frame(1020.0, &[], true).unwrap()),
        0
    );
    execute(&mut application, SessionCommand::EnterReplay {});
    assert_eq!(application.flight_failure(), Some(error));
    assert_eq!(
        tick_count(application.advance_flight_frame(1030.0, &[], true).unwrap()),
        0
    );
    execute(&mut application, SessionCommand::LeaveReplay {});
    execute(&mut application, SessionCommand::Retry {});
    assert_eq!(application.flight_failure(), None);
    assert_eq!(
        tick_count(application.advance_flight_frame(1040.0, &[], true).unwrap()),
        0
    );
}

#[test]
fn menus_observe_frame_freshness_without_consuming_live_flight_ticks() {
    let mut application = ApplicationSession::default();
    assert!(!application.flight_frame_available_at(1000.0).unwrap());
    assert_eq!(
        tick_count(application.advance_flight_frame(1000.0, &[], true).unwrap()),
        0
    );
    assert!(application.flight_frame_available_at(1150.0).unwrap());
    assert!(!application.flight_frame_available_at(1150.001).unwrap());
    assert_eq!(
        tick_count(application.advance_flight_frame(1100.0, &[], true).unwrap()),
        0
    );
    assert_eq!(application.flight_interpolation_fraction(), 0.0);
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::Title
    );
}

#[test]
fn unavailable_frames_cancel_countdown_and_pause_live_flight_until_explicit_resume() {
    let mut application = launch(100);
    key(&mut application, FlightControl::NoseUp, true, false);
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    let retained = application
        .core_session()
        .flight_record()
        .unwrap()
        .samples()
        .to_vec();
    let report = application
        .advance_flight_frame(1010.0, &[], false)
        .unwrap();
    assert!(report.phase_changed);
    assert_eq!(tick_count(report), 0);
    assert!(!application.can_resume());
    assert!(
        application
            .advance_flight_frame(1020.0, &[], true)
            .unwrap()
            .phase_changed
    );
    assert!(application.can_resume());
    assert_eq!(
        application
            .core_session()
            .flight_record()
            .unwrap()
            .samples(),
        retained
    );
    execute(&mut application, SessionCommand::Resume {});
    assert_eq!(
        tick_count(application.advance_flight_frame(9000.0, &[], true).unwrap()),
        0
    );
    assert_eq!(
        tick_count(application.advance_flight_frame(9010.0, &[], true).unwrap()),
        1
    );
    execute(&mut application, SessionCommand::Abort {});
    execute(&mut application, SessionCommand::Retry {});
    execute(&mut application, SessionCommand::StartCountdown {});
    let token = application.core_session().countdown_token().unwrap();
    assert!(
        application
            .advance_flight_frame(9020.0, &[], false)
            .unwrap()
            .phase_changed
    );
    assert_eq!(
        application.core_session().snapshot().phase(),
        SessionPhase::BriefingReady
    );
    assert_eq!(
        application.observe_countdown(token, 3.0, true),
        Ok(CountdownProgress::Ignored)
    );
}

#[test]
fn fatal_adapter_failure_retires_admission_and_preserves_an_earlier_driver_cause() {
    let mut application = launch(100);
    application.advance_flight_frame(1000.0, &[], true).unwrap();
    application.fail_flight_adapter();
    assert_eq!(
        application.flight_failure(),
        Some(FlightFrameError::AdapterUnavailable)
    );
    assert!(!application.can_resume());
    assert_eq!(
        application.advance_flight_frame(1010.0, &[], true),
        Err(FlightFrameError::AdapterUnavailable)
    );
    execute(&mut application, SessionCommand::Abort {});
    execute(&mut application, SessionCommand::Retry {});
    execute(&mut application, SessionCommand::StartCountdown {});
    let token = application.core_session().countdown_token().unwrap();
    application.observe_countdown(token, 3.0, true).unwrap();
    application.advance_flight_frame(2000.0, &[], true).unwrap();
    let original = FlightFrameError::Clock(FlightClockError::NonMonotonicTimestamp);
    assert_eq!(
        application.advance_flight_frame(1999.0, &[], true),
        Err(original)
    );
    application.fail_flight_adapter();
    assert_eq!(application.flight_failure(), Some(original));
}
