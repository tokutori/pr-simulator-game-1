use std::{fs::File, io::Read, path::PathBuf};

use birdman_game_content::{AircraftDefinition, AircraftDefinitionError, PLAYABLE_AIRCRAFT_JSON};
use birdman_game_core::SteadyGlideTrim;
use birdman_game_format::aircraft::MAX_AIRCRAFT_JSON_BYTES;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Aircraft(AircraftDefinitionError),
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
            sha256: (bytes.len() <= MAX_AIRCRAFT_JSON_BYTES)
                .then(|| format!("{:x}", Sha256::digest(bytes))),
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Design,
    Characteristics,
    Planform,
}

pub struct ViewerApp {
    pub document: DocumentState,
    pub tab: Tab,
}

impl ViewerApp {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            document: path.map_or_else(Self::registered, DocumentState::open),
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
                self.document = DocumentState::open(path);
            } else if let Some(bytes) = file.bytes {
                self.document = DocumentState::decode(file.name, &bytes);
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
