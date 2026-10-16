use birdman_game_core::{
    FlightRecord, FlightRecordControlCapture, FlightRecordControls, FlightTelemetry,
    TailFlightTickReport,
};
use vstd::prelude::*;

verus! {
    fn no_elapsed_report_must_not_add_a_sample(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    )
        requires FlightRecordControls::from_tail_report(report) matches FlightRecordControlCapture::NoElapsedInterval,
    {
        let before = record.sample_count();
        let _ = record.append_tail_report(report, telemetry);
        let added = record.sample_count() > before;
        assert(added);
    }
}
