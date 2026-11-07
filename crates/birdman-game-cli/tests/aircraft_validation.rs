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
    let mut legacy = serde_json::to_value(&valid).unwrap();
    legacy["schema_version"] = 4.into();
    legacy["mass"]
        .as_object_mut()
        .unwrap()
        .remove("airframe_center_of_mass_frd_m");
    for surface in legacy["surfaces"].as_array_mut().unwrap() {
        surface["actuator"] = serde_json::Value::Null;
        for section in surface["sections"].as_array_mut().unwrap() {
            section.as_object_mut().unwrap().remove("twist_rad");
        }
    }
    let mut invalid_authoring = valid.clone();
    invalid_authoring.mass.airframe_center_of_mass_frd_m = [1000.0, 0.0, 0.0];
    invalid_authoring.surfaces[wing_index].sections[0].twist_rad = 4.0;
    let tail_index = valid
        .surfaces
        .iter()
        .position(|s| s.role == AircraftSurfaceRole::HorizontalTail)
        .unwrap();
    invalid_authoring.surfaces[tail_index].actuator = None;
    for (name, document, expected_code, expected_reasons) in [
        ("valid", serde_json::to_value(valid).unwrap(), 0, vec![]),
        (
            "asymmetric-strips",
            serde_json::to_value(asymmetric).unwrap(),
            2,
            vec![format!("/surfaces/{wing_index}/strip_fractions")],
        ),
        (
            "untrimmable",
            serde_json::to_value(untrimmable).unwrap(),
            2,
            vec![SteadyGlideError::NotBracketed.to_string()],
        ),
        (
            "wing-without-root",
            serde_json::to_value(displaced_wing).unwrap(),
            2,
            vec![ShapeError::OutsideSurface(HybridSurfaceRole::MainWing).to_string()],
        ),
        (
            "schema-four",
            legacy,
            2,
            vec![
                "Aircraft schema version is unsupported".into(),
                "SchemaVersion /schema_version: 4 (expected schema_version == 5)".into(),
            ],
        ),
        (
            "schema-five-authoring",
            serde_json::to_value(invalid_authoring).unwrap(),
            2,
            vec![
                "/mass".into(),
                "airframe_center_of_mass_frd_m: [1000.0".into(),
                format!("/surfaces/{tail_index}/actuator: None"),
                format!("/surfaces/{wing_index}/sections/0/twist_rad: 4.0"),
            ],
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
            for reason in expected_reasons {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains(&reason),
                    "{name}: expected reason {reason:?}, stderr: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
}
