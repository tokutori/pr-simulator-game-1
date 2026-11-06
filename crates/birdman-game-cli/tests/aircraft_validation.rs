//! End-to-end admission of external aircraft documents through the CLI.

use birdman_game_content::ShapeError;
use birdman_game_core::{HybridSurfaceRole, SteadyGlideError};
use birdman_game_format::aircraft::{AircraftDocument, AircraftSurfaceRole};
use std::{fs, io::Write, process::Command, time::SystemTime};

#[test]
fn aircraft_command_admits_only_prepared_geometry_and_a_feasible_trim() {
    let valid = AircraftDocument::decode_json(include_bytes!(
        "../../../assets/aircraft/playable-glider-v7.json"
    ))
    .unwrap();
    let wing_index = valid
        .surfaces
        .iter()
        .position(|surface| surface.role == AircraftSurfaceRole::MainWing)
        .unwrap();
    let mut asymmetric = valid.clone();
    asymmetric.surfaces[wing_index].strip_fractions = vec![0.0, 0.3, 1.0];
    let mut untrimmable = valid.clone();
    untrimmable.trim.airspeed_mps = 1.0;
    let mut displaced_wing = valid.clone();
    let wing = &mut displaced_wing.surfaces[wing_index];
    wing.mirror_span = false;
    for section in &mut wing.sections {
        section.quarter_chord_frd_m[1] += 30.0;
    }
    for (name, document, expected_code, expected_reason) in [
        ("valid", valid, 0, None),
        (
            "asymmetric-strips",
            asymmetric,
            2,
            Some(format!("/surfaces/{wing_index}/strip_fractions")),
        ),
        (
            "untrimmable",
            untrimmable,
            2,
            Some(SteadyGlideError::NotBracketed.to_string()),
        ),
        (
            "wing-without-root",
            displaced_wing,
            2,
            Some(ShapeError::OutsideSurface(HybridSurfaceRole::MainWing).to_string()),
        ),
    ] {
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
            let reason = expected_reason.unwrap();
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(&reason),
                "{name}: expected reason {reason:?}, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
