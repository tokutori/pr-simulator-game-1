use birdman_game_core::{SessionPhase, SessionReplaySource, SessionSnapshot};
use birdman_game_format::{DifficultyPreset, DifficultySettings};

use super::types::*;

/// Retained view dependencies, excluding Frame values; application operations invalidate record content.
#[derive(Debug, PartialEq)]
pub struct GameUiStructureKey {
    phase: SessionPhase,
    replay_source: Option<SessionReplaySource>,
    selections: GameUiState,
    difficulty: DifficultySettings,
    preset: DifficultyPreset,
    identity: Option<birdman_game_core::SessionScenarioIdentity>,
    record_available: bool,
    environment_readout: String,
    platform: GameUiPlatformInput,
}

impl GameUiStructureKey {
    /// Derives structural dependencies from the same inputs as the shared view.
    pub fn new(state: &GameUiState, context: &GameUiContext<'_, '_>) -> Self {
        let snapshot = context.session.snapshot();
        let phase = match snapshot.phase() {
            SessionPhase::Countdown { .. } => SessionPhase::Countdown {
                remaining_seconds: 0,
            },
            phase => phase,
        };
        let replay_source = match snapshot {
            SessionSnapshot::Replay { source, .. } => Some(source),
            _ => None,
        };
        let mut selections = state.clone();
        selections.analysis_time_seconds = 0.0;
        Self {
            phase,
            replay_source,
            selections,
            difficulty: context.difficulty,
            preset: context.preset,
            identity: context.identity,
            record_available: context.record.is_some(),
            environment_readout: context.environment_readout.to_owned(),
            platform: context.platform.clone(),
        }
    }
}

#[derive(Default)]
pub(crate) struct GameUiStructureCache {
    key: Option<GameUiStructureKey>,
    revision: u32,
}

impl GameUiStructureCache {
    pub(crate) fn observe(&mut self, key: GameUiStructureKey) -> Result<u32, GameUiError> {
        if self.key.as_ref() != Some(&key) {
            let revision = self
                .revision
                .checked_add(1)
                .ok_or(GameUiError::RevisionExhausted)?;
            self.key = Some(key);
            self.revision = revision;
        }
        Ok(self.revision)
    }

    pub(crate) fn invalidate(&mut self) {
        self.key = None;
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{context, difficulty, platform};
    use super::*;

    #[test]
    fn exhaustion_preserves_the_revision_and_never_reuses_an_invalidated_key() {
        let session = birdman_game_core::GameSession::new();
        let state = GameUiState::default();
        let platform = platform();
        let key = || GameUiStructureKey::new(&state, &context(&session, difficulty(), &platform));
        let mut cache = GameUiStructureCache {
            key: Some(key()),
            revision: u32::MAX,
        };
        assert_eq!(cache.observe(key()), Ok(u32::MAX));
        cache.invalidate();
        assert_eq!(cache.observe(key()), Err(GameUiError::RevisionExhausted));
        assert_eq!(cache.revision, u32::MAX);
        assert!(cache.key.is_none());
    }
}
