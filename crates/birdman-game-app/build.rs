//! Generates shared root-relative source fingerprints and record build provenance.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace_dir = manifest_dir.join("../..").canonicalize().unwrap();
    let core_source = workspace_dir.join("crates/birdman-game-core/src");
    let environment_snapshot_source =
        workspace_dir.join("crates/birdman-game-app/src/environment_snapshot.rs");
    let environment_asset = workspace_dir.join("assets/biwa-typical-july-environment-v6.json");
    let shared_defaults_source = workspace_dir.join("crates/birdman-game-app/src/lib.rs");
    let shared_hybrid_source = workspace_dir.join("crates/birdman-game-app/src/hybrid_session.rs");
    let shared_environment_source =
        workspace_dir.join("crates/birdman-game-app/src/environment.rs");
    let shared_venue_source = workspace_dir.join("crates/birdman-game-app/src/venue.rs");
    let shared_venue_asset = workspace_dir.join("assets/biwa-launch-venue.json");
    let content_source = workspace_dir.join("crates/birdman-game-content/src");
    let aircraft_asset = workspace_dir.join("assets/aircraft/playable-glider-v6.json");
    let hybrid_trim_source = core_source.join("steady_glide.rs");
    let dynamics_source = core_source.join("dynamics.rs");
    let aerodynamics_source = core_source.join("aerodynamics.rs");
    let wind_source = core_source.join("wind_field.rs");

    let core_files = rust_sources(&workspace_dir, &core_source);
    let mut aircraft_files = vec![
        content_source.join("aircraft.rs"),
        content_source.join("shape.rs"),
        content_source.join("structure.rs"),
    ];
    aircraft_files.extend(rust_sources(&workspace_dir, &content_source.join("shape")));
    aircraft_files.extend(rust_sources(
        &workspace_dir,
        &content_source.join("structure"),
    ));
    aircraft_files.extend([
        aircraft_asset.clone(),
        workspace_dir.join("crates/birdman-game-content/Cargo.toml"),
        workspace_dir.join("crates/birdman-game-format/src/aircraft.rs"),
        dynamics_source.clone(),
        aerodynamics_source.clone(),
        hybrid_trim_source,
    ]);
    let environment_files = vec![
        environment_snapshot_source.clone(),
        aerodynamics_source,
        wind_source,
        environment_asset.clone(),
        shared_defaults_source.clone(),
        shared_environment_source.clone(),
    ];
    let mut scenario_files = vec![
        environment_snapshot_source,
        shared_defaults_source,
        shared_hybrid_source,
        shared_environment_source,
        shared_venue_source,
        shared_venue_asset,
        workspace_dir.join("crates/birdman-game-app/src/application.rs"),
        workspace_dir.join("crates/birdman-game-app/src/application/commands.rs"),
        aircraft_asset,
    ];
    scenario_files.extend(rust_sources(&workspace_dir, &content_source));

    println!("cargo:rerun-if-changed={}", environment_asset.display());
    let asset_digest = Sha256::digest(fs::read(environment_asset).unwrap());
    let asset_hash = asset_digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("cargo:rustc-env=BPG_ENVIRONMENT_V6_SHA256={asset_hash}");

    let mut physics_files = core_files;
    physics_files.push(workspace_dir.join("crates/birdman-game-core/Cargo.toml"));
    physics_files.push(workspace_dir.join("Cargo.lock"));
    physics_files.sort_by_key(|path| normalized_relative_path(&workspace_dir, path));

    for path in aircraft_files
        .iter()
        .chain(environment_files.iter())
        .chain(scenario_files.iter())
        .chain(physics_files.iter())
    {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let fingerprints = [
        (
            "SCENARIO_SOURCE_FINGERPRINT",
            digest(&workspace_dir, b"scenario-source-v1", &scenario_files),
        ),
        (
            "AIRCRAFT_SOURCE_FINGERPRINT",
            digest(&workspace_dir, b"aircraft-source-v1", &aircraft_files),
        ),
        (
            "ENVIRONMENT_SOURCE_FINGERPRINT",
            digest(&workspace_dir, b"environment-source-v1", &environment_files),
        ),
        (
            "PHYSICS_BUILD_FINGERPRINT",
            digest(&workspace_dir, b"physics-build-v1", &physics_files),
        ),
    ];
    let output = fingerprints
        .iter()
        .map(|(name, bytes)| format!("pub const {name}: [u8; 32] = {bytes:?};\n"))
        .collect::<String>();
    println!("cargo:rerun-if-env-changed=BPG_NATIVE_SOURCE_SHA");
    println!("cargo:rerun-if-env-changed=RUSTC");
    let lock_digest: [u8; 32] =
        Sha256::digest(fs::read(workspace_dir.join("Cargo.lock")).unwrap()).into();
    let rustc_version = env::var_os("RUSTC")
        .and_then(|rustc| Command::new(rustc).arg("--version").output().ok())
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|version| version.trim().to_owned());
    let declared_source = env::var("BPG_NATIVE_SOURCE_SHA")
        .ok()
        .filter(|sha| sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let output = format!(
        "{output}pub const CARGO_LOCK_SHA256: [u8; 32] = {lock_digest:?};\npub const RUSTC_VERSION: Option<&str> = {rustc_version:?};\npub const TARGET: Option<&str> = {:?};\npub const PROFILE: Option<&str> = {:?};\npub const DECLARED_SOURCE_COMMIT: Option<&str> = {declared_source:?};\n",
        env::var("TARGET").ok(),
        env::var("PROFILE").ok(),
    );
    let output_path =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("personal_best_fingerprints.rs");
    fs::write(output_path, output).unwrap();
}

fn rust_sources(workspace_dir: &Path, directory: &Path) -> Vec<PathBuf> {
    let mut pending = vec![directory.to_path_buf()];
    let mut paths = Vec::new();
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                paths.push(path);
            }
        }
    }
    paths.sort_by_key(|path| normalized_relative_path(workspace_dir, path));
    paths
}

fn digest(workspace_dir: &Path, domain: &[u8], paths: &[PathBuf]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    let mut sorted_paths = paths.to_vec();
    sorted_paths.sort_by_key(|path| normalized_relative_path(workspace_dir, path));
    for path in sorted_paths {
        let normalized_path = normalized_relative_path(workspace_dir, &path);
        let path_bytes = normalized_path.as_bytes();
        let contents = fs::read(path).unwrap();
        hasher.update((path_bytes.len() as u64).to_be_bytes());
        hasher.update(path_bytes);
        hasher.update((contents.len() as u64).to_be_bytes());
        hasher.update(contents);
    }
    hasher.finalize().into()
}

fn normalized_relative_path(workspace_dir: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_dir)
        .expect("fingerprint inputs must be inside the workspace")
        .to_string_lossy()
        .replace('\\', "/")
}
