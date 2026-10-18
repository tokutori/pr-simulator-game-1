use birdman_game_core::{
    FlightRecord, FlightTelemetry, TailFlightTickOutcome, TailFlightTickReport,
};
use vstd::prelude::*;

verus! {
    fn accepted_advanced_report_must_match_latest_payload(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    ) {
        let TailFlightTickOutcome::Advanced(state) = report.outcome() else {
            return;
        };
        if record.append_tail_report(report, telemetry).is_err() {
            return;
        }
        let ghost mismatched = !record.latest_matches_tail_state(state);
        assert(mismatched);
    }
}
