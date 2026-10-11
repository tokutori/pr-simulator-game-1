use birdman_game_core::{
    ControlMode, FbwAuthority, GameSession, GameSessionConfiguration, GameSessionError,
    SessionPhase, TailFlightTickInput, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionIntent, TailRateTarget,
};
use birdman_game_format::{
    AssistanceLevel, DifficultySettings, FlightRecordHeaderDocument, FlightRecordStateDocument,
    FlightRecordTailIdentityDocument, InformationLevel, TailFlightRecordControlsDocument,
    TailFlightRecordDocument, TailFlightRecordFinalizationDocument, WeatherClass,
};
use birdman_game_session::HybridSessionPreparation;
use serde::Serialize;

const TICK_LIMIT: u64 = 8;
#[cfg(test)]
const CONTROLLER_ID: &str = "bpg040-tail-rate-feedback";

pub(super) fn run_verification(requested_mode: &str) -> Result<(), String> {
    let modes = [
        ("manual", ControlMode::Manual),
        (
            "shared",
            ControlMode::Shared(FbwAuthority::try_new(0.5).map_err(super::display_error)?),
        ),
        ("automatic", ControlMode::Automatic),
    ];
    if requested_mode != "all" && !modes.iter().any(|(name, _)| *name == requested_mode) {
        return Err(super::usage());
    }
    for (name, mode) in modes {
        if requested_mode != "all" && name != requested_mode {
            continue;
        }
        let first = verify_mode(mode)?;
        let repeated = verify_mode(mode)?;
        if first != repeated {
            return Err(format!("{name} hybrid smoke record was not deterministic"));
        }
        println!("{}", named_terminal_json(name, &first)?);
    }
    Ok(())
}

fn input(tick: u64) -> Result<TailFlightTickInput, String> {
    Ok(TailFlightTickInput::new(
        TailPilotIntent::try_new(0.25, 0.25).map_err(super::display_error)?,
        TailRateTarget::try_new(0.02, 0.02).map_err(super::display_error)?,
        if tick == 0 {
            TailPilotPositionCommand::Set(
                TailPilotPositionIntent::try_new(0.5).map_err(super::display_error)?,
            )
        } else {
            TailPilotPositionCommand::Hold
        },
    ))
}

fn verify_mode(mode: ControlMode) -> Result<TailFlightRecordDocument, String> {
    let preparation =
        HybridSessionPreparation::try_new_for_weather(mode, TICK_LIMIT, 0, WeatherClass::Calm)
            .map_err(super::display_error)?;
    let (configuration, identity) = preparation.into_parts();
    run_configuration(mode, configuration, identity)
}

fn run_configuration(
    mode: ControlMode,
    configuration: GameSessionConfiguration<'_>,
    record_identity: FlightRecordTailIdentityDocument,
) -> Result<TailFlightRecordDocument, String> {
    let mut session = GameSession::new();
    session.open_setup().map_err(super::display_error)?;
    session
        .prepare_flight(configuration)
        .map_err(super::display_error)?;
    session
        .mark_briefing_ready()
        .map_err(super::display_error)?;
    session.start_countdown(1).map_err(super::display_error)?;
    session.advance_countdown().map_err(super::display_error)?;
    session.launch().map_err(super::display_error)?;
    for tick in 0..TICK_LIMIT {
        match session.advance_tail_flight_tick(input(tick)?) {
            Ok(_) => {}
            Err(GameSessionError::TailTick(_))
                if session.snapshot().phase() == SessionPhase::Result =>
            {
                break;
            }
            Err(error) => return Err(super::display_error(error)),
        }
    }
    if session.snapshot().phase() != SessionPhase::Result {
        return Err("hybrid smoke did not finalize the configured tick sequence".to_owned());
    }
    let record = session
        .flight_record()
        .ok_or_else(|| "hybrid smoke record is missing".to_owned())?;
    let settings = DifficultySettings::custom(
        InformationLevel::Full,
        match mode {
            ControlMode::Manual => AssistanceLevel::Manual,
            ControlMode::Shared(_) => AssistanceLevel::Assisted,
            ControlMode::Automatic => AssistanceLevel::Strong,
        },
        WeatherClass::Calm,
    );
    let document = TailFlightRecordDocument::from_record(record, settings, record_identity)
        .map_err(super::display_error)?;
    let archive = TailFlightRecordDocument::decode_json(
        &document.encode_json().map_err(super::display_error)?,
    )
    .map_err(super::display_error)?;
    let restored = archive
        .to_finalized_core_record()
        .map_err(super::display_error)?;
    if restored.finalization() != record.finalization()
        || restored
            .samples()
            .iter()
            .zip(record.samples())
            .any(|(saved, original)| {
                saved.controls != original.controls || saved.telemetry != original.telemetry
            })
    {
        return Err(
            "hybrid smoke saved controls or terminal metadata changed during export".to_owned(),
        );
    }
    Ok(document)
}

fn named_terminal_json(mode: &str, document: &TailFlightRecordDocument) -> Result<String, String> {
    #[derive(Serialize)]
    struct Terminal<'document> {
        kind: &'static str,
        mode: &'document str,
        record_schema_version: u32,
        header: &'document FlightRecordHeaderDocument,
        control_identity: &'document FlightRecordTailIdentityDocument,
        sample_count: usize,
        state: &'document FlightRecordStateDocument,
        controls: &'document TailFlightRecordControlsDocument,
        finalization: TailFlightRecordFinalizationDocument,
    }
    let last = document
        .samples
        .last()
        .ok_or_else(|| "hybrid smoke record is empty".to_owned())?;
    serde_json::to_string(&Terminal {
        kind: "native_hybrid_smoke",
        mode,
        record_schema_version: document.schema_version,
        header: &document.header,
        control_identity: &document.control_identity,
        sample_count: document.samples.len(),
        state: &last.state,
        controls: &last.controls,
        finalization: document.finalization,
    })
    .map_err(super::display_error)
}

#[cfg(test)]
fn verify_mode_at_speed(
    mode: ControlMode,
    speed_mps: f64,
) -> Result<TailFlightRecordDocument, String> {
    use birdman_game_core::{
        BodyPoint, BodyVector, CompositeCgLaunchConditions, CourseAxis, Gravity,
        HybridAerodynamicLoad, HybridMockDefinition, HybridMockTrim, HybridModel, NedPoint,
        NedVector, SessionScenarioIdentity, TailControlProfile, TailFlightScenario,
        TailFlightScenarioParameters, TailIncidence, WindField,
    };
    const CONTACT_POINTS: [BodyPoint; 1] = [BodyPoint::origin()];
    let definition = HybridMockDefinition::try_new().map_err(super::display_error)?;
    let surfaces = definition.surfaces().map_err(super::display_error)?;
    let trim = HybridMockTrim::try_new(&definition).map_err(super::display_error)?;
    let cg_position = NedPoint::try_new(0.0, 0.0, -10.5).map_err(super::display_error)?;
    let initial = trim
        .initial_state_for_ground_launch(cg_position, 0.0)
        .map_err(super::display_error)?;
    let launch = CompositeCgLaunchConditions::try_new(
        cg_position,
        initial
            .datum_velocity_ned()
            .scaled(speed_mps / HybridMockTrim::AIRSPEED_MPS)
            .map_err(super::display_error)?,
        initial.attitude_body_to_ned(),
        BodyVector::zero(),
        trim.pilot_position_m(),
        0.0,
    )
    .map_err(super::display_error)?;
    let parameters = TailFlightScenarioParameters::try_new(
        definition.aircraft(),
        launch,
        TailIncidence::neutral(),
        Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).map_err(super::display_error)?,
        &CONTACT_POINTS,
        CourseAxis::try_new(1.0, 0.0).map_err(super::display_error)?,
    )
    .map_err(super::display_error)?;
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().map_err(super::display_error)?, &surfaces)
            .map_err(super::display_error)?,
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .map_err(super::display_error)?;
    let scenario = TailFlightScenario::try_new(
        parameters,
        load,
        TailControlProfile::try_new(0.2, 0.2, 1.0).map_err(super::display_error)?,
    )
    .map_err(super::display_error)?;
    let identity = SessionScenarioIdentity {
        catalog_version: 3,
        scenario_id: 1,
        scenario_version: 3,
        aircraft_model_version: HybridMockDefinition::MODEL_VERSION,
        environment_version: 1,
        controller_profile_version: 3,
        seed: 0,
    };
    let configuration =
        GameSessionConfiguration::try_new_tail(scenario, mode, TICK_LIMIT, identity)
            .map_err(super::display_error)?;
    run_configuration(
        mode,
        configuration,
        FlightRecordTailIdentityDocument {
            aircraft_configuration_id: HybridMockDefinition::CONFIGURATION_ID.to_owned(),
            controller_profile_id: CONTROLLER_ID.to_owned(),
        },
    )
}

#[cfg(test)]
mod tests;
