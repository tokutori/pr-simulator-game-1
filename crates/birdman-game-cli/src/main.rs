//! Native verification commands for deterministic flight scenarios.

use birdman_game_core::{
    ActuatorConfig, AeroCoefficients, AerodynamicElement, AerodynamicModel, AerodynamicRole,
    AircraftModel, BodyPoint, BodyRateFeedbackConfig, BodyVector, CoefficientLaw,
    CompositeCgLaunchConditions, ControlCoefficientDerivatives, ControlMode, CourseAxis,
    ElementEnvelope, ElementOrientation, ElementReference, FbwAuthority, FlightRunOutcome,
    FlightTickConfig, FlightTickInput, FlightTickState, Gravity, InertiaTensor, NedPoint,
    NedVector, PHYSICS_HZ, PilotPositionTarget, SurfaceCommands, UniformAerodynamicLoad,
    UniformAir, UnitQuaternion, WaterContactGeometry, body_rate_feedback_commands,
    flight_state_from_composite_cg_launch, run_flight,
};

const DEFAULT_TICK_LIMIT: usize = 400;

fn main() {
    if let Err(error) = run_command() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run_command() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("verify-flight") => {
            let requested_mode = arguments.next().unwrap_or_else(|| "all".to_owned());
            if arguments.next().is_some() {
                return Err(usage());
            }
            run_verification(&requested_mode)
        }
        Some("--help" | "-h") | None => {
            println!("{}", usage());
            Ok(())
        }
        Some(_) => Err(usage()),
    }
}

fn usage() -> String {
    "Usage: birdman-game-cli verify-flight [all|manual|shared|automatic]".to_owned()
}

fn run_verification(requested_mode: &str) -> Result<(), String> {
    let modes = match requested_mode {
        "all" => vec![
            ("manual", ControlMode::Manual),
            (
                "shared",
                ControlMode::Shared(FbwAuthority::try_new(0.5).map_err(display_error)?),
            ),
            ("automatic", ControlMode::Automatic),
        ],
        "manual" => vec![("manual", ControlMode::Manual)],
        "shared" => vec![(
            "shared",
            ControlMode::Shared(FbwAuthority::try_new(0.5).map_err(display_error)?),
        )],
        "automatic" => vec![("automatic", ControlMode::Automatic)],
        _ => return Err(usage()),
    };

    let scenario = VerificationScenario::new(DEFAULT_TICK_LIMIT)?;
    println!(
        "Synthetic verification scenario; aerodynamic coefficients and FBW gains are not aircraft-tuned."
    );
    println!(
        "Physics: {PHYSICS_HZ} Hz, tick limit: {}",
        scenario.pilot_inputs.len()
    );
    for (name, mode) in modes {
        let first = scenario.run(mode)?;
        let repeated = scenario.run(mode)?;
        if first != repeated {
            return Err(format!("{name} replay was not deterministic"));
        }
        print_outcome(name, first);
    }
    Ok(())
}

fn print_outcome(name: &str, outcome: FlightRunOutcome) {
    match outcome {
        FlightRunOutcome::WaterContact { sample, score } => println!(
            "{name}: WaterContact interval={} fraction={:.9} score_m={:.6} cross_track_m={:.6}",
            sample.interval_start_tick(),
            sample.fraction(),
            score.course_parallel_m(),
            score.cross_track_m(),
        ),
        FlightRunOutcome::TimeLimit { state, score } => println!(
            "{name}: TimeLimit tick={} score_m={:.6} cross_track_m={:.6} roll_actuator_rad={:.6}",
            state.tick_index(),
            score.course_parallel_m(),
            score.cross_track_m(),
            state.actuator_state().roll_rad(),
        ),
    }
}

struct VerificationScenario {
    aircraft: AircraftModel,
    initial: FlightTickState,
    pilot_inputs: Vec<(SurfaceCommands, PilotPositionTarget)>,
    loads: UniformAerodynamicLoad,
    contact_geometry: [BodyPoint; 1],
    course_axis: CourseAxis,
    feedback: BodyRateFeedbackConfig,
}

impl VerificationScenario {
    fn new(tick_limit: usize) -> Result<Self, String> {
        let actuator_limits = [ActuatorConfig::try_new(0.35, 1.0).map_err(display_error)?; 3];
        let aircraft = AircraftModel::try_new(
            30.0,
            InertiaTensor::diagonal(8.0, 10.0, 12.0).map_err(display_error)?,
            70.0,
            -0.1,
            -0.4,
            0.4,
            0.3,
            0.8,
        )
        .map_err(display_error)?;
        let launch = CompositeCgLaunchConditions::try_new(
            NedPoint::try_new(0.0, 0.0, -100.0).map_err(display_error)?,
            NedVector::try_new(15.0, 0.0, 0.0).map_err(display_error)?,
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .map_err(display_error)?;
        let initial_flight =
            flight_state_from_composite_cg_launch(&aircraft, launch).map_err(display_error)?;
        let initial = FlightTickState::try_new(
            &aircraft,
            actuator_limits,
            0,
            initial_flight,
            birdman_game_core::ActuatorState::neutral(),
        )
        .map_err(display_error)?;
        let pilot_inputs = (0..tick_limit)
            .map(|tick| {
                let roll = if tick % 80 < 40 { 0.04 } else { -0.04 };
                let pilot_commands =
                    SurfaceCommands::try_new(roll, 0.015, 0.0).map_err(display_error)?;
                let pilot_position = if tick % 160 < 80 { 0.12 } else { -0.12 };
                let target = PilotPositionTarget::try_new(&aircraft, pilot_position)
                    .map_err(display_error)?;
                Ok((pilot_commands, target))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let feedback =
            BodyRateFeedbackConfig::try_new([0.2; 3], [0.2; 3]).map_err(display_error)?;

        Ok(Self {
            aircraft,
            initial,
            pilot_inputs,
            loads: UniformAerodynamicLoad::new(
                aerodynamic_model()?,
                UniformAir::try_new(NedVector::zero(), 1.225).map_err(display_error)?,
            ),
            contact_geometry: [BodyPoint::try_new(0.0, 0.0, 0.0).map_err(display_error)?],
            course_axis: CourseAxis::try_new(1.0, 0.0).map_err(display_error)?,
            feedback,
        })
    }

    fn run(&self, mode: ControlMode) -> Result<FlightRunOutcome, String> {
        let config = FlightTickConfig::new(
            mode,
            [ActuatorConfig::try_new(0.35, 1.0).map_err(display_error)?; 3],
            Gravity::try_new(9.80665).map_err(display_error)?,
        );
        let contact_geometry =
            WaterContactGeometry::try_new(&self.contact_geometry).map_err(display_error)?;
        let mut predicted = self.initial;
        let mut inputs = Vec::with_capacity(self.pilot_inputs.len());
        for (pilot_commands, pilot_position_target) in self.pilot_inputs.iter().copied() {
            let fbw_commands = body_rate_feedback_commands(
                self.feedback,
                BodyVector::zero(),
                predicted.flight_state().angular_velocity_body(),
            )
            .map_err(display_error)?;
            let input = FlightTickInput::new(pilot_commands, fbw_commands, pilot_position_target);
            inputs.push(input);
            match birdman_game_core::advance_flight_tick_with_contact(
                &self.aircraft,
                predicted,
                config,
                input,
                &self.loads,
                contact_geometry,
            )
            .map_err(display_error)?
            {
                birdman_game_core::FlightTickOutcome::Advanced(next) => predicted = next,
                birdman_game_core::FlightTickOutcome::WaterContact(_) => break,
            }
        }
        run_flight(
            &self.aircraft,
            self.initial,
            config,
            &inputs,
            &self.loads,
            contact_geometry,
            self.course_axis,
        )
        .map_err(display_error)
    }
}

fn aerodynamic_model() -> Result<AerodynamicModel, String> {
    use AerodynamicRole::{Fuselage, HorizontalTail, LeftWing, RightWing, VerticalTail};

    let roles = [LeftWing, RightWing, HorizontalTail, VerticalTail, Fuselage];
    let envelope =
        ElementEnvelope::try_new(-0.8, 0.8, -0.8, 0.8, 0.0, 100_000.0).map_err(display_error)?;
    let reference = ElementReference::try_new(0.4, 1.0, 0.5).map_err(display_error)?;
    let zero = CoefficientLaw::try_new(0.0, 0.0, 0.0).map_err(display_error)?;
    let drag = CoefficientLaw::try_new(0.04, 0.0, 0.0).map_err(display_error)?;
    let lift = CoefficientLaw::try_new(0.15, 2.0, 0.0).map_err(display_error)?;

    let elements = roles.map(|role| {
        let (x, y, z) = match role {
            LeftWing => (0.0, -0.8, 0.0),
            RightWing => (0.0, 0.8, 0.0),
            HorizontalTail => (-1.2, 0.0, 0.1),
            VerticalTail => (-1.2, 0.0, -0.1),
            Fuselage => (0.0, 0.0, 0.0),
        };
        let point = BodyPoint::try_new(x, y, z).map_err(display_error)?;
        let control_lift = match role {
            LeftWing => [0.4, 0.0, 0.0],
            RightWing => [-0.4, 0.0, 0.0],
            HorizontalTail => [0.0, 0.3, 0.0],
            VerticalTail => [0.0, 0.0, 0.2],
            Fuselage => [0.0; 3],
        };
        let derivatives = ControlCoefficientDerivatives::try_new(
            control_lift,
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
            [0.0; 3],
        )
        .map_err(display_error)?;
        let coefficients = AeroCoefficients::new(lift, drag, zero, zero, zero, zero)
            .with_control_derivatives(derivatives);
        AerodynamicElement::try_new(
            role,
            point,
            point,
            ElementOrientation::IDENTITY,
            reference,
            coefficients,
            envelope,
        )
        .map_err(display_error)
    });
    AerodynamicModel::try_new(
        elements
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "invalid verification element set".to_owned())?,
    )
    .map_err(display_error)
}

fn display_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::VerificationScenario;
    use birdman_game_core::{ControlMode, FbwAuthority, FlightRunOutcome};

    #[test]
    fn all_control_modes_run_reproducibly_with_distinct_synthetic_inputs() {
        let scenario = VerificationScenario::new(120).unwrap();
        let modes = [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ];
        let results = modes.map(|mode| {
            let first = scenario.run(mode).unwrap();
            let repeated = scenario.run(mode).unwrap();
            assert_eq!(first, repeated);
            assert!(matches!(first, FlightRunOutcome::TimeLimit { .. }));
            first
        });
        assert_ne!(results[0], results[1]);
        assert_ne!(results[1], results[2]);
    }
}
