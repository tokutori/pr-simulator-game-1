//! Prepares the same fixed-source Web bundle used by all Native packages.

use birdman_game_host::web_distribution::write_web_bundle_manifest;
use birdman_game_runtime::APPLICATION_CONTRACT_SHA256;
use std::{error::Error, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let directory = arguments
        .next()
        .map(PathBuf::from)
        .ok_or("Usage: web_bundle <finished-web-directory> <full-source-sha>")?;
    let source = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or("Usage: web_bundle <finished-web-directory> <full-source-sha>")?;
    if arguments.next().is_some() {
        return Err("Usage: web_bundle <finished-web-directory> <full-source-sha>".into());
    }
    let checkout = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .output()?;
    if !checkout.status.success() || String::from_utf8(checkout.stdout)?.trim() != source {
        return Err("Web bundle source differs from the current checkout".into());
    }
    let manifest = write_web_bundle_manifest(&directory, &source, APPLICATION_CONTRACT_SHA256)?;
    println!("{}", manifest.display());
    Ok(())
}
