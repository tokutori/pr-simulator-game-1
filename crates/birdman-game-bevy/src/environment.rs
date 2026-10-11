use bevy::ecs as bevy_ecs;
use bevy::prelude::{AssetId, Component, Image, Resource, Shader};
use birdman_game_core::{SessionPhase, SessionScenarioIdentity};
use birdman_game_format::{
    EnvironmentFormatError, SkyStateDocument, WaveStateDocument, WeatherClass,
};
use birdman_game_session::{
    DEFAULT_SESSION_SEED, DEFAULT_WEATHER, HybridSessionPreparation, HybridSessionPreparationError,
    bundled_environment, identity_for_selection, preset_environment_for_version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentTarget {
    DefaultExhibition,
    Sealed(SessionScenarioIdentity),
    MissingIdentity(SessionPhase),
}

impl EnvironmentTarget {
    pub(crate) fn project(phase: SessionPhase, identity: Option<SessionScenarioIdentity>) -> Self {
        match phase {
            SessionPhase::Title => Self::DefaultExhibition,
            SessionPhase::FlightSetup if identity.is_none() => Self::DefaultExhibition,
            _ => identity.map_or(Self::MissingIdentity(phase), Self::Sealed),
        }
    }

    pub(crate) fn resolve(self) -> Result<ResolvedEnvironment, EnvironmentFailure> {
        let identity = match self {
            Self::DefaultExhibition => identity_for_selection(
                HybridSessionPreparation::select_scenario(DEFAULT_WEATHER, DEFAULT_SESSION_SEED)
                    .map_err(EnvironmentFailure::Registry)?,
            ),
            Self::Sealed(identity) => identity,
            Self::MissingIdentity(phase) => return Err(EnvironmentFailure::MissingIdentity(phase)),
        };
        for weather in [
            WeatherClass::Calm,
            WeatherClass::Mild,
            WeatherClass::Typical,
            WeatherClass::Challenging,
            WeatherClass::NearLimit,
        ] {
            let selection = HybridSessionPreparation::select_scenario(weather, identity.seed)
                .map_err(EnvironmentFailure::Registry)?;
            if identity_for_selection(selection) != identity {
                continue;
            }
            let (waves, sky) = if weather == WeatherClass::Typical {
                let document = bundled_environment()
                    .map_err(EnvironmentFailure::Metadata)?
                    .document();
                (document.waves, Some(document.sky))
            } else {
                let environment = preset_environment_for_version(selection.environment_version)
                    .ok_or(EnvironmentFailure::UnknownIdentity(identity))?;
                (environment.waves, None)
            };
            return Ok(ResolvedEnvironment {
                target: self,
                condition: EnvironmentCondition {
                    identity,
                    weather,
                    waves,
                    sky,
                },
            });
        }
        Err(EnvironmentFailure::UnknownIdentity(identity))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EnvironmentCondition {
    pub(crate) identity: SessionScenarioIdentity,
    pub(crate) weather: WeatherClass,
    pub(crate) waves: WaveStateDocument,
    pub(crate) sky: Option<SkyStateDocument>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentSurface {
    Near,
    Far,
    Sky,
}

impl EnvironmentSurface {
    pub(crate) const ALL: [Self; 3] = [Self::Near, Self::Far, Self::Sky];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Near => 0,
            Self::Far => 1,
            Self::Sky => 2,
        }
    }
}

#[derive(Component)]
pub(crate) struct EnvironmentSun;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentFailure {
    Registry(HybridSessionPreparationError),
    Metadata(EnvironmentFormatError),
    MissingIdentity(SessionPhase),
    UnknownIdentity(SessionScenarioIdentity),
    TargetNotApplied(EnvironmentTarget),
    SurfaceUnavailable(EnvironmentSurface),
    SurfaceDuplicated(EnvironmentSurface),
    MaterialUnavailable(EnvironmentSurface),
    SharedMaterial,
    BoundsUnavailable,
    SunUnavailable,
    WavePreparation(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedEnvironment {
    target: EnvironmentTarget,
    condition: EnvironmentCondition,
}

impl ResolvedEnvironment {
    pub(crate) fn condition(self) -> EnvironmentCondition {
        self.condition
    }
}

#[derive(Clone, Debug, PartialEq)]
enum EnvironmentState {
    Applied(ResolvedEnvironment),
    Failed {
        applied: ResolvedEnvironment,
        requested: EnvironmentTarget,
        cause: EnvironmentFailure,
    },
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct NativeEnvironment {
    state: EnvironmentState,
    detail_mip_failure: Option<DetailMipFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DetailMipFailure {
    pub(crate) images: Vec<AssetId<Image>>,
    pub(crate) shader: AssetId<Shader>,
    pub(crate) entry: &'static str,
    pub(crate) cause: String,
}

impl NativeEnvironment {
    pub(crate) fn try_default() -> Result<Self, EnvironmentFailure> {
        Ok(Self {
            state: EnvironmentState::Applied(EnvironmentTarget::DefaultExhibition.resolve()?),
            detail_mip_failure: None,
        })
    }

    fn applied(&self) -> ResolvedEnvironment {
        match &self.state {
            EnvironmentState::Applied(applied) | EnvironmentState::Failed { applied, .. } => {
                *applied
            }
        }
    }

    pub(crate) fn condition_for(
        &self,
        target: EnvironmentTarget,
    ) -> Result<EnvironmentCondition, EnvironmentFailure> {
        match &self.state {
            EnvironmentState::Applied(applied) if applied.target == target => Ok(applied.condition),
            EnvironmentState::Failed { cause, .. } => Err(cause.clone()),
            EnvironmentState::Applied(_) => Err(EnvironmentFailure::TargetNotApplied(target)),
        }
    }

    pub(crate) fn needs_update(&self, target: EnvironmentTarget) -> bool {
        match &self.state {
            EnvironmentState::Applied(applied) => applied.target != target,
            EnvironmentState::Failed { requested, .. } => *requested != target,
        }
    }

    pub(crate) fn commit(&mut self, resolved: ResolvedEnvironment) {
        self.state = EnvironmentState::Applied(resolved);
        self.detail_mip_failure = None;
    }

    pub(crate) fn failed(&mut self, requested: EnvironmentTarget, cause: EnvironmentFailure) {
        self.state = EnvironmentState::Failed {
            applied: self.applied(),
            requested,
            cause,
        };
    }

    pub(crate) fn detail_mip_failure(&self) -> Option<&DetailMipFailure> {
        self.detail_mip_failure.as_ref()
    }

    pub(crate) fn report_detail_mip_failure(&mut self, cause: Option<DetailMipFailure>) {
        self.detail_mip_failure = cause;
    }

    pub(crate) fn failure_notice(&self) -> Option<String> {
        let environment = match &self.state {
            EnvironmentState::Applied(_) => None,
            EnvironmentState::Failed {
                applied,
                requested,
                cause,
            } => Some(format!(
                "描画環境の適用に失敗した。以前の表示を保持する。\n要求: {requested:?}\n表示中のidentity: {:?}\n原因: {cause:?}",
                applied.condition.identity,
            )),
        };
        let mip = self.detail_mip_failure.as_ref().map(|failure| format!(
            "水面微細波のGPU mipmap生成に失敗した。対象画像の再queueを停止した。\nimages: {:?}\nshader: {:?}\nentry point: {}\n原因: {}",
            failure.images, failure.shader, failure.entry, failure.cause,
        ));
        match (environment, mip) {
            (Some(environment), Some(mip)) => Some(format!("{environment}\n{mip}")),
            (Some(notice), None) | (None, Some(notice)) => Some(notice),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_title_and_unprepared_setup_use_the_explicit_default_exhibition() {
        for phase in [SessionPhase::Title, SessionPhase::FlightSetup] {
            assert_eq!(
                EnvironmentTarget::project(phase, None),
                EnvironmentTarget::DefaultExhibition
            );
        }
        let identity = EnvironmentTarget::DefaultExhibition
            .resolve()
            .unwrap()
            .condition()
            .identity;
        assert_eq!(
            EnvironmentTarget::project(SessionPhase::FlightSetup, Some(identity)),
            EnvironmentTarget::Sealed(identity)
        );
        assert_eq!(
            EnvironmentTarget::project(SessionPhase::Title, Some(identity)),
            EnvironmentTarget::DefaultExhibition
        );
        for phase in [
            SessionPhase::BriefingPreparing,
            SessionPhase::BriefingReady,
            SessionPhase::Countdown { remaining_ticks: 3 },
            SessionPhase::FlightRunning,
            SessionPhase::Result,
            SessionPhase::Replay,
            SessionPhase::Attract,
        ] {
            let target = EnvironmentTarget::project(phase, None);
            assert_eq!(
                target.resolve(),
                Err(EnvironmentFailure::MissingIdentity(phase))
            );
        }
    }

    #[test]
    fn shared_current_full_identity_preserves_calm_waves_and_unavailable_sky() {
        for weather in [WeatherClass::Calm, WeatherClass::Typical] {
            let identity = identity_for_selection(
                HybridSessionPreparation::select_scenario(weather, 42).unwrap(),
            );
            let condition = EnvironmentTarget::Sealed(identity)
                .resolve()
                .unwrap()
                .condition();
            assert_eq!(condition.identity, identity);
            assert_eq!(condition.weather, weather);
            if weather == WeatherClass::Calm {
                let registered = preset_environment_for_version(1).unwrap();
                assert_eq!(condition.waves, registered.waves);
                assert_eq!(registered.wind_velocity_ned_mps, [0.0; 3]);
                assert_ne!(condition.waves.wind_velocity_ne_mps, [0.0; 2]);
                assert!(condition.sky.is_none());
            } else {
                let document = bundled_environment().unwrap().document();
                assert_eq!(condition.waves, document.waves);
                assert_eq!(condition.sky, Some(document.sky));
            }
        }
    }

    #[test]
    fn every_full_identity_field_is_checked_and_failure_retains_the_applied_condition() {
        let mut presentation = NativeEnvironment::try_default().unwrap();
        let initial = presentation.applied();
        for field in 0..6 {
            let mut identity = initial.condition.identity;
            match field {
                0 => identity.catalog_version = 99,
                1 => identity.scenario_id = 99,
                2 => identity.scenario_version = 99,
                3 => identity.aircraft_model_version = 99,
                4 => identity.environment_version = 99,
                5 => identity.controller_profile_version = 99,
                _ => unreachable!(),
            }
            let target = EnvironmentTarget::Sealed(identity);
            assert_eq!(
                target.resolve(),
                Err(EnvironmentFailure::UnknownIdentity(identity))
            );
            presentation.failed(target, target.resolve().unwrap_err());
            assert_eq!(presentation.applied(), initial);
            assert!(presentation.condition_for(target).is_err());
            assert!(
                presentation
                    .failure_notice()
                    .unwrap()
                    .contains("UnknownIdentity")
            );
        }
        presentation.commit(initial);
        assert_eq!(presentation.applied(), initial);
        assert!(presentation.failure_notice().is_none());
    }

    #[test]
    fn gpu_mip_diagnostic_is_separate_from_cpu_exchange_and_clears_on_new_environment() {
        let mut presentation = NativeEnvironment::try_default().unwrap();
        let initial = presentation.applied();
        let failure = DetailMipFailure {
            images: vec![AssetId::<Image>::default()],
            shader: AssetId::<Shader>::default(),
            entry: "downsample_second",
            cause: "CreateShaderModule(original GPU validation cause)".into(),
        };
        presentation.report_detail_mip_failure(Some(failure.clone()));
        assert_eq!(
            presentation.condition_for(initial.target),
            Ok(initial.condition)
        );
        let notice = presentation.failure_notice().unwrap();
        assert!(notice.contains("GPU mipmap"));
        assert!(notice.contains("downsample_second"));
        assert!(notice.contains("original GPU validation cause"));
        assert!(!notice.contains("以前の表示を保持"));
        assert_eq!(presentation.detail_mip_failure(), Some(&failure));
        presentation.failed(initial.target, EnvironmentFailure::BoundsUnavailable);
        let notice = presentation.failure_notice().unwrap();
        assert!(notice.contains("BoundsUnavailable"));
        assert!(notice.contains("original GPU validation cause"));
        presentation.commit(initial);
        assert!(presentation.detail_mip_failure().is_none());
        assert!(presentation.failure_notice().is_none());
    }
}
