use birdman_game_core::{SessionPhase, SessionReplaySource, SessionSnapshot};
use birdman_game_format::{DifficultyPreset, DifficultySettings};

use super::types::*;
use birdman_game_app::application::RecordSourceId;

/// Retained view dependencies, excluding Frame values and record-source identity.
#[derive(Debug, PartialEq)]
pub struct GameUiStructureKey {
    phase: SessionPhase,
    resume_available: bool,
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
            resume_available: context.resume_available,
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

/// Retained structural revision owned alongside one application owner's presentation resources.
#[derive(Default)]
pub struct GameUiStructureCache {
    key: Option<(RecordSourceId, GameUiStructureKey)>,
    revision: u32,
}

impl GameUiStructureCache {
    /// Observes structural inputs and the application-owned record source, preserving Frame updates.
    pub fn observe(
        &mut self,
        key: GameUiStructureKey,
        source: Option<RecordSourceId>,
    ) -> Result<u32, GameUiError> {
        let source = source.ok_or(GameUiError::RevisionExhausted)?;
        if self
            .key
            .as_ref()
            .is_none_or(|(retained_source, retained_key)| {
                *retained_source != source || retained_key != &key
            })
        {
            let revision = self
                .revision
                .checked_add(1)
                .ok_or(GameUiError::RevisionExhausted)?;
            self.key = Some((source, key));
            self.revision = revision;
        }
        Ok(self.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{context, difficulty, platform};
    use super::*;

    #[test]
    fn exhaustion_preserves_the_retained_key_and_revision() {
        let session = birdman_game_core::GameSession::new();
        let state = GameUiState::default();
        let platform = platform();
        let key = || GameUiStructureKey::new(&state, &context(&session, difficulty(), &platform));
        let source = birdman_game_app::application::ApplicationSession::default()
            .record_source_id()
            .unwrap();
        let mut empty = GameUiStructureCache::default();
        assert_eq!(
            empty.observe(key(), None),
            Err(GameUiError::RevisionExhausted)
        );
        assert_eq!(empty.revision, 0);
        assert!(empty.key.is_none());
        let mut cache = GameUiStructureCache {
            key: Some((source, key())),
            revision: u32::MAX,
        };
        assert_eq!(cache.observe(key(), Some(source)), Ok(u32::MAX));
        assert_eq!(
            cache.observe(key(), None),
            Err(GameUiError::RevisionExhausted)
        );
        let mut changed = key();
        changed.platform.notification = "変更".to_owned();
        assert_eq!(
            cache.observe(changed, Some(source)),
            Err(GameUiError::RevisionExhausted)
        );
        assert_eq!(cache.revision, u32::MAX);
        assert_eq!(cache.key, Some((source, key())));
    }
}
