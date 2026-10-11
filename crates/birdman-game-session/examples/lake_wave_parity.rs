//! Exports shared detail bytes for an independent complete-image Web parity check.

use std::error::Error;
use std::path::PathBuf;

use birdman_game_session::lake_water::{
    LakeBroaderWavelets, LakeDetailConfig, generate_lake_detail,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Reference {
    details: Vec<Detail>,
}

#[derive(Deserialize)]
struct Detail {
    configuration: Configuration,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    extent_meters: f64,
    wavelet_count: u32,
    seed: u32,
    direction_x: f64,
    direction_z: f64,
    broader_wavelets: Option<Broader>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Broader {
    count: u32,
    feature_scale_meters: f64,
    height_scale: f64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Provide an output directory")?,
    );
    std::fs::create_dir_all(&output)?;
    let reference: Reference =
        serde_json::from_str(include_str!("../src/lake_water/web-reference.json"))?;
    for (index, detail) in reference.details.into_iter().enumerate() {
        let configuration = detail.configuration;
        let broader = configuration
            .broader_wavelets
            .map(|waves| {
                LakeBroaderWavelets::try_new(
                    waves.count,
                    waves.feature_scale_meters,
                    waves.height_scale,
                )
            })
            .transpose()?;
        let config = LakeDetailConfig::try_new(
            configuration.extent_meters,
            configuration.wavelet_count,
            configuration.seed,
            [configuration.direction_x, configuration.direction_z],
            broader,
        )?;
        let image = generate_lake_detail(config)?;
        std::fs::write(output.join(format!("detail-{index}.rgba")), image.rgba())?;
    }
    Ok(())
}
