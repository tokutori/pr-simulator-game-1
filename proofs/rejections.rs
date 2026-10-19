use birdman_game_core::{
    DistanceScore, FlightRecord, FlightTelemetry, GameSession, SessionEndReason, SessionSnapshot,
    TailFlightTickOutcome, TailFlightTickReport,
};
use vstd::prelude::*;

verus! {
    fn verified_public_contract_is_available() {
        let session = GameSession::new();
        assert(session.snapshot() == SessionSnapshot::Title);
    }

    fn an_initial_session_cannot_be_running() {
        let session = GameSession::new();
        let initial_running = matches!(session.snapshot(), SessionSnapshot::TailFlightRunning { .. });
        assert(initial_running);
    }

    fn failed_finalization_cannot_change_the_record(record: &mut FlightRecord) {
        let ghost before = *record;
        let outcome = record.finalize(SessionEndReason::TimeLimit, 0, 0.0, None);
        if outcome.is_err() {
            let ghost changed_on_failure = *record != before;
            assert(changed_on_failure);
        }
    }

    fn contact_append_and_finalization_preserve_the_payload(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
        score: Option<DistanceScore>,
    ) {
        let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else {
            return;
        };
        if report.applied_controls().is_none() || record.append_tail_report(report, telemetry).is_err() {
            return;
        }
        if record.finalize(SessionEndReason::WaterContact, sample.interval_start_tick(), sample.fraction(), score).is_err() {
            return;
        }
        let ghost changed_at_contact = record.recorded_samples().last().flight_state != sample.flight_state()
            || record.recorded_samples().last().controls.incidence != sample.incidence();
        assert(changed_at_contact);
    }
}
