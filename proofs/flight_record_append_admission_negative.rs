use birdman_game_core::{FlightRecord, FlightTelemetry, TailFlightTickReport};
use vstd::prelude::*;

verus! {
    fn finalized_record_must_not_append(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    )
        requires old(record).finalization().is_some(),
    {
        let appended = record.append_tail_report(report, telemetry).is_ok();
        assert(appended);
    }
}
