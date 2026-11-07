use birdman_game_content::{AircraftDefinition, AircraftDefinitionError, DEFAULT_CONTROLLER_JSON};
use birdman_game_core::{ControllerReference, TailControlProfile};
use birdman_game_format::controller::{
    ControllerDocument, ControllerFormatError, ControllerValidationIssue,
};
use sha2::{Digest, Sha256};

/// Selected independently validated controller and its exact original bytes.
pub struct SelectedController {
    original: Vec<u8>,
    document: ControllerDocument,
    raw_sha256: String,
    identity: [u8; 32],
}

impl SelectedController {
    /// Validates the document independently of the selected aircraft.
    pub fn decode(original: Vec<u8>) -> Result<Self, ControllerFormatError> {
        let document = ControllerDocument::decode_json(&original)?;
        let raw_sha256: String = Sha256::digest(&original)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let identity =
            birdman_game_format::controller_identity(&document.controller_id, &raw_sha256);
        Ok(Self {
            original,
            document,
            raw_sha256,
            identity,
        })
    }

    /// Selects the bundled document through the same independent validation path.
    pub fn bundled() -> Result<Self, ControllerFormatError> {
        Self::decode(DEFAULT_CONTROLLER_JSON.to_vec())
    }

    /// Borrows independently validated settings.
    pub fn document(&self) -> &ControllerDocument {
        &self.document
    }
    /// Borrows exact source bytes for persistence.
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Original source SHA-256, independently of the selected aircraft.
    pub fn raw_sha256(&self) -> &str {
        &self.raw_sha256
    }
    /// Fixed-size controller identity sealed into a flight.
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    /// Identifies the actual bundled source without interpreting matching user identifiers.
    pub fn is_bundled(&self) -> bool {
        self.original == DEFAULT_CONTROLLER_JSON
    }

    /// Resolves the current pair; failure blocks flight preparation, not selection.
    pub fn prepare(
        &self,
        aircraft: &AircraftDefinition,
    ) -> Result<TailControlProfile, ControllerPreparationError> {
        let trim = aircraft
            .trim()
            .map_err(ControllerPreparationError::Aircraft)?;
        let issues =
            self.document
                .pair_issues(aircraft.document(), trim.theta_rad(), trim.alpha_rad());
        if !issues.is_empty() {
            return Err(ControllerPreparationError::Pair(issues));
        }
        let reference =
            ControllerReference::try_new(trim.theta_rad(), trim.conditions().airspeed_mps())
                .map_err(|cause| {
                    ControllerPreparationError::Document(ControllerFormatError::Core(cause))
                })?;
        self.document
            .core_profile(reference, trim.alpha_rad())
            .map_err(ControllerPreparationError::Document)
    }
}

/// A selected controller's intrinsic preparation or latest aircraft-pair diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerPreparationError {
    /// Controller source is unavailable or intrinsically invalid.
    Document(ControllerFormatError),
    /// Aircraft source is unavailable or could not resolve its trim.
    Aircraft(AircraftDefinitionError),
    /// Independently valid sources violate one or more pair constraints.
    Pair(Vec<ControllerValidationIssue>),
}

impl core::fmt::Display for ControllerPreparationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Document(cause) => write!(f, "{cause}"),
            Self::Aircraft(cause) => write!(f, "{cause}"),
            Self::Pair(issues) => {
                for (index, issue) in issues.iter().enumerate() {
                    if index > 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{issue}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ControllerPreparationError {}
