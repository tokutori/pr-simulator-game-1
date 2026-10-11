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
        Some("validate-environment") => {
            let path = arguments.next().ok_or_else(usage)?;
            if arguments.next().is_some() {
                return Err(usage());
            }
            validate_environment(&path)
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
    "Usage: birdman-game-cli verify-hybrid-flight [all|manual|shared|automatic]\n       birdman-game-cli validate-environment <path>".to_owned()
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

fn display_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
