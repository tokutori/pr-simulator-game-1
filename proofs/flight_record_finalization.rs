use birdman_game_core::{
    DistanceScore, FlightRecord, FlightRecordControlCapture, FlightRecordControls, FlightRecordError,
    FlightRecordFinalization, FlightTelemetry, SessionEndReason, TailFlightTickError,
    TailFlightTickOutcome, TailFlightTickReport, TailFlightTickState,
};
use vstd::prelude::*;

verus! {
    fn verify_domain_session_initial_projection() {
        let session = birdman_game_core::GameSession::new();
        assert(session.snapshot() == birdman_game_core::SessionSnapshot::Title);
        assert(session.flight_record().is_none());
    }

    fn verify_domain_session_countdown_rejection() {
        let mut session = birdman_game_core::GameSession::new();
        let ghost before = session;
        let outcome = session.start_countdown(1);
        assert(outcome == Err(birdman_game_core::GameSessionError::InvalidTransition));
        assert(session == before);
        assert(session.snapshot() == birdman_game_core::SessionSnapshot::Title);
    }

    fn verify_domain_archived_record_playback(record: FlightRecord) {
        let mut session = birdman_game_core::GameSession::new();
        let ghost before = session;
        let ghost provided = record;
        let outcome = session.open_archived_replay(record);
        if outcome.is_ok() {
            assert(provided.finalization().is_some());
            assert(session.flight_record() == Some(&provided));
            assert(session.playback_record() == session.flight_record());
            assert(session.snapshot() == birdman_game_core::SessionSnapshot::Replay {
                scenario: provided.header().scenario,
                source: birdman_game_core::SessionReplaySource::ArchivedRecord,
            });
            let returned = session.leave_replay();
            assert(returned == Ok(()));
            assert(session.snapshot() == birdman_game_core::SessionSnapshot::Title);
            assert(session.playback_record().is_none());
        } else {
            assert(provided.finalization().is_none());
            assert(session == before);
        }
    }

    fn verify_domain_result_record_transfer<'a>(session: &mut birdman_game_core::GameSession<'a>)
        requires old(session).projection_valid(),
    {
        let ghost before = *session;
        let outcome = session.take_finalized_result_record();
        match outcome {
            Ok(record) => {
                assert(record.finalization().is_some());
                assert(before.flight_record() == Some(&record));
                assert(session.snapshot() == birdman_game_core::SessionSnapshot::Title);
                assert(session.flight_record().is_none());
                assert(session.playback_record().is_none());
            },
            Err(_) => { assert(*session == before); },
        }
    }

    fn verify_domain_demo_record_installation(record: FlightRecord)
        requires record.finalization().is_none(),
    {
        let mut session = birdman_game_core::GameSession::new();
        let ghost before = session;
        let ghost provided = record;
        let outcome = session.install_attract_record(record);
        assert(session.snapshot() == birdman_game_core::SessionSnapshot::Title);
        assert(session.flight_record().is_none());
        assert(session.playback_record().is_none());
        assert(provided.finalization().is_none());
        assert(outcome.is_err());
        assert(session == before);
        let ghost before_return = session;
        let returned = session.leave_attract();
        assert(returned == Err(birdman_game_core::GameSessionError::InvalidTransition));
        assert(session == before_return);
    }

    fn verify_domain_finalization_commit(
        record: &mut FlightRecord,
        reason: SessionEndReason,
        tick: u64,
        fraction: f64,
        score: Option<DistanceScore>,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures match outcome {
            Ok(metadata) => final(record).finalization() == Some(metadata)
                && final(record).header() == old(record).header()
                && final(record).sample_count() == old(record).sample_count()
                && final(record).finalized_from(*old(record), metadata),
            Err(_) => *final(record) == *old(record),
        },
    {
        record.finalize(reason, tick, fraction, score)
    }

    fn verify_domain_finalization_retention(
        record: &mut FlightRecord,
        reason: SessionEndReason,
        tick: u64,
        fraction: f64,
        score: Option<DistanceScore>,
        failure: Option<TailFlightTickError>,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures outcome.is_err() ==> *final(record) == *old(record),
    {
        match failure {
            Some(failure) => record.finalize_with_failure(reason, tick, fraction, score, failure),
            None => record.finalize(reason, tick, fraction, score),
        }
    }

    fn verify_domain_finalization_metadata(
        record: &mut FlightRecord,
        reason: SessionEndReason,
        tick: u64,
        fraction: f64,
        score: Option<DistanceScore>,
        failure: TailFlightTickError,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures match outcome {
            Ok(metadata) => metadata.reason == reason && metadata.terminal_tick == tick
                && metadata.terminal_fraction == fraction && metadata.score == score
                && metadata.failure == Some(failure),
            Err(_) => true,
        },
    {
        record.finalize_with_failure(reason, tick, fraction, score, failure)
    }

    fn verify_domain_record_begin(
        record: &mut FlightRecord,
        initial: TailFlightTickState,
        telemetry: FlightTelemetry,
    ) -> (outcome: Result<(), FlightRecordError>)
        ensures match outcome {
            Ok(()) => old(record).recorded_samples().len() == 0
                && initial.tick_index() == 0
                && final(record).sample_count() == 1
                && final(record).recorded_samples().last().flight_state == initial.flight_state()
                && final(record).recorded_samples().last().controls == FlightRecordControls::initial_tail(initial)
                && final(record).recorded_samples().last().telemetry == telemetry
                && final(record).latest_matches_tail_state(initial)
                && final(record).header() == old(record).header()
                && final(record).sample_limit() == old(record).sample_limit(),
            Err(_) => *final(record) == *old(record),
        },
    {
        record.begin_tail(initial, telemetry)
    }

    fn verify_domain_record_append(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    ) -> (outcome: Result<(), FlightRecordError>)
        ensures match outcome {
            Ok(()) => old(record).finalization().is_none()
                && old(record).recorded_samples().len() > 0
                && match report.outcome() {
                    TailFlightTickOutcome::Advanced(state) =>
                        old(record).recorded_samples().last().tick_index.checked_add(1) == Some(state.tick_index())
                            && state.tick_index() <= old(record).header().maximum_flight_ticks,
                    TailFlightTickOutcome::WaterContact(sample) =>
                        sample.interval_start_tick() == old(record).recorded_samples().last().tick_index
                            && sample.interval_start_tick() < old(record).header().maximum_flight_ticks,
                }
                && match FlightRecordControls::from_tail_report(report) {
                    FlightRecordControlCapture::NoElapsedInterval => *final(record) == *old(record),
                    FlightRecordControlCapture::Applied(controls) =>
                        final(record).sample_count() == old(record).sample_count() + 1
                        && final(record).sample_count() <= final(record).sample_limit()
                        && final(record).recorded_samples().drop_last() == old(record).recorded_samples()
                        && final(record).recorded_samples().last().controls == controls
                        && final(record).recorded_samples().last().telemetry == telemetry
                        && final(record).recorded_samples().last().wind_at_cg_ned_mps == telemetry.wind_velocity_ned_mps
                        && match report.outcome() {
                            TailFlightTickOutcome::Advanced(state) =>
                                final(record).latest_matches_tail_state(state)
                                    && final(record).recorded_samples().last().flight_state == state.flight_state()
                                    && final(record).recorded_samples().last().tick_index == state.tick_index(),
                            TailFlightTickOutcome::WaterContact(sample) =>
                                final(record).recorded_samples().last().flight_state == sample.flight_state()
                                    && final(record).recorded_samples().last().tick_index == sample.interval_start_tick()
                                    && final(record).recorded_samples().last().fraction == sample.fraction(),
                        },
                },
            Err(_) => *final(record) == *old(record),
        },
    {
        record.append_tail_report(report, telemetry)
    }

    fn verify_domain_record_no_elapsed(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    )
        requires FlightRecordControls::from_tail_report(report) matches FlightRecordControlCapture::NoElapsedInterval,
        ensures *final(record) == *old(record),
    {
        let _ = record.append_tail_report(report, telemetry);
    }

    fn verify_domain_contact_terminal_payload(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
        score: Option<DistanceScore>,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        ensures match outcome {
            Ok(metadata) => final(record).finalization() == Some(metadata)
                && match report.outcome() {
                    TailFlightTickOutcome::WaterContact(sample) =>
                        final(record).recorded_samples().len() > 0
                            && final(record).recorded_samples().last().flight_state == sample.flight_state()
                            && final(record).recorded_samples().last().controls.incidence == sample.incidence()
                            && final(record).recorded_samples().last().telemetry == telemetry
                            && metadata.terminal_tick == sample.interval_start_tick()
                            && metadata.terminal_fraction == sample.fraction(),
                    _ => false,
                },
            Err(_) => true,
        },
    {
        let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
            return Err(FlightRecordError::InvalidTime);
        };
        if report.applied_controls().is_none() {
            return Err(FlightRecordError::InvalidTime);
        }
        record.append_tail_report(report, telemetry)?;
        record.finalize(SessionEndReason::WaterContact, sample.interval_start_tick(), sample.fraction(), score)
    }

    fn verify_domain_no_elapsed_terminal_tick(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
        score: Option<DistanceScore>,
    ) -> (outcome: Result<FlightRecordFinalization, FlightRecordError>)
        requires FlightRecordControls::from_tail_report(report) matches FlightRecordControlCapture::NoElapsedInterval,
        ensures final(record).recorded_samples() == old(record).recorded_samples(),
            match outcome {
                Ok(metadata) => old(record).recorded_samples().len() > 0
                    && metadata.terminal_tick == old(record).recorded_samples().last().tick_index
                    && final(record).finalization() == Some(metadata),
                Err(_) => *final(record) == *old(record),
            },
    {
        record.append_tail_report(report, telemetry)?;
        let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
            return Err(FlightRecordError::InvalidTime);
        };
        record.finalize(SessionEndReason::WaterContact, sample.interval_start_tick(), sample.fraction(), score)
    }
}
