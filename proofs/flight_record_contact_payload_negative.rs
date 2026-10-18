use birdman_game_core::{
    DistanceScore, FlightRecord, FlightTelemetry, SessionEndReason, TailFlightTickOutcome,
    TailFlightTickReport,
};
use vstd::prelude::*;

verus! {
    fn accepted_contact_must_retain_terminal_payload(
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
        let ghost changed = record.recorded_samples().last().flight_state != sample.flight_state()
            || record.recorded_samples().last().controls.incidence != sample.incidence();
        assert(changed);
    }
}
