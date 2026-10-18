use birdman_game_core::{
    FlightRecord, FlightRecordControlCapture, FlightRecordControls, FlightTelemetry,
    TailFlightTickReport, TailFlightTickState,
};
use vstd::prelude::*;

verus! {
    fn no_elapsed_report_must_not_change_initial_physical_state(
        record: &mut FlightRecord,
        previous: TailFlightTickState,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
    )
        requires FlightRecordControls::from_tail_report(report) matches FlightRecordControlCapture::NoElapsedInterval,
    {
        if record.begin_tail(previous, telemetry).is_err() {
            return;
        }
        let _ = record.append_tail_report(report, telemetry);
        let ghost changed = record.recorded_samples().last().flight_state != previous.flight_state();
        assert(changed);
    }
}
