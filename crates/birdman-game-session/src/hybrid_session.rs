use std::sync::OnceLock;

use birdman_game_core::{
    ActuatorError, BodyPoint, BodyVector, CompositeCgLaunchConditions, ControlMode, CourseAxis,
    DistanceScoreError, DynamicsError, GameSessionConfiguration, GameSessionError, Gravity,
    HybridAerodynamicLoad, HybridError, HybridMockConfiguration, HybridMockDefinition,
    HybridMockError, HybridMockTrim, HybridModel, HybridSurface, MathError, NedPoint, NedVector,
    SessionScenarioIdentity, TailAngleOfAttackGuard, TailControlProfile, TailFlightScenario,
    TailFlightScenarioError, TailFlightScenarioParameters, TailIncidence, WindField,
};
use birdman_game_format::{
    ConfigurationError, EnvironmentFormatError, FlightRecordTailIdentityDocument, ScenarioCatalog,
    ScenarioCatalogEntry, ScenarioSelection, WeatherClass,
};

use crate::{
    DEFAULT_CONTROL_MODE, DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, DEFAULT_WEATHER,
    LaunchVenueError,
    environment::{bundled_environment, legacy_wind_for_version},
    launch_venue,
};

const CONTROLLER_PROFILE_ID: &str = "bpg040-tail-rate-feedback";
const CONTROLLER_PROFILE_VERSION: u32 = 3;
const CATALOG_VERSION: u32 = 3;
const SCENARIO_VERSION: u32 = 3;
const SCENARIOS: [ScenarioCatalogEntry; 5] = [
    entry(1, WeatherClass::Calm),
    entry(2, WeatherClass::Mild),
    entry(4, WeatherClass::Challenging),
    entry(5, WeatherClass::NearLimit),
    entry(6, WeatherClass::Typical),
];
const CONTACT_POINTS: [BodyPoint; 1] = [BodyPoint::origin()];
static HYBRID_DEFINITION: OnceLock<Result<HybridMockDefinition, HybridMockError>> = OnceLock::new();
static HYBRID_SURFACES: OnceLock<Result<[HybridSurface<'static>; 3], HybridMockError>> =
    OnceLock::new();

/// Typed preparation failures before any hybrid session state or record is published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridSessionPreparationError {
    /// The shared launch venue configuration failed validation.
    Venue(LaunchVenueError),
    /// The requested weather has no matching registered scenario or model identity.
    Configuration(ConfigurationError),
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
            DEFAULT_CONTROL_MODE,
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

    /// Resolves Weather against catalog three and uses the matching registered wind provider.
    pub fn try_new_for_weather(
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        seed: u64,
        weather: WeatherClass,
    ) -> Result<Self, HybridSessionPreparationError> {
        let selection = Self::select_scenario(weather, seed)?;
        let definition = cached_definition()?;
        let surfaces = cached_surfaces()?;
        let wind = if selection.environment_version == 6 {
            bundled_environment()
                .map_err(HybridSessionPreparationError::Environment)?
                .wind_field()
                .map_err(HybridSessionPreparationError::Environment)?
        } else {
            let velocity = legacy_wind_for_version(selection.environment_version).ok_or(
                HybridSessionPreparationError::Configuration(
                    ConfigurationError::ScenarioUnavailable,
                ),
            )?;
            WindField::uniform(
                NedVector::try_new(velocity[0], velocity[1], velocity[2])
                    .map_err(HybridSessionPreparationError::Math)?,
            )
        };
        let trim =
            HybridMockTrim::try_new(definition).map_err(HybridSessionPreparationError::Mock)?;
        let cg_position =
            NedPoint::try_new(0.0, 0.0, -10.5).map_err(HybridSessionPreparationError::Math)?;
        let platform = launch_venue()
            .map_err(HybridSessionPreparationError::Venue)?
            .platform;
        let air_state = trim
            .initial_state_for_ground_launch(cg_position, platform.heading_rad())
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
        let [course_north, course_east] = platform.horizontal_direction_ned();
        let course_axis = CourseAxis::try_new(course_north, course_east)
            .map_err(HybridSessionPreparationError::Course)?;
        let parameters = TailFlightScenarioParameters::try_new(
            definition.aircraft(),
            launch,
            TailIncidence::neutral(),
            Gravity::try_new(HybridMockTrim::GRAVITY_MPS2)
                .map_err(HybridSessionPreparationError::Dynamics)?,
            &CONTACT_POINTS,
            course_axis,
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
            aircraft_configuration_id: definition.configuration().configuration_id().to_owned(),
            controller_profile_id: CONTROLLER_PROFILE_ID.to_owned(),
        };
        Ok(Self {
            configuration,
            record_identity,
            controller_profile,
            course_axis,
        })
    }

    /// Selects registered catalog-three identities without constructing a flight state.
    pub fn select_scenario(
        weather: WeatherClass,
        seed: u64,
    ) -> Result<ScenarioSelection, HybridSessionPreparationError> {
        ScenarioCatalog::try_new(CATALOG_VERSION, &SCENARIOS)
            .and_then(|catalog| catalog.select(weather, seed))
            .map_err(HybridSessionPreparationError::Configuration)
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

const fn entry(version: u32, weather: WeatherClass) -> ScenarioCatalogEntry {
    ScenarioCatalogEntry {
        scenario_id: version,
        scenario_version: SCENARIO_VERSION,
        aircraft_model_version: HybridMockConfiguration::Playable.model_version(),
        environment_version: version,
        weather,
    }
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

fn cached_definition() -> Result<&'static HybridMockDefinition, HybridSessionPreparationError> {
    HYBRID_DEFINITION
        .get_or_init(|| HybridMockDefinition::try_new(HybridMockConfiguration::Playable))
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
