use std::{fs::File, io::Read, path::PathBuf};

use birdman_game_content::{AircraftDefinition, AircraftDefinitionError, PLAYABLE_AIRCRAFT_JSON};
use birdman_game_core::SteadyGlideTrim;
use birdman_game_format::aircraft::MAX_AIRCRAFT_JSON_BYTES;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Aircraft(AircraftDefinitionError),
    Controller(birdman_game_format::controller::ControllerFormatError),
}

pub struct LoadedAircraft {
    pub definition: AircraftDefinition,
    pub trim: SteadyGlideTrim,
}

pub struct DocumentState {
    pub source: String,
    pub sha256: Option<String>,
    pub bytes: usize,
    pub result: Result<LoadedAircraft, LoadError>,
}

impl DocumentState {
    pub fn decode(source: String, bytes: &[u8]) -> Self {
        let result = AircraftDefinition::decode_json(bytes)
            .and_then(|definition| {
                let trim = definition.trim()?;
                Ok(LoadedAircraft { definition, trim })
            })
            .map_err(LoadError::Aircraft);
        Self {
            source,
            sha256: (bytes.len() <= MAX_AIRCRAFT_JSON_BYTES).then(|| {
                Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect()
            }),
            bytes: bytes.len(),
            result,
        }
    }

    pub fn open(path: PathBuf) -> Self {
        // Read at most the format's limit plus one byte to classify oversized input.
        let read = File::open(&path).and_then(|file| {
            let mut bytes = Vec::new();
            file.take(MAX_AIRCRAFT_JSON_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        match read {
            Ok(bytes) => Self::decode(path.display().to_string(), &bytes),
            Err(error) => Self {
                source: path.display().to_string(),
                sha256: None,
                bytes: 0,
                result: Err(LoadError::Io(error)),
            },
        }
    }
}

pub struct ControllerState {
    pub source: String,
    pub result: Result<birdman_game_format::controller::ControllerDocument, LoadError>,
}

impl ControllerState {
    pub fn decode(source: String, bytes: &[u8]) -> Self {
        Self {
            source,
            result: birdman_game_format::controller::ControllerDocument::decode_json(bytes)
                .map_err(LoadError::Controller),
        }
    }

    pub fn open(path: PathBuf) -> Self {
        let result = File::open(&path).and_then(|file| {
            let mut bytes = Vec::new();
            file.take(birdman_game_format::controller::MAX_CONTROLLER_JSON_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        match result {
            Ok(bytes) => Self::decode(path.display().to_string(), &bytes),
            Err(cause) => Self {
                source: path.display().to_string(),
                result: Err(LoadError::Io(cause)),
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Design,
    Characteristics,
    Planform,
    Controller,
}

pub struct ViewerApp {
    pub document: DocumentState,
    pub controller: ControllerState,
    pub tab: Tab,
}

impl ViewerApp {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            document: path.map_or_else(Self::registered, DocumentState::open),
            controller: ControllerState::decode(
                "同梱の制御設定".into(),
                birdman_game_content::DEFAULT_CONTROLLER_JSON,
            ),
            tab: Tab::Design,
        }
    }

    pub fn registered() -> DocumentState {
        DocumentState::decode("登録機体（同梱JSON）".into(), PLAYABLE_AIRCRAFT_JSON)
    }
}

impl eframe::App for ViewerApp {
    fn update(&mut self, context: &eframe::egui::Context, _: &mut eframe::Frame) {
        if let Some(file) = context.input(|input| input.raw.dropped_files.last().cloned()) {
            if let Some(path) = file.path {
                if self.tab == Tab::Controller {
                    self.controller = ControllerState::open(path);
                } else {
                    self.document = DocumentState::open(path);
                }
            } else if let Some(bytes) = file.bytes {
                if self.tab == Tab::Controller {
                    self.controller = ControllerState::decode(file.name, &bytes);
                } else {
                    self.document = DocumentState::decode(file.name, &bytes);
                }
            }
        }
        crate::ui::show(context, self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_replacement_keeps_its_identity_and_replaces_previous_model() {
        let mut app = ViewerApp::new(None);
        assert!(app.document.result.is_ok());
        assert_eq!(
            app.document
                .result
                .as_ref()
                .unwrap()
                .definition
                .document()
                .schema_version,
            5
        );

        let invalid = String::from_utf8(birdman_game_content::DEFAULT_CONTROLLER_JSON.to_vec())
            .unwrap()
            .replacen("\"kp\": 0.2", "\"kp\": -1.0", 1)
            .replace("\"shared_authority\": 0.5", "\"shared_authority\": 1.5");
        app.controller =
            ControllerState::decode("invalid-controller.json".into(), invalid.as_bytes());
        let Err(LoadError::Controller(
            birdman_game_format::controller::ControllerFormatError::Validation(issues),
        )) = &app.controller.result
        else {
            panic!("typed controller issues");
        };
        assert!(issues.iter().any(|issue| issue.path == "/pitch/rate/kp"));
        assert!(issues.iter().any(|issue| issue.path == "/shared_authority"));
        assert_eq!(app.controller.source, "invalid-controller.json");
        app.document = DocumentState::decode(
            "schema-four.json".into(),
            br#"{"schema_version":4,"mass":{},"surfaces":[{"actuator":null}]}"#,
        );
        let Err(LoadError::Aircraft(AircraftDefinitionError::Format(cause))) = &app.document.result
        else {
            panic!("typed aircraft schema error")
        };
        assert!(matches!(
            cause,
            birdman_game_format::aircraft::AircraftFormatError::UnsupportedSchemaVersion(_)
        ));
        assert_eq!(cause.validation_issues()[0].path, "/schema_version");
        app.document = DocumentState::decode("broken.json".into(), b"{");
        assert!(matches!(app.document.result, Err(LoadError::Aircraft(_))));
        assert_eq!(app.document.source, "broken.json");
        assert_eq!(app.document.bytes, 1);
        assert_eq!(
            app.document.sha256.as_deref(),
            Some("021fb596db81e6d02bf3d2586ee3981fe519f275c0ac9ca76bbcf2ebb4097d96")
        );
    }
}
