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
    "Usage: birdman-game-cli verify-hybrid-flight [all|manual|shared|automatic]\n       birdman-game-cli validate-environment <path>\n       birdman-game-cli validate-aircraft <path>".to_owned()
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

fn display_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
