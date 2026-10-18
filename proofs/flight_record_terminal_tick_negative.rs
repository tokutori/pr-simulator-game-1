use birdman_game_core::{
    DistanceScore, FlightRecord, FlightRecordControlCapture, FlightRecordControls, FlightTelemetry,
    SessionEndReason, TailFlightTickOutcome, TailFlightTickReport,
};
use vstd::prelude::*;

verus! {
    fn reject_shifted_no_elapsed_terminal_tick(
        record: &mut FlightRecord,
        report: TailFlightTickReport,
        telemetry: FlightTelemetry,
        score: Option<DistanceScore>,
    )
        requires FlightRecordControls::from_tail_report(report) matches FlightRecordControlCapture::NoElapsedInterval,
    {
        let ghost before = *record;
        if record.append_tail_report(report, telemetry).is_err() { return; }
        let TailFlightTickOutcome::WaterContact(sample) = report.outcome() else { return; };
        if let Ok(metadata) = record.finalize(
            SessionEndReason::WaterContact, sample.interval_start_tick(), sample.fraction(), score,
        ) {
            let ghost shifted = metadata.terminal_tick != before.recorded_samples().last().tick_index;
            assert(shifted);
        }
    }
}
