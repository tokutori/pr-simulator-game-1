//! Native verification commands for the current flight model.

mod hybrid_smoke;

fn main() {
    if let Err(error) = run_command() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run_command() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some(command @ ("validate-environment" | "validate-aircraft")) => {
            let path = arguments.next().ok_or_else(usage)?;
            if arguments.next().is_some() {
                return Err(usage());
            }
            if command == "validate-aircraft" {
                validate_aircraft(&path)
            } else {
                validate_environment(&path)
            }
        }
        Some("validate-controller") => {
            let path = arguments.next().ok_or_else(usage)?;
            let aircraft = arguments.next();
            if arguments.next().is_some() {
                return Err(usage());
            }
            validate_controller(&path, aircraft.as_deref())
        }
        Some("verify-hybrid-flight") => {
            let requested_mode = arguments.next().unwrap_or_else(|| "all".to_owned());
            if arguments.next().is_some() {
                return Err(usage());
            }
            hybrid_smoke::run_verification(&requested_mode)
        }
        Some("--help" | "-h") | None => {
            println!("{}", usage());
            Ok(())
        }
        Some(_) => Err(usage()),
    }
}

fn usage() -> String {
    "Usage: birdman-game-cli verify-hybrid-flight [all|manual|shared|automatic]\n       birdman-game-cli validate-environment <path>\n       birdman-game-cli validate-aircraft <path>\n       birdman-game-cli validate-controller <path> [aircraft-path]".to_owned()
}

fn validate_environment(path: &str) -> Result<(), String> {
    use birdman_game_format::{EnvironmentDocument, MAX_ENVIRONMENT_JSON_BYTES};

    let bytes = read_document(path, MAX_ENVIRONMENT_JSON_BYTES)?;
    let document = EnvironmentDocument::decode_json(&bytes).map_err(display_error)?;
    println!(
        "Valid environment version {}: {}",
        document.environment_version, document.name
    );
    Ok(())
}

fn validate_aircraft(path: &str) -> Result<(), String> {
    use birdman_game_content::AircraftDefinition;
    use birdman_game_format::aircraft::MAX_AIRCRAFT_JSON_BYTES;

    let bytes = read_document(path, MAX_AIRCRAFT_JSON_BYTES)?;
    let definition = AircraftDefinition::decode_json(&bytes).map_err(display_error)?;
    let document = definition.document();
    println!(
        "Valid aircraft model {}: {} (schema {})",
        document.model_version, document.configuration_id, document.schema_version
    );
    Ok(())
}

fn validate_controller(path: &str, aircraft_path: Option<&str>) -> Result<(), String> {
    use birdman_game_app::SelectedController;
    use birdman_game_format::controller::MAX_CONTROLLER_JSON_BYTES;
    let bytes = read_document(path, MAX_CONTROLLER_JSON_BYTES)?;
    let selected = SelectedController::decode(bytes).map_err(display_error)?;
    if let Some(path) = aircraft_path {
        let bytes = read_document(path, birdman_game_format::aircraft::MAX_AIRCRAFT_JSON_BYTES)?;
        let aircraft =
            birdman_game_content::AircraftDefinition::decode_json(&bytes).map_err(display_error)?;
        selected.prepare(&aircraft).map_err(|cause| {
            format!(
                "Controller document valid; aircraft pair {path}: {}",
                display_error(cause)
            )
        })?;
        println!("Valid aircraft/controller pair: {path}");
    }
    let controller = selected.document();
    println!(
        "Valid controller {} (schema {})",
        controller.controller_id, controller.schema_version
    );
    Ok(())
}

fn read_document(path: &str, maximum_bytes: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let source = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    source
        .take((maximum_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn display_error(error: impl core::error::Error) -> String {
    birdman_game_app::technical_error_text(&error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_command_uses_independent_diagnostics_and_the_explicit_aircraft_pair() {
        let directory =
            std::env::temp_dir().join(format!("birdman-controller-cli-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let controller_path = directory.join("controller.json");
        let aircraft_path = directory.join("aircraft.json");
        std::fs::write(
            &aircraft_path,
            [
                b"\xef\xbb\xbf".as_slice(),
                birdman_game_content::PLAYABLE_AIRCRAFT_JSON,
            ]
            .concat(),
        )
        .unwrap();
        let mut source: serde_json::Value =
            serde_json::from_slice(birdman_game_content::DEFAULT_CONTROLLER_JSON).unwrap();
        // Intrinsically valid authoring data can conflict with this aircraft's mechanical stops.
        source["incidence_bounds_rad"][0] = serde_json::json!([-0.3, 0.3]);
        std::fs::write(
            &controller_path,
            [
                b"\xef\xbb\xbf".as_slice(),
                &serde_json::to_vec(&source).unwrap(),
            ]
            .concat(),
        )
        .unwrap();
        let controller = controller_path.to_str().unwrap();
        assert!(validate_controller(controller, None).is_ok());
        let pair =
            validate_controller(controller, Some(aircraft_path.to_str().unwrap())).unwrap_err();
        assert!(pair.contains("Controller document valid"));
        assert!(pair.contains("/incidence_bounds_rad/0"));
        assert!(pair.contains("/actuator/incidence_bounds_rad"));
        source["pitch"]["rate"]["kp"] = serde_json::json!(-1.0);
        source["shared_authority"] = serde_json::json!(1.5);
        std::fs::write(&controller_path, serde_json::to_vec(&source).unwrap()).unwrap();
        let independent = validate_controller(controller, None).unwrap_err();
        assert!(independent.contains("/pitch/rate/kp"));
        assert!(independent.contains("/shared_authority"));
        std::fs::remove_file(&controller_path).unwrap();
        std::fs::remove_file(&aircraft_path).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }
}
