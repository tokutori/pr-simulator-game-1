use super::{
    FlightRecordDispositionDocument, FlightRecordDocument, FlightRecordEndReasonDocument,
    FlightRecordFinalizationDocument, FlightRecordFormatError, FlightRecordHeaderDocument,
    FlightRecordInformationDocument, FlightRecordTelemetryDocument, MAX_FLIGHT_RECORD_JSON_BYTES,
};
use crate::DifficultySettings;
use alloc::{string::String, vec::Vec};
use birdman_game_core::{
    BodyVector, DistanceScore, FlightRecord, FlightRecordControls, FlightRecordFinalization,
    FlightRecordHeader, FlightRecordSample, FlightRecordTailInput, FlightState, FlightTelemetry,
    NedPoint, NedVector, PersonalBestComparison, PersonalBestKey, SessionScenarioIdentity,
    TailFlightTickInput, TailIncidence, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionIntent, TailRateTarget, UnitQuaternion, compare_personal_best,
};
use serde::{Deserialize, Serialize};

/// Archive schema with named two-tail controls, distinct from legacy schemas 1 through 5.
pub const TAIL_FLIGHT_RECORD_SCHEMA_VERSION: u32 = 6;

/// Explicit model/controller names; their independent versions remain in the common header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordTailIdentityDocument {
    /// Immutable aircraft configuration name, including the distinct zero-dihedral oracle name.
    pub aircraft_configuration_id: String,
    /// Software tail-controller profile name, independent of aircraft data.
    pub controller_profile_id: String,
}

/// Named physical angles in radians, preserving the two-tail contract without a roll slot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailIncidenceDocument {
    /// Horizontal-tail effective incidence, positive for negative pitch moment on an aft tail.
    pub horizontal_tail_rad: f64,
    /// Vertical-tail effective incidence, positive for negative yaw moment with the configured fin.
    pub vertical_tail_rad: f64,
}

impl TailIncidenceDocument {
    fn from_core(incidence: TailIncidence) -> Self {
        Self {
            horizontal_tail_rad: incidence.elevator_rad(),
            vertical_tail_rad: incidence.rudder_rad(),
        }
    }

    fn to_core(self) -> Result<TailIncidence, FlightRecordFormatError> {
        TailIncidence::try_new(self.horizontal_tail_rad, self.vertical_tail_rad)
            .map_err(|_| FlightRecordFormatError::InvalidRecord)
    }
}

/// Saved input absence or a normalized longitudinal-position instruction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TailPilotPositionCommandDocument {
    /// Retain the previously resolved physical target.
    Hold {},
    /// Resolve this normalized target through the configured trim mapping.
    Set {
        /// Normalized position in the inclusive interval [-1, 1].
        normalized: f64,
    },
}

/// One saved interval's distinct intent, resolved target and already computed command values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailFlightRecordInputDocument {
    /// Normalized positive nose-up intent, independent of physical incidence sign.
    pub nose_up: f64,
    /// Normalized positive right-turn intent, independent of physical incidence sign.
    pub turn_right: f64,
    /// Desired body-positive pitch rate q in rad/s.
    pub desired_pitch_rate_rad_s: f64,
    /// Desired body-positive yaw rate r in rad/s.
    pub desired_yaw_rate_rad_s: f64,
    /// Original Hold/Set command, distinct from the resolved physical target.
    pub pilot_position_command: TailPilotPositionCommandDocument,
    /// Authoritative retained or newly resolved pilot target in metres.
    pub resolved_pilot_position_target_m: f64,
    /// Manual intent mapped to physical incidence before authority mixing.
    pub manual_incidence_target: TailIncidenceDocument,
    /// Core feedback output before authority mixing.
    pub fbw_incidence_target: TailIncidenceDocument,
    /// Mixed target before the software actuator slew step.
    pub mixed_incidence_target: TailIncidenceDocument,
}

/// Schema-specific tagged control pair; unknown layouts and surplus axes are rejected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "layout", rename_all = "snake_case", deny_unknown_fields)]
pub enum TailFlightRecordControlsDocument {
    /// Matching two-tail physical values and interval input.
    TailIncidence {
        /// Physical incidences actually held in the interval ending at this sample.
        physical_incidence: TailIncidenceDocument,
        /// Original interval input and outputs, absent only at tick zero.
        input_from_previous: Option<TailFlightRecordInputDocument>,
    },
}

/// Saved physics and telemetry values shared by analysis and snapshot playback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRecordStateDocument {
    /// Aircraft datum position in NED metres.
    pub datum_position_ned_m: [f64; 3],
    /// Aircraft datum ground velocity in NED metres per second.
    pub datum_velocity_ned_mps: [f64; 3],
    /// Body-to-NED quaternion in scalar-first order.
    pub attitude_body_to_ned: [f64; 4],
    /// Body p/q/r rates in rad/s; roll rate remains physical telemetry.
    pub angular_velocity_body_rad_s: [f64; 3],
    /// Actual longitudinal pilot position in metres.
    pub pilot_position_m: f64,
    /// Actual pilot velocity relative to the airframe in metres per second.
    pub pilot_velocity_mps: f64,
    /// Ambient wind at the composite center in NED metres per second.
    pub wind_at_cg_ned_mps: [f64; 3],
    /// Saved derived telemetry, with undefined angles represented explicitly.
    pub telemetry: FlightRecordTelemetryDocument,
}

/// One integer or fractional terminal snapshot with its paired two-tail controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailFlightRecordSampleDocument {
    /// Integer tick containing this sample.
    pub tick_index: u64,
    /// Fraction within the tick interval.
    pub fraction: f64,
    /// Saved aircraft, pilot, wind and telemetry values.
    pub state: FlightRecordStateDocument,
    /// Physical controls paired with their own interval input.
    pub controls: TailFlightRecordControlsDocument,
}

/// Finalized v6 archive, independent of the current public legacy writer/default model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailFlightRecordDocument {
    /// Exactly schema version 6, never interpreted as a legacy three-axis schema.
    pub schema_version: u32,
    /// Scenario, model/controller versions, bounds, difficulty and comparison metadata.
    pub header: FlightRecordHeaderDocument,
    /// Model/controller configuration names, independent of their version counters.
    pub control_identity: FlightRecordTailIdentityDocument,
    /// Chronological initial/successful-tick/optional terminal samples.
    pub samples: Vec<TailFlightRecordSampleDocument>,
    /// Immutable exact terminal time, disposition and score.
    pub finalization: FlightRecordFinalizationDocument,
}

/// Version-specific archive interpretation, preserving all legacy saved-snapshot meanings.
#[derive(Clone, Debug, PartialEq)]
pub enum FlightRecordArchiveDocument {
    /// Saved three-axis snapshots from schemas 1 through 5.
    Legacy(FlightRecordDocument),
    /// Saved two-tail snapshots from schema 6.
    Tail(TailFlightRecordDocument),
}

impl TailFlightRecordDocument {
    /// Copies a finalized two-tail record without re-running any physics or controller.
    pub fn from_record(
        record: &FlightRecord,
        settings: DifficultySettings,
        identity: FlightRecordTailIdentityDocument,
    ) -> Result<Self, FlightRecordFormatError> {
        let finalization = record
            .finalization()
            .ok_or(FlightRecordFormatError::RecordUnavailable)?;
        if finalization.failure.is_some() {
            return Err(FlightRecordFormatError::IncompatibleTerminalCause);
        }
        let core_header = record.header();
        let scenario = core_header.scenario;
        let document = Self {
            schema_version: TAIL_FLIGHT_RECORD_SCHEMA_VERSION,
            header: FlightRecordHeaderDocument {
                catalog_version: scenario.catalog_version,
                scenario_id: scenario.scenario_id,
                scenario_version: scenario.scenario_version,
                aircraft_model_version: scenario.aircraft_model_version,
                environment_version: scenario.environment_version,
                controller_profile_version: scenario.controller_profile_version,
                difficulty: settings.into(),
                seed: scenario.seed,
                maximum_flight_ticks: core_header.maximum_flight_ticks,
                physics_hz: core_header.physics_hz,
                score_definition_version: Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION),
                physics_model_version: Some(birdman_game_core::PHYSICS_MODEL_VERSION),
                personal_best_key: None,
            },
            control_identity: identity,
            samples: record
                .samples()
                .iter()
                .map(TailFlightRecordSampleDocument::from_core)
                .collect::<Result<Vec<_>, _>>()?,
            finalization: FlightRecordFinalizationDocument {
                reason: finalization.reason.into(),
                disposition: finalization.disposition.into(),
                terminal_tick: finalization.terminal_tick,
                terminal_fraction: finalization.terminal_fraction,
                score_m: finalization.score.map(|score| {
                    [
                        score.course_parallel_m(),
                        score.cross_track_m(),
                        score.net_horizontal_m(),
                    ]
                }),
            },
        };
        document.validate()?;
        Ok(document)
    }

    fn scenario_identity(&self) -> SessionScenarioIdentity {
        SessionScenarioIdentity {
            catalog_version: self.header.catalog_version,
            scenario_id: self.header.scenario_id,
            scenario_version: self.header.scenario_version,
            aircraft_model_version: self.header.aircraft_model_version,
            environment_version: self.header.environment_version,
            controller_profile_version: self.header.controller_profile_version,
            seed: self.header.seed,
        }
    }

    /// Validates schema/identity and restores the same immutable core query representation.
    pub fn to_finalized_core_record(&self) -> Result<FlightRecord, FlightRecordFormatError> {
        if self.schema_version != TAIL_FLIGHT_RECORD_SCHEMA_VERSION {
            return Err(FlightRecordFormatError::UnsupportedSchemaVersion);
        }
        let valid_id = |value: &str| {
            !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
        };
        if !valid_id(&self.control_identity.aircraft_configuration_id)
            || !valid_id(&self.control_identity.controller_profile_id)
            || self.header.physics_hz != birdman_game_core::PHYSICS_HZ
            || self.header.score_definition_version
                != Some(birdman_game_core::COURSE_DISTANCE_SCORE_VERSION)
            || !self.header.physics_model_version.is_some_and(|version| {
                (1..=birdman_game_core::PHYSICS_MODEL_VERSION).contains(&version)
            })
            || matches!(
                self.header.difficulty.information,
                FlightRecordInformationDocument::Custom
            ) != self.header.difficulty.hud_profile.is_some()
            || self.samples.len() > birdman_game_core::MAX_FLIGHT_RECORD_SAMPLES
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        let header =
            FlightRecordHeader::try_new(self.scenario_identity(), self.header.maximum_flight_ticks)
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?;
        let samples = self
            .samples
            .iter()
            .map(TailFlightRecordSampleDocument::to_core)
            .collect::<Result<Vec<_>, _>>()?;
        let score = self
            .finalization
            .score_m
            .map(|[course, cross_track, net]| {
                DistanceScore::try_from_recorded(course, cross_track, net)
                    .map_err(|_| FlightRecordFormatError::InvalidRecord)
            })
            .transpose()?;
        let finalization = FlightRecordFinalization {
            reason: match self.finalization.reason {
                FlightRecordEndReasonDocument::WaterContact => {
                    birdman_game_core::SessionEndReason::WaterContact
                }
                FlightRecordEndReasonDocument::OutOfValidEnvelope => {
                    birdman_game_core::SessionEndReason::OutOfValidEnvelope
                }
                FlightRecordEndReasonDocument::ManualAbort => {
                    birdman_game_core::SessionEndReason::ManualAbort
                }
                FlightRecordEndReasonDocument::FatalSimulationError => {
                    birdman_game_core::SessionEndReason::FatalSimulationError
                }
                FlightRecordEndReasonDocument::TimeLimit => {
                    birdman_game_core::SessionEndReason::TimeLimit
                }
            },
            disposition: match self.finalization.disposition {
                FlightRecordDispositionDocument::Complete => {
                    birdman_game_core::FlightRecordDisposition::Complete
                }
                FlightRecordDispositionDocument::Interrupted => {
                    birdman_game_core::FlightRecordDisposition::Interrupted
                }
                FlightRecordDispositionDocument::Failed => {
                    birdman_game_core::FlightRecordDisposition::Failed
                }
            },
            terminal_tick: self.finalization.terminal_tick,
            terminal_fraction: self.finalization.terminal_fraction,
            score,
            failure: None,
        };
        if self.header.personal_best_key.is_some()
            && (finalization.reason != birdman_game_core::SessionEndReason::WaterContact
                || finalization.disposition != birdman_game_core::FlightRecordDisposition::Complete
                || score.is_none())
        {
            return Err(FlightRecordFormatError::InvalidRecord);
        }
        FlightRecord::try_from_finalized_samples(header, samples, finalization)
            .map_err(|_| FlightRecordFormatError::InvalidRecord)
    }

    /// Validates all archive semantics without reintegrating saved inputs.
    pub fn validate(&self) -> Result<(), FlightRecordFormatError> {
        self.to_finalized_core_record().map(|_| ())
    }

    /// Attaches the separately derived canonical comparison key to eligible finalized data.
    pub fn with_personal_best_key(
        mut self,
        key: Option<PersonalBestKey>,
    ) -> Result<Self, FlightRecordFormatError> {
        self.header.personal_best_key = key.map(PersonalBestKey::digest);
        self.validate()?;
        Ok(self)
    }

    /// Returns a current-model complete contact score; this does not generate a canonical key.
    pub fn personal_best_candidate_score(
        &self,
    ) -> Result<Option<DistanceScore>, FlightRecordFormatError> {
        let record = self.to_finalized_core_record()?;
        if self.header.physics_model_version != Some(birdman_game_core::PHYSICS_MODEL_VERSION) {
            return Ok(None);
        }
        Ok(record.personal_best_candidate_score())
    }

    /// Encodes this validated v6 archive within the bounded JSON size.
    pub fn encode_json(&self) -> Result<Vec<u8>, FlightRecordFormatError> {
        self.validate()?;
        let encoded =
            serde_json::to_vec(self).map_err(|_| FlightRecordFormatError::EncodingFailed)?;
        if encoded.len() > MAX_FLIGHT_RECORD_JSON_BYTES {
            return Err(FlightRecordFormatError::InputTooLarge);
        }
        Ok(encoded)
    }
}

impl FlightRecordArchiveDocument {
    /// Uses the exact version-specific decoder without reinterpreting legacy field meanings.
    pub fn decode_json(input: &[u8]) -> Result<Self, FlightRecordFormatError> {
        if input.len() > MAX_FLIGHT_RECORD_JSON_BYTES {
            return Err(FlightRecordFormatError::InputTooLarge);
        }
        #[derive(Deserialize)]
        struct SchemaVersion {
            schema_version: u32,
        }
        let version: SchemaVersion =
            serde_json::from_slice(input).map_err(|_| FlightRecordFormatError::InvalidJson)?;
        match version.schema_version {
            1..=5 => FlightRecordDocument::decode_json(input).map(Self::Legacy),
            TAIL_FLIGHT_RECORD_SCHEMA_VERSION => {
                let document: TailFlightRecordDocument = serde_json::from_slice(input)
                    .map_err(|_| FlightRecordFormatError::InvalidJson)?;
                document.validate()?;
                Ok(Self::Tail(document))
            }
            _ => Err(FlightRecordFormatError::UnsupportedSchemaVersion),
        }
    }

    /// Re-encodes the original schema and values, retaining old model/controller identity.
    pub fn encode_json(&self) -> Result<Vec<u8>, FlightRecordFormatError> {
        match self {
            Self::Legacy(document) => document.encode_json(),
            Self::Tail(document) => document.encode_json(),
        }
    }

    /// Restores saved states for the existing Analysis/Replay query; no model is evaluated.
    pub fn to_finalized_core_record(&self) -> Result<FlightRecord, FlightRecordFormatError> {
        match self {
            Self::Legacy(document) => document.to_finalized_core_record(),
            Self::Tail(document) => document.to_finalized_core_record(),
        }
    }

    /// Explicitly rejects old controls or mismatched model/controller/scenario/physics identity.
    pub fn require_tail_reintegration_compatibility(
        &self,
        identity: &FlightRecordTailIdentityDocument,
        scenario: SessionScenarioIdentity,
        physics_model_version: u32,
    ) -> Result<(), FlightRecordFormatError> {
        let Self::Tail(document) = self else {
            self.to_finalized_core_record()?;
            return Err(FlightRecordFormatError::IncompatibleReintegration);
        };
        document.validate()?;
        if document.control_identity != *identity
            || document.scenario_identity() != scenario
            || document.header.physics_model_version != Some(physics_model_version)
        {
            return Err(FlightRecordFormatError::IncompatibleReintegration);
        }
        Ok(())
    }
}

/// Compares only eligible v6 records with matching explicit model/controller identity and keys.
pub fn compare_archive_personal_best_records(
    candidate: &FlightRecordArchiveDocument,
    existing: &FlightRecordArchiveDocument,
) -> Result<Option<PersonalBestComparison>, FlightRecordFormatError> {
    candidate.to_finalized_core_record()?;
    existing.to_finalized_core_record()?;
    let (FlightRecordArchiveDocument::Tail(candidate), FlightRecordArchiveDocument::Tail(existing)) =
        (candidate, existing)
    else {
        return Ok(None);
    };
    let (Some(candidate_score), Some(existing_score)) = (
        candidate.personal_best_candidate_score()?,
        existing.personal_best_candidate_score()?,
    ) else {
        return Ok(None);
    };
    let (Some(candidate_key), Some(existing_key)) = (
        candidate.header.personal_best_key,
        existing.header.personal_best_key,
    ) else {
        return Ok(None);
    };
    if candidate.control_identity != existing.control_identity
        || candidate.header.aircraft_model_version != existing.header.aircraft_model_version
        || candidate.header.controller_profile_version != existing.header.controller_profile_version
    {
        return Ok(Some(PersonalBestComparison::DifferentConfiguration));
    }
    Ok(Some(compare_personal_best(
        PersonalBestKey::from_digest(candidate_key),
        candidate_score,
        PersonalBestKey::from_digest(existing_key),
        existing_score,
    )))
}

impl TailFlightRecordSampleDocument {
    fn from_core(sample: &FlightRecordSample) -> Result<Self, FlightRecordFormatError> {
        let FlightRecordControls::TailIncidence {
            incidence,
            input_from_previous,
        } = sample.controls
        else {
            return Err(FlightRecordFormatError::IncompatibleControlLayout);
        };
        let state = sample.flight_state;
        let telemetry = sample.telemetry;
        Ok(Self {
            tick_index: sample.tick_index,
            fraction: sample.fraction,
            state: FlightRecordStateDocument {
                datum_position_ned_m: state.datum_position_ned().components(),
                datum_velocity_ned_mps: state.datum_velocity_ned().components(),
                attitude_body_to_ned: state.attitude_body_to_ned().components(),
                angular_velocity_body_rad_s: state.angular_velocity_body().components(),
                pilot_position_m: state.pilot_position_m(),
                pilot_velocity_mps: state.pilot_velocity_mps(),
                wind_at_cg_ned_mps: sample.wind_at_cg_ned_mps.components(),
                telemetry: FlightRecordTelemetryDocument {
                    composite_cg_position_ned_m: telemetry.composite_cg_position_ned_m.components(),
                    altitude_m: telemetry.altitude_m,
                    airspeed_mps: telemetry.airspeed_mps,
                    groundspeed_mps: telemetry.groundspeed_mps,
                    angle_of_attack_rad: telemetry.angle_of_attack_rad,
                    sideslip_angle_rad: telemetry.sideslip_angle_rad,
                    attitude_euler_rad: [
                        telemetry.roll_rad,
                        telemetry.pitch_rad,
                        telemetry.heading_rad,
                    ],
                },
            },
            controls: TailFlightRecordControlsDocument::TailIncidence {
                physical_incidence: TailIncidenceDocument::from_core(incidence),
                input_from_previous: input_from_previous
                    .map(TailFlightRecordInputDocument::from_core),
            },
        })
    }

    fn to_core(&self) -> Result<FlightRecordSample, FlightRecordFormatError> {
        let state = &self.state;
        let vector = |components: [f64; 3]| {
            NedVector::try_new(components[0], components[1], components[2])
                .map_err(|_| FlightRecordFormatError::InvalidRecord)
        };
        let point = |components: [f64; 3]| {
            NedPoint::try_new(components[0], components[1], components[2])
                .map_err(|_| FlightRecordFormatError::InvalidRecord)
        };
        let attitude = state.attitude_body_to_ned;
        let angular_velocity = state.angular_velocity_body_rad_s;
        let telemetry = &state.telemetry;
        let TailFlightRecordControlsDocument::TailIncidence {
            physical_incidence,
            input_from_previous,
        } = &self.controls;
        Ok(FlightRecordSample {
            tick_index: self.tick_index,
            fraction: self.fraction,
            flight_state: FlightState::try_new(
                point(state.datum_position_ned_m)?,
                vector(state.datum_velocity_ned_mps)?,
                UnitQuaternion::try_new(attitude[0], attitude[1], attitude[2], attitude[3])
                    .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
                BodyVector::try_new(
                    angular_velocity[0],
                    angular_velocity[1],
                    angular_velocity[2],
                )
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
                state.pilot_position_m,
                state.pilot_velocity_mps,
            )
            .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
            controls: FlightRecordControls::TailIncidence {
                incidence: physical_incidence.to_core()?,
                input_from_previous: input_from_previous
                    .as_ref()
                    .map(TailFlightRecordInputDocument::to_core)
                    .transpose()?,
            },
            wind_at_cg_ned_mps: vector(state.wind_at_cg_ned_mps)?,
            telemetry: FlightTelemetry {
                composite_cg_position_ned_m: point(telemetry.composite_cg_position_ned_m)?,
                altitude_m: telemetry.altitude_m,
                airspeed_mps: telemetry.airspeed_mps,
                groundspeed_mps: telemetry.groundspeed_mps,
                wind_velocity_ned_mps: vector(state.wind_at_cg_ned_mps)?,
                angle_of_attack_rad: telemetry.angle_of_attack_rad,
                sideslip_angle_rad: telemetry.sideslip_angle_rad,
                roll_rad: telemetry.attitude_euler_rad[0],
                pitch_rad: telemetry.attitude_euler_rad[1],
                heading_rad: telemetry.attitude_euler_rad[2],
            },
        })
    }
}

impl TailFlightRecordInputDocument {
    fn from_core(input: FlightRecordTailInput) -> Self {
        Self {
            nose_up: input.manual_intent().nose_up(),
            turn_right: input.manual_intent().turn_right(),
            desired_pitch_rate_rad_s: input.desired_body_rate().pitch_rad_per_second(),
            desired_yaw_rate_rad_s: input.desired_body_rate().yaw_rad_per_second(),
            pilot_position_command: match input.pilot_position_command() {
                TailPilotPositionCommand::Hold => TailPilotPositionCommandDocument::Hold {},
                TailPilotPositionCommand::Set(intent) => TailPilotPositionCommandDocument::Set {
                    normalized: intent.value(),
                },
            },
            resolved_pilot_position_target_m: input.resolved_pilot_position_target_m(),
            manual_incidence_target: TailIncidenceDocument::from_core(
                input.manual_incidence_target(),
            ),
            fbw_incidence_target: TailIncidenceDocument::from_core(input.fbw_incidence_target()),
            mixed_incidence_target: TailIncidenceDocument::from_core(
                input.mixed_incidence_target(),
            ),
        }
    }

    fn to_core(&self) -> Result<FlightRecordTailInput, FlightRecordFormatError> {
        let command = match self.pilot_position_command {
            TailPilotPositionCommandDocument::Hold {} => TailPilotPositionCommand::Hold,
            TailPilotPositionCommandDocument::Set { normalized } => TailPilotPositionCommand::Set(
                TailPilotPositionIntent::try_new(normalized)
                    .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
            ),
        };
        let input = TailFlightTickInput::new(
            TailPilotIntent::try_new(self.nose_up, self.turn_right)
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
            TailRateTarget::try_new(self.desired_pitch_rate_rad_s, self.desired_yaw_rate_rad_s)
                .map_err(|_| FlightRecordFormatError::InvalidRecord)?,
            command,
        );
        FlightRecordTailInput::try_from_recorded(
            input,
            self.resolved_pilot_position_target_m,
            self.manual_incidence_target.to_core()?,
            self.fbw_incidence_target.to_core()?,
            self.mixed_incidence_target.to_core()?,
        )
        .map_err(|_| FlightRecordFormatError::InvalidRecord)
    }
}

#[cfg(test)]
mod tests;
