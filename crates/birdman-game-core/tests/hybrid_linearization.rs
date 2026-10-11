#![doc = "Host-only local linearization of the public 100 Hz hybrid tick map."]
#![cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]

use std::cell::Cell;

use birdman_game_core::{
    AerodynamicEvaluationError, AircraftModel, BodyPoint, BodyVector, ControlMode,
    ExternalLoadProvider, FlightState, Gravity, HybridAerodynamicLoad, HybridMockDefinition,
    HybridMockTrim, HybridModel, LoadError, NedPoint, NedVector, PHYSICS_DT_SECONDS,
    PilotAcceleration, TailControlProfile, TailFlightTickConfig, TailFlightTickInput,
    TailFlightTickOutcome, TailFlightTickState, TailIncidence, TailPilotIntent,
    TailPilotPositionCommand, TailRateTarget, UnitQuaternion, WaterContactGeometry, WindField,
    Wrench, advance, advance_tail_flight_tick_with_contact_report,
};
use nalgebra::{Quaternion, SMatrix, SVector, UnitQuaternion as Rotation, Vector3, linalg::Schur};

const DIMENSIONS: usize = 11;
const SCALES: [f64; DIMENSIONS] = [9.7, 9.7, 9.7, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.2, 0.2];
const DIFFERENCE_WIDTHS: [f64; 3] = [1.0 / 65536.0, 1.0 / 131072.0, 1.0 / 262144.0];
const PERTURBATION_WIDTHS: [f64; 2] = [1.0 / 4096.0, 1.0 / 8192.0];
const CHECKPOINTS: [usize; 4] = [1, 10, 25, 50];
const JACOBIAN_TOLERANCE: f64 = 3.0e-7;
const DIFFERENCE_ROUNDOFF: f64 = 2.0e-8;
const SPECTRUM_TOLERANCE: f64 = 1.0e-5;
const RESPONSE_TOLERANCE: f64 = 3.0e-3;
const RESPONSE_ROUNDOFF: f64 = 2.0e-7;
const FLOW_TOLERANCE: f64 = 3.0e-5;
const SPECTRAL_POWER_TOLERANCE: f64 = 5.0e-4;
const NEUTRAL_BAND: f64 = 1.0e-6;

type Coordinates = SVector<f64, DIMENSIONS>;
type Jacobian = SMatrix<f64, DIMENSIONS, DIMENSIONS>;

struct TickMap<'a> {
    aircraft: AircraftModel,
    initial: TailFlightTickState,
    load: HybridAerodynamicLoad<'a>,
    contacts: WaterContactGeometry<'a>,
    profile: TailControlProfile,
    config: TailFlightTickConfig,
    gravity: Gravity,
}

struct StageObserver<'a> {
    load: HybridAerodynamicLoad<'a>,
    incidence: TailIncidence,
    pilot_position_m: f64,
    evaluations: Cell<usize>,
}

impl ExternalLoadProvider for StageObserver<'_> {
    fn evaluate(
        &self,
        _aircraft: &AircraftModel,
        state: &FlightState,
    ) -> Result<Wrench, LoadError> {
        let velocity = state
            .attitude_body_to_ned()
            .ned_to_body(state.datum_velocity_ned())
            .unwrap()
            .components();
        let alpha = velocity[2].atan2(velocity[0]);
        assert!(alpha > 0.03 && alpha < 0.05, "PWL segment changed: {alpha}");
        assert!(state.datum_position_ned().components()[2] < -90.0);
        assert_eq!(state.pilot_position_m(), self.pilot_position_m);
        assert_eq!(state.pilot_velocity_mps(), 0.0);
        self.evaluations.set(self.evaluations.get() + 1);
        self.load
            .evaluate_hybrid(state, self.incidence)
            .map(|evaluation| evaluation.total_wrench())
            .map_err(|cause| LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(cause)))
    }
}

impl TickMap<'_> {
    fn step(&self, previous: TailFlightTickState) -> TailFlightTickState {
        let input = TailFlightTickInput::new(
            TailPilotIntent::try_new(0.0, 0.0).unwrap(),
            TailRateTarget::try_new(0.0, 0.0).unwrap(),
            TailPilotPositionCommand::Hold,
        );
        let report = advance_tail_flight_tick_with_contact_report(
            &self.aircraft,
            previous,
            self.config,
            input,
            &self.load,
            self.contacts,
        )
        .expect("every public RK stage must remain inside its validated envelope");
        let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
            panic!("linearization interval must precede water contact");
        };
        let target = report
            .applied_controls()
            .unwrap()
            .commands()
            .mixed_incidence_target();
        let maximum_step = self.profile.maximum_slew_rad_per_second() * PHYSICS_DT_SECONDS;
        for (command, current) in [target.elevator_rad(), target.rudder_rad()]
            .into_iter()
            .zip([
                previous.incidence().elevator_rad(),
                previous.incidence().rudder_rad(),
            ])
        {
            assert!(command.abs() < 0.1, "incidence saturation margin was lost");
            assert!(
                (command - current).abs() < maximum_step,
                "slew branch changed"
            );
        }
        assert_eq!(next.incidence(), target);
        assert_eq!(
            next.pilot_position_target(),
            self.initial.pilot_position_target()
        );
        let observer = StageObserver {
            load: self.load,
            incidence: next.incidence(),
            pilot_position_m: self.initial.flight_state().pilot_position_m(),
            evaluations: Cell::new(0),
        };
        let observed = advance(
            &self.aircraft,
            &previous.flight_state(),
            PilotAcceleration::try_new(0.0).unwrap(),
            self.gravity,
            &observer,
            PHYSICS_DT_SECONDS,
        )
        .unwrap();
        assert_eq!(observed, next.flight_state());
        assert_eq!(observer.evaluations.get(), 4);
        next
    }

    fn perturb(&self, reference: TailFlightTickState, delta: Coordinates) -> TailFlightTickState {
        let state = reference.flight_state();
        let velocity = state.datum_velocity_ned().components();
        let rate = state.angular_velocity_body().components();
        let attitude = rotation(state.attitude_body_to_ned())
            * Rotation::from_scaled_axis(Vector3::new(delta[3], delta[4], delta[5]));
        let quaternion = attitude.quaternion();
        let perturbed = FlightState::try_new(
            state.datum_position_ned(),
            NedVector::try_new(
                velocity[0] + SCALES[0] * delta[0],
                velocity[1] + SCALES[1] * delta[1],
                velocity[2] + SCALES[2] * delta[2],
            )
            .unwrap(),
            UnitQuaternion::try_new(quaternion.w, quaternion.i, quaternion.j, quaternion.k)
                .unwrap(),
            BodyVector::try_new(rate[0] + delta[6], rate[1] + delta[7], rate[2] + delta[8])
                .unwrap(),
            state.pilot_position_m(),
            state.pilot_velocity_mps(),
        )
        .unwrap();
        TailFlightTickState::try_new(
            &self.aircraft,
            reference.tick_index(),
            perturbed,
            TailIncidence::try_new(
                reference.incidence().elevator_rad() + SCALES[9] * delta[9],
                reference.incidence().rudder_rad() + SCALES[10] * delta[10],
            )
            .unwrap(),
            reference.pilot_position_target(),
        )
        .unwrap()
    }
}

fn with_map(mode: ControlMode, verify: impl FnOnce(&TickMap<'_>)) {
    let definition = HybridMockDefinition::try_new().unwrap();
    let aircraft = definition.aircraft();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let initial_flight = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -100.0).unwrap(), 0.0)
        .unwrap();
    let surfaces = definition.surfaces().unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let contact_points = [BodyPoint::origin()];
    let mapping = trim.pilot_mapping().unwrap();
    let profile = TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap();
    let gravity = Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap();
    verify(&TickMap {
        aircraft,
        initial: TailFlightTickState::try_new(
            &aircraft,
            0,
            initial_flight,
            TailIncidence::neutral(),
            mapping.trim_target(),
        )
        .unwrap(),
        load,
        contacts: WaterContactGeometry::try_new(&contact_points).unwrap(),
        profile,
        config: TailFlightTickConfig::new(mode, profile, mapping, gravity),
        gravity,
    });
}

fn rotation(attitude: UnitQuaternion) -> Rotation<f64> {
    let [scalar, first, second, third] = attitude.components();
    Rotation::from_quaternion(Quaternion::new(scalar, first, second, third))
}

fn coordinates(reference: TailFlightTickState, actual: TailFlightTickState) -> Coordinates {
    assert_eq!(reference.tick_index(), actual.tick_index());
    let reference_flight = reference.flight_state();
    let actual_flight = actual.flight_state();
    let velocity = actual_flight
        .datum_velocity_ned()
        .minus(reference_flight.datum_velocity_ned())
        .unwrap()
        .components();
    let attitude = (rotation(reference_flight.attitude_body_to_ned()).inverse()
        * rotation(actual_flight.attitude_body_to_ned()))
    .scaled_axis();
    let rates = actual_flight
        .angular_velocity_body()
        .minus(reference_flight.angular_velocity_body())
        .unwrap()
        .components();
    Coordinates::from_row_slice(&[
        velocity[0] / SCALES[0],
        velocity[1] / SCALES[1],
        velocity[2] / SCALES[2],
        attitude[0],
        attitude[1],
        attitude[2],
        rates[0],
        rates[1],
        rates[2],
        (actual.incidence().elevator_rad() - reference.incidence().elevator_rad()) / SCALES[9],
        (actual.incidence().rudder_rad() - reference.incidence().rudder_rad()) / SCALES[10],
    ])
}

fn basis(axis: usize, width: f64) -> Coordinates {
    let mut delta = Coordinates::zeros();
    delta[axis] = width;
    delta
}

fn jacobian(map: &TickMap<'_>, reference: TailFlightTickState, width: f64) -> Jacobian {
    let next_reference = map.step(reference);
    let mut derivative = Jacobian::zeros();
    for axis in 0..DIMENSIONS {
        let positive = coordinates(
            next_reference,
            map.step(map.perturb(reference, basis(axis, width))),
        );
        let negative = coordinates(
            next_reference,
            map.step(map.perturb(reference, basis(axis, -width))),
        );
        derivative.set_column(axis, &((positive - negative) / (2.0 * width)));
    }
    assert!(derivative.iter().all(|value| value.is_finite()));
    derivative
}

fn spectrum(matrix: Jacobian, label: &str) -> Vec<f64> {
    let decomposition = Schur::try_new(matrix, 64.0 * f64::EPSILON, 2000)
        .expect("bounded real Schur iteration did not converge");
    let roots = decomposition.complex_eigenvalues();
    let (orthogonal, triangular) = decomposition.unpack();
    assert!((orthogonal * triangular * orthogonal.transpose() - matrix).amax() < 2.0e-10);
    assert!((orthogonal.transpose() * orthogonal - Jacobian::identity()).amax() < 2.0e-10);
    let mut magnitudes = Vec::with_capacity(DIMENSIONS);
    for root in roots.iter() {
        assert!(root.re.is_finite() && root.im.is_finite());
        let magnitude = root.norm();
        let classification = if magnitude < NEUTRAL_BAND {
            "erased"
        } else if (magnitude - 1.0).abs() <= NEUTRAL_BAND {
            "neutral"
        } else if magnitude < 1.0 {
            "decaying"
        } else {
            "growing"
        };
        println!("{label}: lambda={root}, |lambda|={magnitude:.12}, {classification}");
        magnitudes.push(magnitude);
    }
    magnitudes.sort_by(f64::total_cmp);
    magnitudes
}

fn assert_reference(map: &TickMap<'_>, reference: TailFlightTickState) {
    let relabeled_initial = TailFlightTickState::try_new(
        &map.aircraft,
        reference.tick_index(),
        map.initial.flight_state(),
        map.initial.incidence(),
        map.initial.pilot_position_target(),
    )
    .unwrap();
    assert!(coordinates(relabeled_initial, reference).amax() < 1.0e-9);
    let initial_position = map.initial.flight_state().datum_position_ned().components();
    let initial_velocity = map.initial.flight_state().datum_velocity_ned().components();
    let elapsed = reference.tick_index() as f64 * PHYSICS_DT_SECONDS;
    for ((actual, initial), velocity) in reference
        .flight_state()
        .datum_position_ned()
        .components()
        .into_iter()
        .zip(initial_position)
        .zip(initial_velocity)
    {
        assert!((actual - initial - elapsed * velocity).abs() < 1.0e-8);
    }
}

fn response_error(map: &TickMap<'_>, derivative: Jacobian, width: f64) -> (f64, Jacobian) {
    let mut reference = map.initial;
    let mut positive =
        std::array::from_fn::<_, DIMENSIONS, _>(|axis| map.perturb(reference, basis(axis, width)));
    let mut negative =
        std::array::from_fn::<_, DIMENSIONS, _>(|axis| map.perturb(reference, basis(axis, -width)));
    let mut power = Jacobian::identity();
    let mut maximum_error = 0.0_f64;
    let mut flow_derivative = Jacobian::zeros();
    for tick in 1..=50 {
        reference = map.step(reference);
        assert_reference(map, reference);
        power = derivative * power;
        for axis in 0..DIMENSIONS {
            positive[axis] = map.step(positive[axis]);
            negative[axis] = map.step(negative[axis]);
            if CHECKPOINTS.contains(&tick) {
                let plus_response = coordinates(reference, positive[axis]) / width;
                let minus_response = coordinates(reference, negative[axis]) / -width;
                maximum_error = maximum_error
                    .max((plus_response - power.column(axis)).amax())
                    .max((minus_response - power.column(axis)).amax());
                flow_derivative.set_column(axis, &((plus_response + minus_response) * 0.5));
            }
        }
        if CHECKPOINTS.contains(&tick) {
            assert!(
                (flow_derivative - power).amax() < FLOW_TOLERANCE,
                "central nonlinear flow disagrees with J^{tick}"
            );
        }
    }
    assert!(
        maximum_error < RESPONSE_TOLERANCE,
        "response error={maximum_error}"
    );
    (maximum_error, flow_derivative)
}

#[test]
fn local_chart_and_zero_wind_translation_symmetry_preserve_the_public_map() {
    for mode in [ControlMode::Manual, ControlMode::Automatic] {
        with_map(mode, |map| {
            for axis in 0..DIMENSIONS {
                let delta = basis(axis, PERTURBATION_WIDTHS[0]);
                let actual = coordinates(map.initial, map.perturb(map.initial, delta));
                assert!((actual - delta).amax() < 1.0e-13);
            }
            let state = map.initial.flight_state();
            let position = state.datum_position_ned().components();
            let offset = [4.0, -3.0, -2.0];
            let translated = TailFlightTickState::try_new(
                &map.aircraft,
                0,
                FlightState::try_new(
                    NedPoint::try_new(
                        position[0] + offset[0],
                        position[1] + offset[1],
                        position[2] + offset[2],
                    )
                    .unwrap(),
                    state.datum_velocity_ned(),
                    state.attitude_body_to_ned(),
                    state.angular_velocity_body(),
                    state.pilot_position_m(),
                    state.pilot_velocity_mps(),
                )
                .unwrap(),
                map.initial.incidence(),
                map.initial.pilot_position_target(),
            )
            .unwrap();
            let next = map.step(map.initial);
            let translated_next = map.step(translated);
            assert!(coordinates(next, translated_next).amax() < 1.0e-13);
            for ((actual, reference), shift) in translated_next
                .flight_state()
                .datum_position_ned()
                .components()
                .into_iter()
                .zip(next.flight_state().datum_position_ned().components())
                .zip(offset)
            {
                assert!((actual - reference - shift).abs() < 1.0e-12);
            }
        });
    }
}

#[test]
fn central_difference_spectrum_and_small_perturbations_agree_for_manual_and_fbw() {
    for mode in [ControlMode::Manual, ControlMode::Automatic] {
        with_map(mode, |map| {
            let derivatives = DIFFERENCE_WIDTHS.map(|width| jacobian(map, map.initial, width));
            let coarse_change = (derivatives[1] - derivatives[0]).amax();
            let fine_change = (derivatives[2] - derivatives[1]).amax();
            assert!(coarse_change < JACOBIAN_TOLERANCE && fine_change < JACOBIAN_TOLERANCE);
            assert!(fine_change <= coarse_change + DIFFERENCE_ROUNDOFF);
            let label = format!("{mode:?} 100 Hz");
            let magnitudes = derivatives.map(|matrix| spectrum(matrix, &label));
            for neighbor in magnitudes.windows(2) {
                for (first, second) in neighbor[0].iter().zip(&neighbor[1]) {
                    assert!((first - second).abs() < SPECTRUM_TOLERANCE);
                }
            }
            let derivative = derivatives[2];
            assert!(derivative.column(9).amax() < 1.0e-13);
            assert!(derivative.column(10).amax() < 1.0e-13);
            assert_eq!(
                magnitudes[2]
                    .iter()
                    .filter(|value| **value < NEUTRAL_BAND)
                    .count(),
                2
            );
            assert!(
                magnitudes[2]
                    .iter()
                    .any(|value| (*value - 1.0).abs() <= NEUTRAL_BAND)
            );
            let (coarse_error, _) = response_error(map, derivative, PERTURBATION_WIDTHS[0]);
            let (fine_error, flow_derivative) =
                response_error(map, derivative, PERTURBATION_WIDTHS[1]);
            assert!(
                fine_error <= 0.7 * coarse_error + RESPONSE_ROUNDOFF,
                "perturbation halving failed: {coarse_error} -> {fine_error}"
            );
            let flow_magnitudes = spectrum(flow_derivative, &format!("{mode:?} 50-tick flow"));
            for (actual, multiplier) in flow_magnitudes.iter().zip(&magnitudes[2]) {
                let expected = multiplier.powi(50);
                assert!((actual - expected).abs() < SPECTRAL_POWER_TOLERANCE * (1.0 + expected));
            }
            let mut reference = map.initial;
            for _unused_tick in 0..50 {
                reference = map.step(reference);
            }
            assert!(
                (jacobian(map, reference, DIFFERENCE_WIDTHS[2]) - derivative).amax()
                    < JACOBIAN_TOLERANCE
            );
            println!(
                "{mode:?}: width changes={coarse_change:.3e}/{fine_change:.3e}, response={coarse_error:.3e}/{fine_error:.3e}"
            );
        });
    }
}
