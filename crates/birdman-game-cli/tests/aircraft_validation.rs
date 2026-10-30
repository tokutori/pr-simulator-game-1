//! End-to-end admission of external aircraft documents through the CLI.

use birdman_game_format::aircraft::{AircraftDocument, AircraftSurfaceRole};
use std::{fs, io::Write, process::Command, time::SystemTime};

#[test]
fn aircraft_command_admits_only_prepared_geometry_and_a_feasible_trim() {
    let valid = AircraftDocument::decode_json(include_bytes!(
        "../../../assets/aircraft/playable-glider-v7.json"
    ))
    .unwrap();
    let mut asymmetric = valid.clone();
    asymmetric
        .surfaces
        .iter_mut()
        .find(|surface| surface.role == AircraftSurfaceRole::MainWing)
        .unwrap()
        .strip_fractions = vec![0.0, 0.3, 1.0];
    let mut untrimmable = valid.clone();
    untrimmable.trim.airspeed_mps = 1.0;
    let mut displaced_wing = valid.clone();
    let wing = displaced_wing
        .surfaces
        .iter_mut()
        .find(|surface| surface.role == AircraftSurfaceRole::MainWing)
        .unwrap();
    wing.mirror_span = false;
    for section in &mut wing.sections {
        section.quarter_chord_frd_m[1] += 30.0;
    }
    for (name, document, expected_code) in [
        ("valid", valid, 0),
        ("asymmetric-strips", asymmetric, 2),
        ("untrimmable", untrimmable, 2),
        ("wing-without-root", displaced_wing, 2),
    ] {
        document.validate().unwrap();
        let bytes = serde_json::to_vec(&document).unwrap();
        let serial = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "birdman-aircraft-{}-{serial}-{name}.json",
            std::process::id()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&bytes).unwrap();
        drop(file);
        let result = Command::new(env!("CARGO_BIN_EXE_birdman-game-cli"))
            .arg("validate-aircraft")
            .arg(&path)
            .output();
        fs::remove_file(&path).unwrap();
        let output = result.unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if expected_code == 0 {
            assert!(!output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        } else {
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
    }
}
