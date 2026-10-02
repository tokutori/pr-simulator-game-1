//! Native verification commands for deterministic flight scenarios.

use birdman_game_core::{
    BodyVector, ControlMode, FbwAuthority, FlightFeedbackInput, FlightRunOutcome, PHYSICS_HZ,
    PilotPositionTarget, SurfaceCommands, SyntheticFlight,
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
        Some("validate-environment") => {
            let path = arguments.next().ok_or_else(usage)?;
            if arguments.next().is_some() {
                return Err(usage());
            }
            validate_environment(&path)
        }
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
    "Usage: birdman-game-cli verify-flight [all|manual|shared|automatic]\n       birdman-game-cli validate-environment <path>".to_owned()
}

fn validate_environment(path: &str) -> Result<(), String> {
    use birdman_game_format::{EnvironmentDocument, MAX_ENVIRONMENT_JSON_BYTES};
    use std::io::Read;

    let source = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    source
        .take((MAX_ENVIRONMENT_JSON_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let document = EnvironmentDocument::decode_json(&bytes).map_err(display_error)?;
    println!(
        "Valid environment version {}: {}",
        document.environment_version, document.name
    );
    Ok(())
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

    let time_limit_scenario = VerificationScenario::new(DEFAULT_TICK_LIMIT, 100.0)?;
    let contact_scenario = VerificationScenario::new(DEFAULT_TICK_LIMIT, 10.0)?;
    println!(
        "Synthetic verification scenario; aerodynamic coefficients and FBW gains are not aircraft-tuned."
    );
    println!(
        "Physics: {PHYSICS_HZ} Hz, tick limit: {}",
        time_limit_scenario.pilot_inputs.len()
    );
    for (name, mode) in modes {
        let time_limit = run_reproducibly(&time_limit_scenario, mode, name)?;
        if !matches!(time_limit, FlightRunOutcome::TimeLimit { .. }) {
            return Err(format!("{name} verification did not reach TimeLimit"));
        }
        print_outcome(&format!("{name}/time-limit"), time_limit);

        let water_contact = run_reproducibly(&contact_scenario, mode, name)?;
        if !matches!(water_contact, FlightRunOutcome::WaterContact { .. }) {
            return Err(format!("{name} verification did not reach WaterContact"));
        }
        print_outcome(&format!("{name}/water-contact"), water_contact);
    }
    Ok(())
}

fn run_reproducibly(
    scenario: &VerificationScenario,
    mode: ControlMode,
    name: &str,
) -> Result<FlightRunOutcome, String> {
    let first = scenario.run(mode)?;
    let repeated = scenario.run(mode)?;
    if first != repeated {
        return Err(format!("{name} replay was not deterministic"));
    }
    Ok(first)
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
    fixture: SyntheticFlight,
    pilot_inputs: Vec<FlightFeedbackInput>,
}

impl VerificationScenario {
    fn new(tick_limit: usize, launch_altitude_m: f64) -> Result<Self, String> {
        let fixture = SyntheticFlight::try_new(launch_altitude_m).map_err(display_error)?;
        let aircraft = fixture.aircraft();
        let pilot_inputs = (0..tick_limit)
            .map(|tick| {
                let roll = if tick % 80 < 40 { 0.04 } else { -0.04 };
                let pilot_commands =
                    SurfaceCommands::try_new(roll, 0.015, 0.0).map_err(display_error)?;
                let pilot_position = if tick % 160 < 80 { 0.12 } else { -0.12 };
                let target = PilotPositionTarget::try_new(&aircraft, pilot_position)
                    .map_err(display_error)?;
                Ok(FlightFeedbackInput::new(
                    pilot_commands,
                    BodyVector::zero(),
                    target,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            fixture,
            pilot_inputs,
        })
    }

    fn run(&self, mode: ControlMode) -> Result<FlightRunOutcome, String> {
        self.fixture
            .scenario()
            .run_feedback(mode, self.fixture.feedback(), &self.pilot_inputs)
            .map_err(display_error)
    }
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
        let scenario = VerificationScenario::new(120, 100.0).unwrap();
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

    #[test]
    fn all_control_modes_reproduce_terminal_water_contact() {
        let scenario = VerificationScenario::new(400, 10.0).unwrap();
        let modes = [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ];
        for mode in modes {
            let first = scenario.run(mode).unwrap();
            let repeated = scenario.run(mode).unwrap();
            assert_eq!(first, repeated);
            assert!(matches!(first, FlightRunOutcome::WaterContact { .. }));
        }
    }
}
