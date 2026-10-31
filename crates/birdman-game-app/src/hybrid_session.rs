use std::sync::OnceLock;

use birdman_game_content::{AircraftDefinition, AircraftDefinitionError, playable_aircraft};
use birdman_game_core::{
    ActuatorError, BodyVector, CompositeCgLaunchConditions, ControlMode, CourseAxis,
    DistanceScoreError, DynamicsError, GameSessionConfiguration, GameSessionError, Gravity,
    HybridAerodynamicLoad, HybridError, HybridModel, HybridSurface, MathError, NedPoint, NedVector,
    SessionScenarioIdentity, TailAngleOfAttackGuard, TailControlProfile, TailFlightScenario,
    TailFlightScenarioError, TailFlightScenarioParameters, TailIncidence, WindField,
};
use birdman_game_format::{
    ConfigurationError, EnvironmentFormatError, FlightRecordTailIdentityDocument, ScenarioCatalog,
    ScenarioCatalogEntry, ScenarioSelection, WeatherClass,
};

use crate::{
    DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, DEFAULT_WEATHER, LaunchVenueError,
    environment::{bundled_environment, preset_wind_for_version},
    environment_snapshot::EnvironmentIdentity,
    launch_venue,
};

const CONTROLLER_PROFILE_ID: &str = "bpg040-tail-rate-feedback";
const CONTROLLER_PROFILE_VERSION: u32 = 4;
const CATALOG_VERSION: u32 = 9;
const SCENARIO_VERSION: u32 = 10;
const SCENARIOS: [(u32, WeatherClass); 5] = [
    (1, WeatherClass::Calm),
    (2, WeatherClass::Mild),
    (4, WeatherClass::Challenging),
    (5, WeatherClass::NearLimit),
    (6, WeatherClass::Typical),
];
static AIRCRAFT_DEFINITION: OnceLock<Result<AircraftDefinition, AircraftDefinitionError>> =
    OnceLock::new();
static HYBRID_SURFACES: OnceLock<Result<[HybridSurface<'static>; 3], AircraftDefinitionError>> =
    OnceLock::new();

/// Typed preparation failures before any hybrid session state or record is published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridSessionPreparationError {
    /// The specified operating point cannot be converted to a launch state.
    Trim(birdman_game_core::SteadyGlideError),
    /// The shared launch venue configuration failed validation.
    Venue(LaunchVenueError),
    /// The requested weather has no matching registered scenario or model identity.
    Configuration(ConfigurationError),
    /// The fictional model or its geometric load view failed validation.
    Aircraft(AircraftDefinitionError),
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
    /// Structural storage could not be reserved before preparing flight.
    AllocationFailed,
}

/// Platform-independent preparation with stable owned model and environment resources.
///
/// The configuration borrows immutable cached model and environment storage. It
/// can be moved into the existing GameSession without retaining a self-reference.
pub struct HybridSessionPreparation {
    configuration: GameSessionConfiguration<'static>,
    record_identity: FlightRecordTailIdentityDocument,
    controller_profile: TailControlProfile,
    course_axis: CourseAxis,
}

impl HybridSessionPreparation {
    /// Builds the same default configuration selected by the public Web application.
    pub fn try_default() -> Result<Self, HybridSessionPreparationError> {
        Self::try_new(
            crate::default_control_mode(),
            DEFAULT_MAXIMUM_FLIGHT_TICKS,
            DEFAULT_SESSION_SEED,
        )
    }

    /// Builds the fictional hybrid definition in the registered offline Typical environment.
    ///
    /// Initial air-relative speed and attitude follow the mock trim. The selected
    /// wind at the composite CG is added once to obtain launch ground velocity.
    pub fn try_new(
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        seed: u64,
    ) -> Result<Self, HybridSessionPreparationError> {
        Self::try_new_for_weather(control_mode, maximum_flight_ticks, seed, DEFAULT_WEATHER)
    }

    /// Resolves Weather against the current catalog and its registered wind provider.
    pub fn try_new_for_weather(
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        seed: u64,
        weather: WeatherClass,
    ) -> Result<Self, HybridSessionPreparationError> {
        let selection = Self::select_scenario(weather, seed)?;
        let definition = registered_aircraft().map_err(HybridSessionPreparationError::Aircraft)?;
        let surfaces = cached_surfaces()?;
        let wind = if selection.environment_version == 6 {
            bundled_environment()
                .map_err(HybridSessionPreparationError::Environment)?
                .wind_field()
                .map_err(HybridSessionPreparationError::Environment)?
        } else {
            let velocity = preset_wind_for_version(selection.environment_version).ok_or(
                HybridSessionPreparationError::Configuration(
                    ConfigurationError::ScenarioUnavailable,
                ),
            )?;
            WindField::uniform(
                NedVector::try_new(velocity[0], velocity[1], velocity[2])
                    .map_err(HybridSessionPreparationError::Math)?,
            )
        };
        let trim = definition
            .trim()
            .map_err(HybridSessionPreparationError::Aircraft)?;
        let cg_position =
            NedPoint::try_new(0.0, 0.0, -10.5).map_err(HybridSessionPreparationError::Math)?;
        let platform = launch_venue()
            .map_err(HybridSessionPreparationError::Venue)?
            .platform;
        let air_state = trim
            .initial_state_for_ground_launch(cg_position, platform.heading_rad())
            .map_err(HybridSessionPreparationError::Trim)?;
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
        let [course_north, course_east] = platform.horizontal_direction_ned();
        let course_axis = CourseAxis::try_new(course_north, course_east)
            .map_err(HybridSessionPreparationError::Course)?;
        let parameters = TailFlightScenarioParameters::try_new(
            definition.aircraft(),
            launch,
            TailIncidence::neutral(),
            Gravity::try_new(definition.document().trim.gravity_mps2)
                .map_err(HybridSessionPreparationError::Dynamics)?,
            definition.contact_points(),
            course_axis,
        )
        .map_err(HybridSessionPreparationError::Scenario)?;
        let load = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(
                definition
                    .polar()
                    .map_err(HybridSessionPreparationError::Aircraft)?,
                surfaces,
            )
            .map_err(HybridSessionPreparationError::Hybrid)?,
            definition.document().trim.air_density_kg_m3,
            wind,
        )
        .map_err(HybridSessionPreparationError::Hybrid)?;
        let angle_guard =
            TailAngleOfAttackGuard::try_new([-0.09, 0.09], trim.alpha_rad(), 1.0, 1.6)
                .map_err(HybridSessionPreparationError::Control)?;
        let controller_profile = TailControlProfile::try_new(0.2, 0.2, 1.0)
            .map_err(HybridSessionPreparationError::Control)?
            .with_angle_of_attack_guard(angle_guard);
        let scenario = TailFlightScenario::try_new(parameters, load, controller_profile)
            .map_err(HybridSessionPreparationError::Scenario)?;
        let identity = identity_for_selection(selection);
        let configuration = GameSessionConfiguration::try_new_tail(
            scenario,
            control_mode,
            maximum_flight_ticks,
            identity,
        )
        .map_err(HybridSessionPreparationError::Session)?;
        let record_identity = FlightRecordTailIdentityDocument {
            aircraft_configuration_id: definition.document().configuration_id.clone(),
            controller_profile_id: CONTROLLER_PROFILE_ID.to_owned(),
        };
        Ok(Self {
            configuration,
            record_identity,
            controller_profile,
            course_axis,
        })
    }

    /// Selects current registered identities without constructing a flight state.
    pub fn select_scenario(
        weather: WeatherClass,
        seed: u64,
    ) -> Result<ScenarioSelection, HybridSessionPreparationError> {
        let entries = scenario_entries()?;
        ScenarioCatalog::try_new(CATALOG_VERSION, &entries)
            .and_then(|catalog| catalog.select(weather, seed))
            .map_err(HybridSessionPreparationError::Configuration)
    }

    /// Identifies current registered model/scenario conditions from their canonical selection.
    pub fn weather_for_environment_identity(
        identity: EnvironmentIdentity,
    ) -> Result<Option<WeatherClass>, HybridSessionPreparationError> {
        let seed = u64::from(identity.seed_low) | (u64::from(identity.seed_high) << 32);
        for (_, weather) in SCENARIOS {
            let selection = Self::select_scenario(weather, seed)?;
            if EnvironmentIdentity::from(identity_for_selection(selection)) == identity {
                return Ok(Some(weather));
            }
        }
        Ok(None)
    }

    /// Returns the exact software profile used to seal this scenario.
    pub const fn controller_profile(&self) -> TailControlProfile {
        self.controller_profile
    }

    /// Returns the exact course axis used to seal the distance-scoring contract.
    pub const fn course_axis(&self) -> CourseAxis {
        self.course_axis
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

fn scenario_entries() -> Result<[ScenarioCatalogEntry; 5], HybridSessionPreparationError> {
    let model_version = registered_aircraft()
        .map_err(HybridSessionPreparationError::Aircraft)?
        .document()
        .model_version;
    Ok(SCENARIOS.map(|(version, weather)| ScenarioCatalogEntry {
        scenario_id: version,
        scenario_version: SCENARIO_VERSION,
        aircraft_model_version: model_version,
        environment_version: version,
        weather,
    }))
}

/// Returns the sealed core identity corresponding to a registered catalog selection.
pub fn identity_for_selection(selection: ScenarioSelection) -> SessionScenarioIdentity {
    SessionScenarioIdentity {
        catalog_version: selection.catalog_version,
        scenario_id: selection.scenario_id,
        scenario_version: selection.scenario_version,
        aircraft_model_version: selection.aircraft_model_version,
        environment_version: selection.environment_version,
        controller_profile_version: CONTROLLER_PROFILE_VERSION,
        seed: selection.seed,
    }
}

/// Returns the single cached owner decoded and validated from the registered aircraft JSON.
pub fn registered_aircraft() -> Result<&'static AircraftDefinition, AircraftDefinitionError> {
    AIRCRAFT_DEFINITION
        .get_or_init(playable_aircraft)
        .as_ref()
        .map_err(|error| *error)
}

fn cached_surfaces() -> Result<&'static [HybridSurface<'static>; 3], HybridSessionPreparationError>
{
    let definition = registered_aircraft().map_err(HybridSessionPreparationError::Aircraft)?;
    HYBRID_SURFACES
        .get_or_init(|| definition.surfaces())
        .as_ref()
        .map_err(|error| HybridSessionPreparationError::Aircraft(*error))
}

#[cfg(test)]
mod tests;
