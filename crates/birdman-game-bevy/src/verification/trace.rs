use super::Verification;
use crate::app::native_runtime::NativeInputs;
use bevy::{ecs as bevy_ecs, prelude::*};
use birdman_game_runtime::{
    ApplicationInput, ApplicationObservation, ApplicationOutput, ApplicationPlatformObservation,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

#[derive(Resource)]
pub(crate) struct ApplicationTrace(BufWriter<File>);

impl ApplicationTrace {
    pub(super) fn create(directory: &Path) -> io::Result<Self> {
        Ok(Self(BufWriter::new(File::create(
            directory.join("application-trace.jsonl"),
        )?)))
    }

    fn write(&mut self, value: &impl Serialize) -> io::Result<()> {
        self.0.write_all(
            &serde_json::to_vec(value).map_err(|cause| io::Error::other(cause.to_string()))?,
        )?;
        self.0.write_all(b"\n")
    }
}

fn write(world: &mut World, value: Value, flush: bool) -> Result<(), String> {
    let Some(mut trace) = world.get_resource_mut::<ApplicationTrace>() else {
        return Ok(());
    };
    let result = trace
        .write(&value)
        .and_then(|()| if flush { trace.0.flush() } else { Ok(()) });
    result.map_err(|cause| format!("Native application trace: {cause}"))
}

pub(crate) fn wire_value(value: &impl Serialize) -> Result<Value, String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|cause| format!("Native trace serialization: {cause}"))?;
    serde_json::from_slice(&bytes).map_err(|cause| format!("Native trace JSON: {cause}"))
}

pub(crate) fn report(world: &mut World, result: Result<(), String>) {
    if let Err(cause) = result {
        if let Some(mut verification) = world.get_resource_mut::<Verification>() {
            verification.complete(Err(cause.clone()));
            verification.failure = Some(cause);
        }
        world
            .resource_mut::<NativeInputs>()
            .0
            .push(ApplicationInput::Dispose);
    }
}

pub(crate) fn initialize(world: &mut World, platform: &ApplicationPlatformObservation) {
    let result = write(world, json!({ "platform": platform }), false);
    report(world, result);
}

pub(crate) fn observation(world: &World, observation: &ApplicationObservation) -> Option<Value> {
    world
        .contains_resource::<ApplicationTrace>()
        .then(|| json!(observation))
}

pub(crate) fn output(
    world: &mut World,
    observation: Option<Value>,
    fonts: Vec<Value>,
    output: &ApplicationOutput,
    flight: Value,
) {
    if let Some(observation) = observation {
        let result = wire_value(&output.requests).and_then(|requests| write(
            world,
            json!({ "observation": observation, "fonts": fonts, "requests": requests, "lifecycle": output.lifecycle, "keyboard_consumed": output.keyboard_consumed, "flight": flight }),
            output.lifecycle == birdman_game_runtime::ApplicationLifecycle::Drained,
        ));
        report(world, result);
    }
}

pub(crate) fn failed(
    world: &mut World,
    observation: Option<Value>,
    fonts: Vec<Value>,
    cause: String,
) {
    if let Some(observation) = observation {
        let result = write(
            world,
            json!({ "observation": observation, "fonts": fonts, "error": cause }),
            false,
        );
        report(world, result);
    }
}

pub(super) fn capture_started(world: &mut World, file: &str, serial: u64) -> Result<(), String> {
    write(
        world,
        json!({ "capture_started": serial, "file": file }),
        false,
    )
}

pub(super) fn capture(
    world: &mut World,
    file: &str,
    serial: u64,
    drawing: Option<&Value>,
) -> Result<(), String> {
    if world.contains_resource::<ApplicationTrace>() {
        let value = json!({ "capture": drawing, "file": file, "serial": serial });
        write(world, value, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_retains_actual_observations_failures_and_capture_boundaries() {
        let value = 0.22_f32;
        let recorded = wire_value(&value).unwrap();
        assert_eq!(
            serde_json::to_string(&recorded).unwrap(),
            serde_json::to_string(&value).unwrap()
        );
        assert_ne!(recorded, serde_json::to_value(value).unwrap());
        let directory = crate::app::native_storage::tests::Directory::new();
        let mut world = World::new();
        world.insert_resource(ApplicationTrace::create(&directory.0).unwrap());
        let input = ApplicationObservation {
            observed_ms: 3.0,
            input: ApplicationInput::PageVisibility { visible: false },
        };
        let observed = observation(&world, &input);
        let fonts = vec![
            json!({ "probe": "observed probe", "result": { "Err": "measurement-unavailable" } }),
        ];
        output(
            &mut world,
            observed.clone(),
            fonts.clone(),
            &ApplicationOutput {
                drawing: None,
                lifecycle: birdman_game_runtime::ApplicationLifecycle::Running,
                diagnostic: None,
                requests: Vec::new(),
                keyboard_consumed: false,
            },
            Value::Null,
        );
        failed(
            &mut world,
            observed,
            fonts,
            "original observation error".into(),
        );
        capture_started(&mut world, "capture.png", 1).unwrap();
        capture(&mut world, "capture.png", 1, None).unwrap();
        let bytes = std::fs::read_to_string(directory.0.join("application-trace.jsonl")).unwrap();
        let rows: Vec<Value> = bytes
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0]["observation"]["input"]["visible"], false);
        assert!(rows[0].get("drawing").is_none());
        assert_eq!(rows[1]["error"], "original observation error");
        assert_eq!(
            rows[1]["fonts"][0]["result"]["Err"],
            "measurement-unavailable"
        );
        assert_eq!(rows[2]["capture_started"], 1);
        assert_eq!(rows[3]["file"], "capture.png");
        assert_eq!(rows[3]["serial"], 1);
        assert!(rows[3]["capture"].is_null());
    }
}
