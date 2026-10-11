use birdman_game_core::DistanceScore;
use birdman_game_core::{
    ControlMode, CourseAxis, PersonalBestComparison, PersonalBestKey, SessionScenarioIdentity,
    TailControlProfile, compare_personal_best,
};
use sha2::{Digest, Sha256};

use crate::{
    AssistanceLevel, DifficultySettings, FlightRecordFormatError, FlightRecordTailIdentityDocument,
    InformationLevel, TailFlightRecordControlsDocument, TailFlightRecordDocument, WeatherClass,
};

/// Resolved two-tail comparison conditions with a sealed controller profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailPersonalBestConfiguration<'a> {
    /// Exact immutable scenario and model versions used by the session.
    pub scenario: SessionScenarioIdentity,
    /// Resolved information, assistance and weather settings.
    pub difficulty: DifficultySettings,
    /// Exact model/controller names accompanying those versions.
    pub identity: &'a FlightRecordTailIdentityDocument,
    /// Authority mode used by the session.
    pub control_mode: ControlMode,
    /// Once-resolved two-axis feedback, software slew and optional datum-alpha guard.
    pub controller_profile: TailControlProfile,
}

/// Deterministic selection among eligible schema-six records of one explicit configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct TailPersonalBestSelection {
    key: PersonalBestKey,
    best_score: DistanceScore,
    selected_existing_id: Option<u64>,
    identity: FlightRecordTailIdentityDocument,
    aircraft_model_version: u32,
    controller_profile_version: u32,
}

impl TailPersonalBestSelection {
    /// Starts selection from a keyed complete contact record.
    pub fn try_new(
        candidate: &TailFlightRecordDocument,
    ) -> Result<Option<Self>, FlightRecordFormatError> {
        let Some(score) = candidate.personal_best_candidate_score()? else {
            return Ok(None);
        };
        let Some(key) = candidate.header.personal_best_key else {
            return Ok(None);
        };
        Ok(Some(Self {
            key: PersonalBestKey::from_digest(key),
            best_score: score,
            selected_existing_id: None,
            identity: candidate.control_identity.clone(),
            aircraft_model_version: candidate.header.aircraft_model_version,
            controller_profile_version: candidate.header.controller_profile_version,
        }))
    }

    /// Considers one validated record with matching explicit identities.
    pub fn consider_existing(
        &mut self,
        id: u64,
        existing: &TailFlightRecordDocument,
    ) -> Result<(), FlightRecordFormatError> {
        if id == 0 {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        let Some(score) = existing.personal_best_candidate_score()? else {
            return Ok(());
        };
        let Some(key) = existing.header.personal_best_key else {
            return Ok(());
        };
        if existing.control_identity != self.identity
            || existing.header.aircraft_model_version != self.aircraft_model_version
            || existing.header.controller_profile_version != self.controller_profile_version
        {
            return Ok(());
        }
        match compare_personal_best(
            self.key,
            self.best_score,
            PersonalBestKey::from_digest(key),
            score,
        ) {
            PersonalBestComparison::ExistingWins => {
                self.best_score = score;
                self.selected_existing_id = Some(id);
            }
            PersonalBestComparison::EqualScore if self.selected_existing_id.is_none() => {
                self.selected_existing_id = Some(id);
            }
            PersonalBestComparison::CandidateWins
            | PersonalBestComparison::EqualScore
            | PersonalBestComparison::DifferentConfiguration => {}
        }
        Ok(())
    }

    /// Returns the distinct two-tail canonical key.
    pub const fn key(&self) -> PersonalBestKey {
        self.key
    }

    /// Returns the first winning persisted record identifier, if present.
    pub const fn selected_existing_id(&self) -> Option<u64> {
        self.selected_existing_id
    }
}

/// Hashes exact two-tail conditions without recomputing flight outputs.
pub fn canonical_tail_personal_best_key(
    record: &TailFlightRecordDocument,
    configuration: TailPersonalBestConfiguration<'_>,
    course_axis: CourseAxis,
    content_hashes: PersonalBestContentHashes,
) -> Result<Option<PersonalBestKey>, FlightRecordFormatError> {
    if record.personal_best_candidate_score()?.is_none() {
        return Ok(None);
    }
    let header = &record.header;
    let scenario = configuration.scenario;
    if header.catalog_version != scenario.catalog_version
        || header.scenario_id != scenario.scenario_id
        || header.scenario_version != scenario.scenario_version
        || header.aircraft_model_version != scenario.aircraft_model_version
        || header.environment_version != scenario.environment_version
        || header.controller_profile_version != scenario.controller_profile_version
        || header.seed != scenario.seed
        || header.difficulty != configuration.difficulty.into()
        || record.control_identity != *configuration.identity
        || matches!(
            configuration.difficulty.assistance(),
            AssistanceLevel::Manual
        ) != matches!(configuration.control_mode, ControlMode::Manual)
    {
        return Err(FlightRecordFormatError::InvalidRecord);
    }
    let mut hasher = Sha256::new();
    hasher.update(b"birdman-game/personal-best-key/tail-incidence/v1\0");
    hash_u32(&mut hasher, record.schema_version);
    for name in [
        &configuration.identity.aircraft_configuration_id,
        &configuration.identity.controller_profile_id,
    ] {
        hash_u64(&mut hasher, name.len() as u64);
        hasher.update(name.as_bytes());
    }
    hash_u32(&mut hasher, header.catalog_version);
    hash_u32(&mut hasher, header.scenario_id);
    hash_u32(&mut hasher, header.scenario_version);
    hash_u32(&mut hasher, header.aircraft_model_version);
    hash_u32(&mut hasher, header.environment_version);
    hash_u32(&mut hasher, header.controller_profile_version);
    hash_u64(&mut hasher, header.seed);
    hash_information(&mut hasher, configuration.difficulty.information());
    hash_hud_profile(&mut hasher, configuration.difficulty.hud_profile());
    hash_assistance(&mut hasher, configuration.difficulty.assistance());
    hash_weather(&mut hasher, configuration.difficulty.weather());
    hash_controller_mode(&mut hasher, configuration.control_mode);
    for gain in configuration.controller_profile.gains_seconds() {
        hash_f64(&mut hasher, gain);
    }
    hash_f64(
        &mut hasher,
        configuration
            .controller_profile
            .maximum_slew_rad_per_second(),
    );
    hash_tail_alpha_guard(&mut hasher, configuration.controller_profile);
    for component in course_axis.components() {
        hash_f64(&mut hasher, component);
    }
    hasher.update(content_hashes.scenario);
    hasher.update(content_hashes.aircraft);
    hasher.update(content_hashes.environment);
    hasher.update(content_hashes.physics_build);
    hash_u32(&mut hasher, header.physics_model_version);
    hash_u32(&mut hasher, header.score_definition_version);
    hash_u32(&mut hasher, header.physics_hz);
    hash_u64(&mut hasher, header.maximum_flight_ticks);
    let initial = &record.samples[0];
    for values in [
        &initial.state.datum_position_ned_m,
        &initial.state.datum_velocity_ned_mps,
        &initial.state.angular_velocity_body_rad_s,
        &initial.state.wind_at_cg_ned_mps,
    ] {
        for value in values {
            hash_f64(&mut hasher, *value);
        }
    }
    for value in initial.state.attitude_body_to_ned {
        hash_f64(&mut hasher, value);
    }
    hash_f64(&mut hasher, initial.state.pilot_position_m);
    hash_f64(&mut hasher, initial.state.pilot_velocity_mps);
    let TailFlightRecordControlsDocument::TailIncidence {
        physical_incidence, ..
    } = initial.controls;
    hash_f64(&mut hasher, physical_incidence.horizontal_tail_rad);
    hash_f64(&mut hasher, physical_incidence.vertical_tail_rad);
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(Some(PersonalBestKey::from_digest(digest)))
}

/// Content hashes required to distinguish the exact simulation inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersonalBestContentHashes {
    /// Hash of the selected scenario definition and its referenced assets.
    pub scenario: [u8; 32],
    /// Hash of the selected aircraft mass and aerodynamic model.
    pub aircraft: [u8; 32],
    /// Hash of the selected environment and wind model.
    pub environment: [u8; 32],
    /// Hash of the Rust physics implementation build.
    pub physics_build: [u8; 32],
}

fn hash_tail_alpha_guard(hasher: &mut Sha256, profile: TailControlProfile) {
    if let Some(guard) = profile.angle_of_attack_guard() {
        hasher.update(b"birdman-game/tail-alpha-guard/v1\0");
        for value in guard.alpha_interval_rad().into_iter().chain([
            guard.trim_alpha_rad(),
            guard.preview_seconds(),
            guard.pitch_gain_seconds(),
        ]) {
            hash_f64(hasher, value);
        }
    }
}

fn hash_controller_mode(hasher: &mut Sha256, mode: ControlMode) {
    match mode {
        ControlMode::Manual => hasher.update([0]),
        ControlMode::Shared(authority) => {
            hasher.update([1]);
            hash_f64(hasher, authority.value());
        }
        ControlMode::Automatic => hasher.update([2]),
    }
}

fn hash_information(hasher: &mut Sha256, information: InformationLevel) {
    hasher.update([match information {
        InformationLevel::Full => 0,
        InformationLevel::Standard => 1,
        InformationLevel::Minimal => 2,
        InformationLevel::Realistic => 3,
        InformationLevel::Custom => 4,
    }]);
}

fn hash_hud_profile(hasher: &mut Sha256, profile: crate::HudProfile) {
    hasher.update([
        u8::from(profile.telemetry()),
        u8::from(profile.attitude()),
        u8::from(profile.wind()),
        u8::from(profile.flight_path()),
        u8::from(profile.angle_of_attack()),
        u8::from(profile.warnings()),
    ]);
}

fn hash_assistance(hasher: &mut Sha256, assistance: AssistanceLevel) {
    hasher.update([match assistance {
        AssistanceLevel::Strong => 0,
        AssistanceLevel::Assisted => 1,
        AssistanceLevel::Light => 2,
        AssistanceLevel::Manual => 3,
    }]);
}

fn hash_weather(hasher: &mut Sha256, weather: WeatherClass) {
    hasher.update([match weather {
        WeatherClass::Calm => 0,
        WeatherClass::Mild => 1,
        WeatherClass::Typical => 2,
        WeatherClass::Challenging => 3,
        WeatherClass::NearLimit => 4,
    }]);
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_be_bytes());
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn hash_f64(hasher: &mut Sha256, value: f64) {
    let canonical_bits = if value == 0.0 { 0 } else { value.to_bits() };
    hash_u64(hasher, canonical_bits);
}
