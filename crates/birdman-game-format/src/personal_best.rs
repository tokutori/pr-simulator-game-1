use birdman_game_core::DistanceScore;
use birdman_game_core::{
    ControlMode, CourseAxis, PersonalBestComparison, PersonalBestKey, compare_personal_best,
};
use sha2::{Digest, Sha256};

use crate::{
    AssistanceLevel, FlightRecordDocument, FlightRecordFormatError, InformationLevel,
    ResolvedConfiguration, WeatherClass,
};

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

/// Incrementally selects the best eligible record for one canonical configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PersonalBestSelection {
    key: PersonalBestKey,
    best_score: DistanceScore,
    selected_existing_id: Option<u64>,
}

impl PersonalBestSelection {
    /// Starts selection with a new record, or returns `None` when it is ineligible.
    pub fn try_new(
        candidate: &FlightRecordDocument,
    ) -> Result<Option<Self>, FlightRecordFormatError> {
        candidate.validate()?;
        let Some(key) = candidate.personal_best_key() else {
            return Ok(None);
        };
        let Some(score) = candidate.personal_best_candidate_score()? else {
            return Ok(None);
        };
        Ok(Some(Self {
            key,
            best_score: score,
            selected_existing_id: None,
        }))
    }

    /// Considers one persisted record while retaining deterministic first-winner ties.
    pub fn consider_existing(
        &mut self,
        id: u64,
        existing: &FlightRecordDocument,
    ) -> Result<(), FlightRecordFormatError> {
        if id == 0 {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        existing.validate()?;
        let Some(key) = existing.personal_best_key() else {
            return Ok(());
        };
        let Some(score) = existing.personal_best_candidate_score()? else {
            return Ok(());
        };
        match compare_personal_best(self.key, self.best_score, key, score) {
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

    /// Returns the canonical key used to group this selection.
    pub const fn key(self) -> PersonalBestKey {
        self.key
    }

    /// Returns the selected existing record ID, or `None` when the new record wins.
    pub const fn selected_existing_id(self) -> Option<u64> {
        self.selected_existing_id
    }
}

/// Compares two validated records when both carry eligible scores and canonical keys.
///
/// `Ok(None)` means at least one record cannot participate in a Personal Best comparison.
pub fn compare_personal_best_records(
    candidate: &FlightRecordDocument,
    existing: &FlightRecordDocument,
) -> Result<Option<PersonalBestComparison>, FlightRecordFormatError> {
    candidate.validate()?;
    existing.validate()?;
    let Some(candidate_score) = candidate.personal_best_candidate_score()? else {
        return Ok(None);
    };
    let Some(candidate_key) = candidate.personal_best_key() else {
        return Ok(None);
    };
    let Some(existing_score) = existing.personal_best_candidate_score()? else {
        return Ok(None);
    };
    let Some(existing_key) = existing.personal_best_key() else {
        return Ok(None);
    };
    Ok(Some(compare_personal_best(
        candidate_key,
        candidate_score,
        existing_key,
        existing_score,
    )))
}

/// Builds a Personal Best key from a complete record and matching resolved configuration.
///
/// The named preset is excluded. Equivalent resolved settings therefore produce the same key.
/// Ineligible records return `Ok(None)`; metadata disagreement returns an error.
pub fn canonical_personal_best_key(
    record: &FlightRecordDocument,
    configuration: ResolvedConfiguration,
    course_axis: CourseAxis,
    content_hashes: PersonalBestContentHashes,
) -> Result<Option<PersonalBestKey>, FlightRecordFormatError> {
    record.validate()?;
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
        || header.controller_profile_version != configuration.controller.version()
        || header.seed != scenario.seed
        || header.difficulty != configuration.difficulty.into()
        || configuration.controller.level() != configuration.difficulty.assistance()
        || configuration.difficulty.weather() != configuration.scenario.weather
    {
        return Err(FlightRecordFormatError::InvalidRecord);
    }

    let mut hasher = Sha256::new();
    hasher.update(b"birdman-game/personal-best-key/v1\0");
    hash_u32(&mut hasher, header.catalog_version);
    hash_u32(&mut hasher, header.scenario_id);
    hash_u32(&mut hasher, header.scenario_version);
    hash_u32(&mut hasher, header.aircraft_model_version);
    hash_u32(&mut hasher, header.environment_version);
    hash_u64(&mut hasher, header.seed);
    hash_information(&mut hasher, configuration.information());
    hash_hud_profile(&mut hasher, configuration.difficulty.hud_profile());
    hash_assistance(&mut hasher, configuration.assistance());
    hash_weather(&mut hasher, configuration.weather());
    hash_controller_mode(&mut hasher, configuration.controller.mode());
    hash_u32(&mut hasher, configuration.controller.version());
    for gain in configuration.controller.feedback().gains_seconds() {
        hash_f64(&mut hasher, gain);
    }
    for limit in configuration.controller.feedback().command_limits_rad() {
        hash_f64(&mut hasher, limit);
    }
    for component in course_axis.components() {
        hash_f64(&mut hasher, component);
    }
    hasher.update(content_hashes.scenario);
    hasher.update(content_hashes.aircraft);
    hasher.update(content_hashes.environment);
    hasher.update(content_hashes.physics_build);
    hash_u32(
        &mut hasher,
        header.physics_model_version.unwrap_or_default(),
    );
    hash_u32(
        &mut hasher,
        header.score_definition_version.unwrap_or_default(),
    );
    hash_u32(&mut hasher, header.physics_hz);
    hash_u64(&mut hasher, header.maximum_flight_ticks);
    hash_initial_sample(&mut hasher, &record.samples[0]);
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(Some(PersonalBestKey::from_digest(digest)))
}

fn hash_initial_sample(hasher: &mut Sha256, sample: &crate::FlightRecordSampleDocument) {
    for value in sample.datum_position_ned_m {
        hash_f64(hasher, value);
    }
    for value in sample.datum_velocity_ned_mps {
        hash_f64(hasher, value);
    }
    for value in sample.attitude_body_to_ned {
        hash_f64(hasher, value);
    }
    for value in sample.angular_velocity_body_rad_s {
        hash_f64(hasher, value);
    }
    hash_f64(hasher, sample.pilot_position_m);
    hash_f64(hasher, sample.pilot_velocity_mps);
    for value in sample.actuator_deflections_rad {
        hash_f64(hasher, value);
    }
    for value in sample.wind_at_cg_ned_mps {
        hash_f64(hasher, value);
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
