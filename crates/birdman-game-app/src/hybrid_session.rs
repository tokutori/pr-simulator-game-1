use birdman_game_content::{AircraftDefinition, AircraftDefinitionError};
use birdman_game_core::{
    ActuatorError, BodyVector, CompositeCgLaunchConditions, ControlMode, CourseAxis,
    DistanceScoreError, DynamicsError, GameSessionConfiguration, GameSessionError, Gravity,
    HybridAerodynamicLoad, HybridError, HybridModel, MathError, NedPoint, NedVector,
    SessionScenarioIdentity, SteadyGlideTrim, TailControlProfile, TailFlightScenario,
    TailFlightScenarioError, TailFlightScenarioParameters, TailIncidence, WindField,
};
use birdman_game_format::{
    ConfigurationError, EnvironmentFormatError, FlightRecordTailIdentityDocument, ScenarioCatalog,
    ScenarioCatalogEntry, ScenarioSelection, WeatherClass,
};

use crate::{
    DEFAULT_MAXIMUM_FLIGHT_TICKS, DEFAULT_SESSION_SEED, DEFAULT_WEATHER, LaunchPlatform,
    LaunchVenueError,
    environment::{bundled_environment, preset_wind_for_version},
    environment_snapshot::EnvironmentIdentity,
    launch_venue,
};

const CATALOG_VERSION: u32 = 10;
const SCENARIO_VERSION: u32 = 11;
const SCENARIOS: [(u32, WeatherClass); 5] = [
    (1, WeatherClass::Calm),
    (2, WeatherClass::Mild),
    (4, WeatherClass::Challenging),
    (5, WeatherClass::NearLimit),
    (6, WeatherClass::Typical),
];
/// Typed preparation failures before any hybrid session state or record is published.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Controller source or the current aircraft/controller pair was rejected.
    Controller(crate::ControllerPreparationError),
    /// The sealed scenario rejected its initial physical or load state.
    Scenario(TailFlightScenarioError),
    /// The session tick limit or versioned identity failed validation.
    Session(GameSessionError),
    /// Structural storage could not be reserved before preparing flight.
    AllocationFailed,
}
#[cfg(not(verus_only))]
impl core::fmt::Display for HybridSessionPreparationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Trim(_) => f.write_str("Launch trim could not be converted to an initial state"),
            Self::Venue(_) => f.write_str("Launch venue preparation failed"),
            Self::Configuration(_) => f.write_str("Registered scenario selection failed"),
            Self::Aircraft(_) => f.write_str("Registered aircraft preparation failed"),
            Self::Environment(_) => f.write_str("Offline environment preparation failed"),
            Self::Math(_) => f.write_str("Launch coordinate validation failed"),
            Self::Dynamics(_) => f.write_str("Initial physical state validation failed"),
            Self::Course(_) => f.write_str("Scoring course validation failed"),
            Self::Hybrid(_) => f.write_str("Hybrid load model preparation failed"),
            Self::Control(_) => f.write_str("Software controller preparation failed"),
            Self::Controller(cause) => write!(f, "{cause}"),
            Self::Scenario(_) => f.write_str("Sealed flight scenario validation failed"),
            Self::Session(_) => f.write_str("Versioned session configuration failed"),
            Self::AllocationFailed => f.write_str("Flight structural storage allocation failed"),
        }
    }
}

#[cfg(not(verus_only))]
impl core::error::Error for HybridSessionPreparationError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Trim(cause) => Some(cause),
            Self::Venue(cause) => Some(cause),
            Self::Configuration(cause) => Some(cause),
            Self::Aircraft(cause) => Some(cause),
            Self::Environment(cause) => Some(cause),
            Self::Math(cause) => Some(cause),
            Self::Dynamics(cause) => Some(cause),
            Self::Course(cause) => Some(cause),
            Self::Hybrid(cause) => Some(cause),
            Self::Control(cause) => Some(cause),
            Self::Scenario(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Platform-independent preparation with stable owned model and environment resources.
///
/// The numerical configuration owns its launch values. Aerodynamics borrow the
/// selected definition only during preparation and execution batches.
pub struct HybridSessionPreparation {
    configuration: GameSessionConfiguration,
    record_identity: FlightRecordTailIdentityDocument,
    controller_profile: TailControlProfile,
    course_axis: CourseAxis,
    wind: WindField<'static>,
}

impl HybridSessionPreparation {
    /// Builds the same default configuration selected by the public Web application.
    pub fn try_default() -> Result<Self, HybridSessionPreparationError> {
        let aircraft =
            crate::SelectedAircraft::bundled().map_err(HybridSessionPreparationError::Aircraft)?;
        let controller = crate::SelectedController::bundled().map_err(|cause| {
            HybridSessionPreparationError::Controller(crate::ControllerPreparationError::Document(
                cause,
            ))
        })?;
        let mode = crate::application::commands::control_mode(
            crate::DEFAULT_ASSISTANCE,
            controller.document().shared_authority,
        )
        .map_err(HybridSessionPreparationError::Control)?;
        Self::try_new(
            &aircraft,
            &controller,
            mode,
            DEFAULT_MAXIMUM_FLIGHT_TICKS,
            DEFAULT_SESSION_SEED,
        )
    }

    /// Seals an explicitly selected aircraft into an owned numerical configuration.
    ///
    /// ```
    /// use birdman_game_app::{HybridSessionPreparation, SelectedAircraft, SelectedController};
    /// use birdman_game_core::{ControlMode, GameSession, SessionPhase};
    /// let selected = SelectedAircraft::bundled().unwrap();
    /// let controller = SelectedController::bundled().unwrap();
    /// let (configuration, identity) = HybridSessionPreparation::try_new(
    ///     &selected, &controller, ControlMode::Manual, 2, 42,
    /// ).unwrap().into_parts();
    /// assert_eq!(identity.aircraft_sha256.as_deref(), Some(selected.raw_sha256()));
    /// drop(selected);
    /// let mut session = GameSession::new();
    /// session.open_setup().unwrap();
    /// session.prepare_flight(configuration).unwrap();
    /// session.mark_briefing_ready().unwrap();
    /// session.start_countdown(1).unwrap();
    /// session.observe_countdown(session.countdown_token().unwrap(), 1.0, true).unwrap();
    /// assert_eq!(session.snapshot().phase(), SessionPhase::FlightRunning);
    /// ```
    pub fn try_new(
        aircraft: &crate::SelectedAircraft,
        controller: &crate::SelectedController,
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        seed: u64,
    ) -> Result<Self, HybridSessionPreparationError> {
        Self::try_new_for_weather(
            aircraft,
            controller,
            control_mode,
            maximum_flight_ticks,
            seed,
            DEFAULT_WEATHER,
        )
    }

    /// Resolves Weather against the current catalog and its registered wind provider.
    pub fn try_new_for_weather(
        aircraft: &crate::SelectedAircraft,
        controller: &crate::SelectedController,
        control_mode: ControlMode,
        maximum_flight_ticks: u64,
        seed: u64,
        weather: WeatherClass,
    ) -> Result<Self, HybridSessionPreparationError> {
        let definition = aircraft.definition();
        let selection = Self::select_scenario(weather, seed, definition.document().model_version)?;
        let surfaces = definition
            .surfaces()
            .map_err(HybridSessionPreparationError::Aircraft)?;
        let wind = wind_for_version(selection.environment_version)?;
        let trim = definition
            .trim()
            .map_err(HybridSessionPreparationError::Aircraft)?;
        let cg_position = aircraft.launch_position()?;
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
                &surfaces,
            )
            .map_err(HybridSessionPreparationError::Hybrid)?,
            definition.document().trim.air_density_kg_m3,
            wind,
        )
        .map_err(HybridSessionPreparationError::Hybrid)?;
        let controller_profile = controller
            .prepare(definition)
            .map_err(HybridSessionPreparationError::Controller)?;
        let scenario = TailFlightScenario::try_new(parameters, load, controller_profile)
            .map_err(HybridSessionPreparationError::Scenario)?;
        let mut identity = identity_for_selection(selection);
        identity.aircraft_identity = aircraft.identity();
        identity.controller_identity = Some(controller.identity());
        let configuration = GameSessionConfiguration::try_new_tail(
            scenario,
            control_mode,
            maximum_flight_ticks,
            identity,
        )
        .map_err(HybridSessionPreparationError::Session)?;
        let record_identity = FlightRecordTailIdentityDocument {
            aircraft_sha256: Some(aircraft.raw_sha256().to_owned()),
            aircraft_configuration_id: definition.document().configuration_id.clone(),
            controller_profile_id: controller.document().controller_id.clone(),
            controller_sha256: Some(controller.raw_sha256().to_owned()),
        };
        Ok(Self {
            configuration,
            record_identity,
            controller_profile,
            course_axis,
            wind,
        })
    }

    /// Selects current registered identities without constructing a flight state.
    pub fn select_scenario(
        weather: WeatherClass,
        seed: u64,
        aircraft_model_version: u32,
    ) -> Result<ScenarioSelection, HybridSessionPreparationError> {
        let entries = scenario_entries(aircraft_model_version);
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
            if Self::environment_identity(weather, seed)? == identity {
                return Ok(Some(weather));
            }
        }
        Ok(None)
    }

    /// Selects environmental resources independently of aircraft and controller identity.
    pub fn environment_identity(
        weather: WeatherClass,
        seed: u64,
    ) -> Result<EnvironmentIdentity, HybridSessionPreparationError> {
        let version = SCENARIOS
            .iter()
            .find(|(_, candidate)| *candidate == weather)
            .map(|(version, _)| *version)
            .ok_or(HybridSessionPreparationError::Configuration(
                ConfigurationError::ScenarioUnavailable,
            ))?;
        Ok(EnvironmentIdentity {
            catalog_version: CATALOG_VERSION,
            scenario_id: version,
            scenario_version: SCENARIO_VERSION,
            environment_version: version,
            seed_low: seed as u32,
            seed_high: (seed >> 32) as u32,
        })
    }

    /// Returns the exact software profile used to seal this scenario.
    pub const fn controller_profile(&self) -> TailControlProfile {
        self.controller_profile
    }

    /// Returns the sealed environmental load resource.
    pub const fn wind(&self) -> WindField<'static> {
        self.wind
    }

    /// Returns the exact course axis used to seal the distance-scoring contract.
    pub const fn course_axis(&self) -> CourseAxis {
        self.course_axis
    }

    /// Moves the sealed core configuration and its definition-derived archive identity to adapters.
    pub fn into_parts(self) -> (GameSessionConfiguration, FlightRecordTailIdentityDocument) {
        (self.configuration, self.record_identity)
    }
}

fn scenario_entries(model_version: u32) -> [ScenarioCatalogEntry; 5] {
    SCENARIOS.map(|(version, weather)| ScenarioCatalogEntry {
        scenario_id: version,
        scenario_version: SCENARIO_VERSION,
        aircraft_model_version: model_version,
        environment_version: version,
        weather,
    })
}

/// Returns the sealed core identity corresponding to a registered catalog selection.
pub fn identity_for_selection(selection: ScenarioSelection) -> SessionScenarioIdentity {
    SessionScenarioIdentity {
        catalog_version: selection.catalog_version,
        scenario_id: selection.scenario_id,
        scenario_version: selection.scenario_version,
        aircraft_model_version: selection.aircraft_model_version,
        aircraft_identity: [0; 32],
        environment_version: selection.environment_version,
        controller_identity: None,
        seed: selection.seed,
    }
}

/// Derives the launch support position from the selected physical hull.
pub fn launch_position(
    definition: &AircraftDefinition,
) -> Result<NedPoint, HybridSessionPreparationError> {
    let trim = definition
        .trim()
        .map_err(HybridSessionPreparationError::Aircraft)?;
    let platform = launch_venue()
        .map_err(HybridSessionPreparationError::Venue)?
        .platform;
    launch_cg_position(definition, trim, platform)
}

/// Borrows the environment resource independently of aircraft selection.
pub(crate) fn wind_for_version(
    version: u32,
) -> Result<WindField<'static>, HybridSessionPreparationError> {
    if version == 6 {
        return bundled_environment()
            .map_err(HybridSessionPreparationError::Environment)?
            .wind_field()
            .map_err(HybridSessionPreparationError::Environment);
    }
    let velocity = preset_wind_for_version(version).ok_or(
        HybridSessionPreparationError::Configuration(ConfigurationError::ScenarioUnavailable),
    )?;
    Ok(WindField::uniform(
        NedVector::try_new(velocity[0], velocity[1], velocity[2])
            .map_err(HybridSessionPreparationError::Math)?,
    ))
}

/// Clears the complete validated aircraft hull from the deck's supporting plane.
/// Every triangle interior satisfies the same affine half-space constraint.
fn launch_cg_position(
    definition: &AircraftDefinition,
    trim: SteadyGlideTrim,
    platform: LaunchPlatform,
) -> Result<NedPoint, HybridSessionPreparationError> {
    let reference = trim
        .initial_state_for_ground_launch(NedPoint::origin(), platform.heading_rad())
        .map_err(HybridSessionPreparationError::Trim)?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(definition.contact_points().len())
        .map_err(|_| HybridSessionPreparationError::AllocationFailed)?;
    points.extend_from_slice(definition.contact_points());
    definition
        .write_deformed_contact_points(
            definition.document().trim.airspeed_mps,
            TailIncidence::neutral(),
            &mut points,
        )
        .map_err(|error| {
            HybridSessionPreparationError::Aircraft(AircraftDefinitionError::Shape(error))
        })?;
    let [north, east] = platform.horizontal_direction_ned();
    let slope = platform.downward_slope_degrees.to_radians().tan();
    let mut altitude = f64::NEG_INFINITY;
    for point in points {
        let [x, y, z] = point.components();
        let offset = reference
            .attitude_body_to_ned()
            .body_to_ned(BodyVector::try_new(x, y, z).map_err(HybridSessionPreparationError::Math)?)
            .map_err(HybridSessionPreparationError::Math)?;
        let [n, e, down] = reference
            .datum_position_ned()
            .translated(offset)
            .map_err(HybridSessionPreparationError::Math)?
            .components();
        let required =
            platform.front_lip_above_water_meters + down - (n * north + e * east) * slope;
        if !required.is_finite() {
            return Err(HybridSessionPreparationError::Math(MathError::NonFinite));
        }
        altitude = altitude.max(required);
    }
    NedPoint::try_new(0.0, 0.0, -altitude).map_err(HybridSessionPreparationError::Math)
}

#[cfg(test)]
mod tests;
