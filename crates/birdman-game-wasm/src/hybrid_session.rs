use std::sync::OnceLock;

use birdman_game_core::{
    ActuatorError, BodyPoint, BodyVector, CompositeCgLaunchConditions, ControlMode, CourseAxis,
    DistanceScoreError, DynamicsError, GameSessionConfiguration, GameSessionError, Gravity,
    HybridAerodynamicLoad, HybridError, HybridMockConfiguration, HybridMockDefinition,
    HybridMockError, HybridMockTrim, HybridModel, HybridSurface, MathError, NedPoint, NedVector,
    SessionScenarioIdentity, TailControlProfile, TailFlightScenario, TailFlightScenarioError,
    TailFlightScenarioParameters, TailIncidence,
};
use birdman_game_format::{EnvironmentFormatError, FlightRecordTailIdentityDocument};

use crate::environment::bundled_environment;

const CONTROLLER_PROFILE_ID: &str = "bpg040-tail-rate-feedback";
const CONTROLLER_PROFILE_VERSION: u32 = 1;
const CATALOG_VERSION: u32 = 2;
const SCENARIO_ID: u32 = 6;
const SCENARIO_VERSION: u32 = 1;
const CONTACT_POINTS: [BodyPoint; 1] = [BodyPoint::origin()];
static HYBRID_DEFINITION: OnceLock<Result<HybridMockDefinition, HybridMockError>> = OnceLock::new();
static HYBRID_SURFACES: OnceLock<Result<[HybridSurface<'static>; 3], HybridMockError>> =
    OnceLock::new();

/// Typed preparation failures before any hybrid session state or record is published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridSessionPreparationError {
    /// The fictional model or its geometric load view failed validation.
    Mock(HybridMockError),
    /// The owned offline environment or its wind grid failed validation.
    Environment(EnvironmentFormatError),
    /// A physical coordinate or course axis failed validation.
    Math(MathError),
    /// The gravity or initial aircraft state failed validation.
    Dynamics(DynamicsError),
    /// The scored course axis failed validation.
    Course(DistanceScoreError),
    /// The hybrid load model failed validation.
    Hybrid(HybridError),
    /// The software control profile failed validation.
    Control(ActuatorError),
    /// The sealed scenario rejected its initial physical or load state.
    Scenario(TailFlightScenarioError),
    /// The session tick limit or versioned identity failed validation.
    Session(GameSessionError),
}

/// Additive Rust adapter preparation with stable owned resources and no JavaScript factory change.
///
/// The configuration borrows immutable cached model and environment storage. It
/// can be moved into the existing GameSession without retaining a self-reference.
pub struct HybridSessionPreparation {
    configuration: GameSessionConfiguration<'static>,
    record_identity: FlightRecordTailIdentityDocument,
}

impl HybridSessionPreparation {
    /// Builds the fictional hybrid definition in the registered offline Typical environment.
    ///
    /// Initial air-relative speed and attitude follow the mock trim. The selected
    /// wind at the composite CG is added once to obtain launch ground velocity.
    pub fn try_new(
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        seed: u64,
    ) -> Result<Self, HybridSessionPreparationError> {
        let definition = cached_definition()?;
        let surfaces = cached_surfaces()?;
        let environment =
            bundled_environment().map_err(HybridSessionPreparationError::Environment)?;
        let wind = environment
            .wind_field()
            .map_err(HybridSessionPreparationError::Environment)?;
        let trim =
            HybridMockTrim::try_new(definition).map_err(HybridSessionPreparationError::Mock)?;
        let cg_position =
            NedPoint::try_new(0.0, 0.0, -10.5).map_err(HybridSessionPreparationError::Math)?;
        let air_state = trim
            .initial_state_for_ground_launch(cg_position, 0.0)
            .map_err(HybridSessionPreparationError::Mock)?;
        let wind_velocity = wind
            .velocity_at(cg_position)
            .map_err(|error| {
                HybridSessionPreparationError::Environment(EnvironmentFormatError::Wind(error))
            })?
            .components();
        let air_velocity = air_state.datum_velocity_ned().components();
        let ground_velocity = NedVector::try_new(
            air_velocity[0] + wind_velocity[0],
            air_velocity[1] + wind_velocity[1],
            air_velocity[2] + wind_velocity[2],
        )
        .map_err(HybridSessionPreparationError::Math)?;
        let launch = CompositeCgLaunchConditions::try_new(
            cg_position,
            ground_velocity,
            air_state.attitude_body_to_ned(),
            BodyVector::zero(),
            trim.pilot_position_m(),
            0.0,
        )
        .map_err(HybridSessionPreparationError::Dynamics)?;
        let parameters = TailFlightScenarioParameters::try_new(
            definition.aircraft(),
            launch,
            TailIncidence::neutral(),
            Gravity::try_new(HybridMockTrim::GRAVITY_MPS2)
                .map_err(HybridSessionPreparationError::Dynamics)?,
            &CONTACT_POINTS,
            CourseAxis::try_new(1.0, 0.0).map_err(HybridSessionPreparationError::Course)?,
        )
        .map_err(HybridSessionPreparationError::Scenario)?;
        let load = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(
                definition
                    .polar()
                    .map_err(HybridSessionPreparationError::Mock)?,
                surfaces,
            )
            .map_err(HybridSessionPreparationError::Hybrid)?,
            HybridMockTrim::AIR_DENSITY_KG_M3,
            wind,
        )
        .map_err(HybridSessionPreparationError::Hybrid)?;
        let scenario = TailFlightScenario::try_new(
            parameters,
            load,
            TailControlProfile::try_new(0.2, 0.2, 1.0)
                .map_err(HybridSessionPreparationError::Control)?,
        )
        .map_err(HybridSessionPreparationError::Scenario)?;
        let identity = SessionScenarioIdentity {
            catalog_version: CATALOG_VERSION,
            scenario_id: SCENARIO_ID,
            scenario_version: SCENARIO_VERSION,
            aircraft_model_version: definition.configuration().model_version(),
            environment_version: environment.document().environment_version,
            controller_profile_version: CONTROLLER_PROFILE_VERSION,
            seed,
        };
        let configuration = GameSessionConfiguration::try_new_tail(
            scenario,
            control_mode,
            maximum_flight_ticks,
            identity,
        )
        .map_err(HybridSessionPreparationError::Session)?;
        let record_identity = FlightRecordTailIdentityDocument {
            aircraft_configuration_id: definition.configuration().configuration_id().to_owned(),
            controller_profile_id: CONTROLLER_PROFILE_ID.to_owned(),
        };
        Ok(Self {
            configuration,
            record_identity,
        })
    }

    /// Moves the sealed core configuration and its definition-derived archive identity to adapters.
    pub fn into_parts(
        self,
    ) -> (
        GameSessionConfiguration<'static>,
        FlightRecordTailIdentityDocument,
    ) {
        (self.configuration, self.record_identity)
    }
}

fn cached_definition() -> Result<&'static HybridMockDefinition, HybridSessionPreparationError> {
    HYBRID_DEFINITION
        .get_or_init(|| HybridMockDefinition::try_new(HybridMockConfiguration::Standard))
        .as_ref()
        .map_err(|error| HybridSessionPreparationError::Mock(*error))
}

fn cached_surfaces() -> Result<&'static [HybridSurface<'static>; 3], HybridSessionPreparationError>
{
    let definition = cached_definition()?;
    HYBRID_SURFACES
        .get_or_init(|| definition.surfaces())
        .as_ref()
        .map_err(|error| HybridSessionPreparationError::Mock(*error))
}

#[cfg(test)]
mod tests;
