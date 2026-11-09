use birdman_game_content::{AircraftDefinition, AircraftDefinitionError, PLAYABLE_AIRCRAFT_JSON};
use sha2::{Digest, Sha256};

/// Selected original document and its prepared physical and structural definition.
pub struct SelectedAircraft {
    original: Vec<u8>,
    definition: std::sync::Arc<AircraftDefinition>,
    raw_sha256: String,
    identity: [u8; 32],
    launch_position: std::sync::OnceLock<
        Result<birdman_game_core::NedPoint, crate::HybridSessionPreparationError>,
    >,
}

impl SelectedAircraft {
    /// Validates the original bounded document and prepares every derived model.
    pub fn decode(original: Vec<u8>) -> Result<Self, AircraftDefinitionError> {
        let definition = AircraftDefinition::decode_json(&original)?;
        let raw_sha256: String = Sha256::digest(&original)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let identity = birdman_game_format::aircraft_identity(
            &definition.document().configuration_id,
            &raw_sha256,
        );
        Ok(Self {
            original,
            definition: std::sync::Arc::new(definition),
            raw_sha256,
            identity,
            launch_position: std::sync::OnceLock::new(),
        })
    }

    /// Prepares the bundled document through the same selection path as external input.
    pub fn bundled() -> Result<Self, AircraftDefinitionError> {
        Self::decode(PLAYABLE_AIRCRAFT_JSON.to_vec())
    }

    /// Borrows the single validated physical and structural definition.
    pub fn definition(&self) -> &AircraftDefinition {
        &self.definition
    }
    /// Shares the same immutable validated definition with owned CPU presentation work.
    pub fn shared_definition(&self) -> std::sync::Arc<AircraftDefinition> {
        std::sync::Arc::clone(&self.definition)
    }
    /// Borrows the original source bytes for atomic persistence.
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Returns the digest of the original source bytes.
    pub fn raw_sha256(&self) -> &str {
        &self.raw_sha256
    }
    /// Returns the fixed-length configuration and original-source identity.
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    /// Caches the selected hull's support position at the shared launch platform.
    pub fn launch_position(
        &self,
    ) -> Result<birdman_game_core::NedPoint, crate::HybridSessionPreparationError> {
        self.launch_position
            .get_or_init(|| crate::hybrid_session::launch_position(&self.definition))
            .clone()
    }

    /// Authorizes legacy builtin geometry only for the actual bundled source bytes.
    pub fn is_bundled(&self) -> bool {
        self.original == PLAYABLE_AIRCRAFT_JSON
    }
}
