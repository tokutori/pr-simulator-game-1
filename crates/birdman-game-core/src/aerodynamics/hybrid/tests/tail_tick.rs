use super::envelope::aircraft;
use super::*;
use crate::{
    AerodynamicLoadProvider, AerodynamicStage, ControlMode, DynamicsError, FbwAuthority, Gravity,
    PHYSICS_DT_SECONDS, TailControlProfile, TailFlightTickConfig, TailFlightTickError,
    TailFlightTickInput, TailFlightTickOutcome, TailFlightTickState, TailPilotIntent,
    TailPilotPositionCommand, TailPilotPositionIntent, TailPilotPositionMapping, TailRateTarget,
    WaterContactGeometry, advance_tail_flight_tick, advance_tail_flight_tick_with_contact,
    advance_tail_flight_tick_with_contact_report, advance_with_surface_deflections,
    pilot_target_acceleration,
};

fn initial_state(down: f64, velocity: [f64; 3], rate: [f64; 3]) -> TailFlightTickState {
    let aircraft = aircraft();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, 0.12).unwrap();
    TailFlightTickState::try_new(
        &aircraft,
        7,
        FlightState::try_new(
            NedPoint::try_new(0.0, 0.0, down).unwrap(),
            ned(velocity),
            UnitQuaternion::IDENTITY,
            vector(rate),
            0.12,
            0.0,
        )
        .unwrap(),
        TailIncidence::neutral(),
        mapping.trim_target(),
    )
    .unwrap()
}

fn config(mode: ControlMode) -> TailFlightTickConfig {
    TailFlightTickConfig::new(
        mode,
        TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap(),
        TailPilotPositionMapping::try_new(&aircraft(), 0.12).unwrap(),
        Gravity::try_new(0.0).unwrap(),
    )
}

fn input(pilot: [f64; 2], position: TailPilotPositionCommand) -> TailFlightTickInput {
    TailFlightTickInput::new(
        TailPilotIntent::try_new(pilot[0], pilot[1]).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        position,
    )
}

fn set_position(normalized: f64) -> TailPilotPositionCommand {
    TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(normalized).unwrap())
}

#[test]
fn tail_tick_feedback_uses_previous_qr_once_and_holds_incidence_at_every_rk_stage() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let previous = initial_state(-10.0, [10.0, 0.0, 0.0], [0.0, 0.1, -0.1]);
    let next = advance_tail_flight_tick(
        &aircraft(),
        previous,
        config(ControlMode::Automatic),
        input([0.0; 2], TailPilotPositionCommand::Hold),
        &load,
    )
    .unwrap();
    let held = TailIncidence::try_new(0.01, -0.01).unwrap();
    let previous_body = previous.flight_state();
    let acceleration = pilot_target_acceleration(
        &aircraft(),
        &previous_body,
        previous.pilot_position_target(),
        PHYSICS_DT_SECONDS,
    )
    .unwrap();
    let expected = advance_with_surface_deflections(
        &aircraft(),
        &previous_body,
        acceleration,
        Gravity::try_new(0.0).unwrap(),
        &AerodynamicLoadProvider::Hybrid(&load),
        SurfaceDeflections::try_new(0.0, held.elevator_rad(), held.rudder_rad()).unwrap(),
        PHYSICS_DT_SECONDS,
    )
    .unwrap();
    assert_eq!(next.tick_index(), 8);
    assert_eq!(next.incidence(), held);
    assert_eq!(next.flight_state(), expected);
    assert_eq!(
        next.pilot_position_target(),
        previous.pilot_position_target()
    );
    assert_ne!(
        next.flight_state().angular_velocity_body(),
        previous_body.angular_velocity_body()
    );
}

#[test]
fn tail_tick_report_retains_normalized_intent_rate_targets_and_physical_command_outputs() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let previous = initial_state(-10.0, [10.0, 0.0, 0.0], [0.0, 0.1, -0.1]);
    let contacts = [BodyPoint::origin()];
    let geometry = WaterContactGeometry::try_new(&contacts).unwrap();
    let command = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.5, -0.25).unwrap(),
        TailRateTarget::try_new(0.15, -0.1).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    for (mode, targets) in [
        (ControlMode::Manual, [-0.1, 0.05]),
        (
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            [-0.055, 0.025],
        ),
        (ControlMode::Automatic, [-0.01, 0.0]),
    ] {
        let report = advance_tail_flight_tick_with_contact_report(
            &aircraft(),
            previous,
            config(mode),
            command,
            &load,
            geometry,
        )
        .unwrap();
        let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
            panic!("expected completed airborne tick");
        };
        let applied = report.applied_controls().unwrap();
        assert_eq!(applied.input(), command);
        assert_eq!(applied.input().manual_intent().nose_up(), 0.5);
        assert_eq!(applied.input().manual_intent().turn_right(), -0.25);
        assert_eq!(
            applied.input().desired_body_rate().pitch_rad_per_second(),
            0.15
        );
        assert_eq!(
            applied.input().desired_body_rate().yaw_rad_per_second(),
            -0.1
        );
        assert_eq!(
            applied.input().pilot_position_command(),
            TailPilotPositionCommand::Hold
        );
        let commands = applied.commands();
        assert_eq!(
            commands.manual_incidence_target(),
            TailIncidence::try_new(-0.1, 0.05).unwrap()
        );
        near(
            commands.fbw_incidence_target().elevator_rad(),
            -0.01,
            1.0e-16,
        );
        assert_eq!(commands.fbw_incidence_target().rudder_rad(), 0.0);
        near(
            commands.mixed_incidence_target().elevator_rad(),
            targets[0],
            1.0e-16,
        );
        near(
            commands.mixed_incidence_target().rudder_rad(),
            targets[1],
            1.0e-16,
        );
        near(next.incidence().elevator_rad(), -0.01, 1.0e-16);
        near(next.incidence().rudder_rad(), targets[1].min(0.01), 1.0e-16);
        assert_eq!(next.pilot_position_target().position_m(), 0.12);
        assert_eq!(
            report.outcome(),
            advance_tail_flight_tick_with_contact(
                &aircraft(),
                previous,
                config(mode),
                command,
                &load,
                geometry,
            )
            .unwrap()
        );
    }
}

#[test]
fn tail_tick_pilot_set_hold_and_trim_are_independent_of_surface_authority() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let previous = initial_state(-10.0, [10.0, 0.0, 0.0], [0.0; 3]);
    for normalized in [-1.0, 1.0] {
        let expected = advance_tail_flight_tick(
            &aircraft(),
            previous,
            config(ControlMode::Manual),
            input([0.0; 2], set_position(normalized)),
            &load,
        )
        .unwrap();
        for mode in [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ] {
            let next = advance_tail_flight_tick(
                &aircraft(),
                previous,
                config(mode),
                input([0.0; 2], set_position(normalized)),
                &load,
            )
            .unwrap();
            assert_eq!(next, expected);
            assert_eq!(next.pilot_position_target().position_m(), normalized * 0.4);
            assert!(normalized * (next.flight_state().pilot_position_m() - 0.12) > 0.0);
            let held = advance_tail_flight_tick(
                &aircraft(),
                next,
                config(mode),
                input([0.0; 2], TailPilotPositionCommand::Hold),
                &load,
            )
            .unwrap();
            assert_eq!(held.pilot_position_target(), next.pilot_position_target());
            let trim = advance_tail_flight_tick(
                &aircraft(),
                held,
                config(mode),
                input([0.0; 2], set_position(0.0)),
                &load,
            )
            .unwrap();
            assert_eq!(trim.pilot_position_target().position_m(), 0.12);
        }
    }
}

#[test]
fn tail_tick_load_failure_preserves_stage_cause_and_every_previous_state_field() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let winds = [NedVector::zero(); 8];
    let wind = WindField::grid(
        NedPoint::try_new(-5.0, -5.0, -11.0).unwrap(),
        ned([5.025, 10.0, 2.0]),
        [2; 3],
        &winds,
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        wind,
    )
    .unwrap();
    let previous = initial_state(-10.0, [10.0, 0.0, 0.0], [0.0; 3]);
    let before = previous;
    let error = advance_tail_flight_tick(
        &aircraft(),
        previous,
        config(ControlMode::Manual),
        input([1.0, -1.0], set_position(1.0)),
        &load,
    )
    .unwrap_err();
    let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(error),
    ))) = error
    else {
        panic!("expected original Hybrid load error");
    };
    assert_eq!(
        error.cause(),
        AeroError::Wind(crate::WindError::OutsideGrid)
    );
    assert_eq!(error.site(), HybridSite::Datum);
    assert_eq!(error.stage(), Some(AerodynamicStage::Second));
    let contacts = [BodyPoint::origin()];
    assert_eq!(
        advance_tail_flight_tick_with_contact_report(
            &aircraft(),
            previous,
            config(ControlMode::Manual),
            input([1.0, -1.0], set_position(1.0)),
            &load,
            WaterContactGeometry::try_new(&contacts).unwrap(),
        ),
        Err(TailFlightTickError::Dynamics(DynamicsError::Load(
            LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error),)
        )))
    );
    assert_eq!(previous, before);
    assert_eq!(previous.tick_index(), 7);
    assert_eq!(previous.incidence(), TailIncidence::neutral());
    assert_eq!(previous.pilot_position_target().position_m(), 0.12);
    let overflow = TailFlightTickState::try_new(
        &aircraft(),
        u64::MAX,
        previous.flight_state(),
        previous.incidence(),
        previous.pilot_position_target(),
    )
    .unwrap();
    assert_eq!(
        advance_tail_flight_tick(
            &aircraft(),
            overflow,
            config(ControlMode::Manual),
            input([0.0; 2], TailPilotPositionCommand::Hold),
            &load,
        ),
        Err(TailFlightTickError::TickOverflow)
    );
}

#[test]
fn tail_tick_contact_interpolates_body_and_pilot_at_one_time_with_interval_inputs() {
    let fixture = Fixture::new(4);
    let surfaces = fixture.surfaces();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let contacts = [BodyPoint::origin()];
    let geometry = WaterContactGeometry::try_new(&contacts).unwrap();
    let command = input([1.0, -1.0], set_position(1.0));
    for down in [-0.005, 0.0] {
        let previous = initial_state(down, [10.0, 0.0, 1.0], [0.0; 3]);
        let next = advance_tail_flight_tick(
            &aircraft(),
            previous,
            config(ControlMode::Manual),
            command,
            &load,
        )
        .unwrap();
        let outcome = advance_tail_flight_tick_with_contact(
            &aircraft(),
            previous,
            config(ControlMode::Manual),
            command,
            &load,
            geometry,
        )
        .unwrap();
        let report = advance_tail_flight_tick_with_contact_report(
            &aircraft(),
            previous,
            config(ControlMode::Manual),
            command,
            &load,
            geometry,
        )
        .unwrap();
        assert_eq!(report.outcome(), outcome);
        let TailFlightTickOutcome::WaterContact(contact) = outcome else {
            panic!("expected fractional water contact");
        };
        assert_eq!(contact.interval_start_tick(), 7);
        assert_eq!(contact.contact_point_index(), 0);
        let fraction = contact.fraction();
        let sampled = contact.flight_state();
        let initial = previous.flight_state();
        let final_state = next.flight_state();
        near(sampled.datum_position_ned().components()[2], 0.0, 1.0e-14);
        for (actual, start, end) in [
            (
                sampled.pilot_position_m(),
                initial.pilot_position_m(),
                final_state.pilot_position_m(),
            ),
            (
                sampled.pilot_velocity_mps(),
                initial.pilot_velocity_mps(),
                final_state.pilot_velocity_mps(),
            ),
        ] {
            near(actual, start + fraction * (end - start), 1.0e-15);
        }
        for (actual, start, end) in [
            (
                sampled.datum_position_ned().components(),
                initial.datum_position_ned().components(),
                final_state.datum_position_ned().components(),
            ),
            (
                sampled.datum_velocity_ned().components(),
                initial.datum_velocity_ned().components(),
                final_state.datum_velocity_ned().components(),
            ),
            (
                sampled.angular_velocity_body().components(),
                initial.angular_velocity_body().components(),
                final_state.angular_velocity_body().components(),
            ),
        ] {
            for axis in 0..3 {
                near(
                    actual[axis],
                    start[axis] + fraction * (end[axis] - start[axis]),
                    1.0e-14,
                );
            }
        }
        assert_eq!(
            sampled.attitude_body_to_ned(),
            initial
                .attitude_body_to_ned()
                .slerp(final_state.attitude_body_to_ned(), fraction)
                .unwrap()
        );
        let held = if down == 0.0 {
            assert_eq!(fraction, 0.0);
            assert_eq!(sampled, initial);
            assert_eq!(report.applied_controls(), None);
            previous
        } else {
            assert!(fraction > 0.0 && fraction < 1.0);
            let applied = report.applied_controls().unwrap();
            assert_eq!(applied.input(), command);
            assert_eq!(
                applied.commands().manual_incidence_target(),
                TailIncidence::try_new(-0.2, 0.2).unwrap()
            );
            next
        };
        assert_eq!(contact.incidence(), held.incidence());
        assert_eq!(
            contact.pilot_position_target(),
            held.pilot_position_target()
        );
    }
    let previous = initial_state(-10.0, [10.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(
        advance_tail_flight_tick_with_contact(
            &aircraft(),
            previous,
            config(ControlMode::Manual),
            command,
            &load,
            geometry,
        )
        .unwrap(),
        TailFlightTickOutcome::Advanced(
            advance_tail_flight_tick(
                &aircraft(),
                previous,
                config(ControlMode::Manual),
                command,
                &load,
            )
            .unwrap()
        )
    );
}
