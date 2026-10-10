use bevy::ecs as bevy_ecs;
use bevy::prelude::Resource;
use birdman_game_core::SessionPhase;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WaterQuality {
    Low,
    Medium,
    #[default]
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WaterQualityProfile {
    pub(crate) near_cells: u32,
}

impl WaterQuality {
    pub(crate) const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub(crate) const fn profile(self) -> WaterQualityProfile {
        WaterQualityProfile {
            near_cells: match self {
                Self::Low => 64,
                Self::Medium => 128,
                Self::High => 256,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WaterQualityFailure {
    NearSurfaceUnavailable,
    MeshUnavailable,
    MaterialUnavailable,
    WavePreparation(String),
}

impl WaterQualityFailure {
    fn label(&self) -> &str {
        match self {
            Self::NearSurfaceUnavailable => "近景水面の描画対象を特定できない",
            Self::MeshUnavailable => "現在の近景mesh assetを取得できない",
            Self::MaterialUnavailable => "現在の近景水面materialを取得できない",
            Self::WavePreparation(cause) => cause,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum WaterQualityState {
    Ready(WaterQuality),
    Requested {
        applied: WaterQuality,
        requested: WaterQuality,
    },
    Failed {
        applied: WaterQuality,
        requested: WaterQuality,
        cause: WaterQualityFailure,
    },
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct WaterQualitySelection {
    state: WaterQualityState,
}

impl Default for WaterQualitySelection {
    fn default() -> Self {
        Self {
            state: WaterQualityState::Ready(WaterQuality::default()),
        }
    }
}

impl WaterQualitySelection {
    pub(crate) const fn applied(&self) -> WaterQuality {
        match &self.state {
            WaterQualityState::Ready(applied)
            | WaterQualityState::Requested { applied, .. }
            | WaterQualityState::Failed { applied, .. } => *applied,
        }
    }

    pub(crate) const fn pending(&self) -> Option<WaterQuality> {
        match &self.state {
            WaterQualityState::Requested { requested, .. } => Some(*requested),
            _ => None,
        }
    }

    pub(crate) fn can_select(&self, phase: SessionPhase) -> bool {
        self.pending().is_none() && quality_menu_visible(phase)
    }

    pub(crate) fn request(&self, quality: WaterQuality, phase: SessionPhase) -> Option<Self> {
        if !self.can_select(phase) {
            return None;
        }
        let applied = self.applied();
        Some(Self {
            state: if quality == applied {
                WaterQualityState::Ready(applied)
            } else {
                WaterQualityState::Requested {
                    applied,
                    requested: quality,
                }
            },
        })
    }

    pub(crate) fn completed(&self, result: Result<(), WaterQualityFailure>) -> Self {
        let WaterQualityState::Requested { applied, requested } = &self.state else {
            return self.clone();
        };
        Self {
            state: match result {
                Ok(()) => WaterQualityState::Ready(*requested),
                Err(cause) => WaterQualityState::Failed {
                    applied: *applied,
                    requested: *requested,
                    cause,
                },
            },
        }
    }

    pub(crate) fn status(&self) -> String {
        match &self.state {
            WaterQualityState::Ready(applied) => format!(
                "湖面画質: {} — {}分割\n描画密度のみを変更する。飛行条件は保持する。",
                applied.label(),
                applied.profile().near_cells,
            ),
            WaterQualityState::Requested { applied, requested } => format!(
                "湖面画質: {}を適用中（現在: {}）",
                requested.label(),
                applied.label(),
            ),
            WaterQualityState::Failed {
                applied,
                requested,
                cause,
            } => format!(
                "湖面画質: {}を保持。{}の適用に失敗した。\n{}。",
                applied.label(),
                requested.label(),
                cause.label(),
            ),
        }
    }
}

pub(crate) fn quality_menu_visible(phase: SessionPhase) -> bool {
    matches!(
        phase,
        SessionPhase::FlightSetup | SessionPhase::FlightPaused { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::super::native_session::{MenuAction, NativeSession};
    use super::*;

    #[test]
    fn profiles_preserve_high_and_have_distinct_mesh_budgets() {
        assert_eq!(WaterQuality::default(), WaterQuality::High);
        assert_eq!(
            WaterQuality::ALL.map(|quality| quality.profile().near_cells),
            [64, 128, 256]
        );
        assert_eq!(
            WaterQuality::ALL.map(WaterQuality::label),
            ["Low", "Medium", "High"]
        );
    }

    #[test]
    fn only_setup_and_pause_admit_quality_requests() {
        let mut session = NativeSession::default();
        for action in [MenuAction::Start, MenuAction::Prepare, MenuAction::Launch] {
            session.action(action).unwrap();
        }
        for _ in 0..3 {
            session.countdown(1.0);
        }
        session.action(MenuAction::Pause).unwrap();
        for phase in [SessionPhase::FlightSetup, session.game.snapshot().phase()] {
            assert!(WaterQualitySelection::default().can_select(phase));
        }
        for phase in [
            SessionPhase::Title,
            SessionPhase::BriefingReady,
            SessionPhase::FlightRunning,
            SessionPhase::Result,
        ] {
            assert!(!WaterQualitySelection::default().can_select(phase));
            assert!(
                WaterQualitySelection::default()
                    .request(WaterQuality::Low, phase)
                    .is_none()
            );
        }
    }

    #[test]
    fn selection_changes_only_after_resource_completion_and_preserves_failure_cause() {
        let initial = WaterQualitySelection::default();
        let pending = initial
            .request(WaterQuality::Low, SessionPhase::FlightSetup)
            .unwrap();
        assert_eq!(initial.applied(), WaterQuality::High);
        assert_eq!(pending.applied(), WaterQuality::High);
        assert_eq!(pending.pending(), Some(WaterQuality::Low));
        assert!(
            pending
                .request(WaterQuality::Medium, SessionPhase::FlightSetup)
                .is_none()
        );
        let failed = pending.completed(Err(WaterQualityFailure::MeshUnavailable));
        assert_eq!(failed.applied(), WaterQuality::High);
        assert_eq!(failed.pending(), None);
        assert!(failed.status().contains("mesh asset"));
        let retry = failed
            .request(WaterQuality::Low, SessionPhase::FlightSetup)
            .unwrap();
        let applied = retry.completed(Ok(()));
        assert_eq!(applied.applied(), WaterQuality::Low);
        assert_eq!(applied.pending(), None);
        assert_eq!(
            applied.completed(Err(WaterQualityFailure::NearSurfaceUnavailable)),
            applied
        );
        assert_eq!(
            applied.request(WaterQuality::Low, SessionPhase::FlightSetup),
            Some(applied)
        );
    }
}
