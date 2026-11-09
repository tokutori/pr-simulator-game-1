//! Build-time aggregate wire contracts and executable-module startup markup.

#[cfg(feature = "schema")]
fn main() {
    use birdman_game_runtime::{
        ApplicationGlyphMetrics, ApplicationObservation, ApplicationOutput,
        ApplicationPlatformObservation, ApplicationTextProbe,
    };
    use sha2::{Digest, Sha256};
    let mut generator = schemars::generate::SchemaSettings::draft07().into_generator();
    let platform = generator.subschema_for::<ApplicationPlatformObservation>();
    let observation = generator.subschema_for::<ApplicationObservation>();
    let output = generator.subschema_for::<ApplicationOutput>();
    let font_probe = generator.subschema_for::<ApplicationTextProbe>();
    let font_metrics = generator.subschema_for::<ApplicationGlyphMetrics>();
    let schema = serde_json::json!({ "$schema": "http://json-schema.org/draft-07/schema#", "title": "ApplicationContract", "type": "object", "properties": { "platform": platform, "observation": observation, "output": output, "font_probe": font_probe, "font_metrics": font_metrics }, "required": ["platform", "observation", "output", "font_probe", "font_metrics"], "additionalProperties": false, "definitions": generator.take_definitions(true) });
    let digest =
        Sha256::digest(serde_json::to_vec(&schema).expect("serializable application schema"));
    let contract_sha256 = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!(
        "{}",
        serde_json::json!({ "schema": schema, "contract_sha256": contract_sha256, "startup": { "loading_html": birdman_game_runtime::application_startup_html(false), "failure_html": birdman_game_runtime::application_startup_html(true) } })
    );
}

#[cfg(not(feature = "schema"))]
fn main() {
    panic!("Enable the schema feature");
}
