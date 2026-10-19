//! Engine-independent projection of committed and recorded physical states.

use birdman_game_core::{FlightState, FlightTelemetry, TailIncidence};
use birdman_game_format::{
    FlightRenderPose, RenderTailControls, RenderTailGeometry, RenderTailIncidence,
    TailControlLayoutDocument,
};

/// Projects the same physical state and simulation time used by the owning session or record.
pub fn flight_render_pose(
    state: FlightState,
    incidence: TailIncidence,
    initial_pilot_position_m: f64,
    simulation_time_seconds: f64,
    telemetry: Option<FlightTelemetry>,
    tail_geometry: RenderTailGeometry,
) -> FlightRenderPose {
    FlightRenderPose {
        datum_position_ned: state.datum_position_ned().components().into(),
        attitude_body_to_ned: state.attitude_body_to_ned().components().into(),
        pilot_position_meters: state.pilot_position_m(),
        initial_pilot_position_meters: initial_pilot_position_m,
        simulation_time_seconds,
        airspeed_meters_per_second: telemetry.map(|value| value.airspeed_mps),
        wind_velocity_ned_meters_per_second: telemetry
            .map(|value| value.wind_velocity_ned_mps.components().into()),
        controls: RenderTailControls {
            layout: TailControlLayoutDocument::TailIncidence,
            physical_incidence: RenderTailIncidence {
                horizontal_tail_radians: incidence.elevator_rad(),
                vertical_tail_radians: incidence.rudder_rad(),
            },
        },
        tail_geometry,
    }
}
