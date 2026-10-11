use super::*;
use crate::{AeroError, AerodynamicStage, HybridLimit, HybridSite};

const UNIFORM_WIND_MPS: [f64; 3] = [0.3, 0.1, -0.04];
const WIND_GRADIENT_PER_METER: [[f64; 3]; 3] =
    [[0.0, 0.0, 0.015], [0.012, 0.0, 0.0], [0.0, -0.003, 0.0]];
const UPPER_ALPHA_MARGIN_RAD: f64 = 0.002;
const OUTSIDE_ALPHA_INCREMENT_RAD: f64 = 1.0e-6;
const NEAR_BOUND_OBSERVATION_INTERVALS: usize = 5;
const AIRSPEED_MPS: f64 = 9.7;
const NEAR_BOUND_PITCH_GAIN_SECONDS: f64 = 0.1;

#[derive(Clone, Copy, Debug)]
enum WindCase {
    Uniform,
    SpatialShear,
    UpperStaticAlpha,
}

impl WindCase {
    fn control_profile(self) -> TailControlProfile {
        let pitch_gain = match self {
            Self::Uniform | Self::SpatialShear => 0.2,
            Self::UpperStaticAlpha => NEAR_BOUND_PITCH_GAIN_SECONDS,
        };
        TailControlProfile::try_new(pitch_gain, 0.2, 1.0).unwrap()
    }

    fn wind(self) -> WindField<'static> {
        let base = NedVector::try_new(
            UNIFORM_WIND_MPS[0],
            UNIFORM_WIND_MPS[1],
            UNIFORM_WIND_MPS[2],
        )
        .unwrap();
        match self {
            Self::Uniform => WindField::uniform(base),
            Self::SpatialShear => WindField::linear_gradient(
                NedPoint::try_new(0.0, 0.0, -50.0).unwrap(),
                base,
                WIND_GRADIENT_PER_METER,
            )
            .unwrap(),
            Self::UpperStaticAlpha => WindField::uniform(NedVector::zero()),
        }
    }

    fn profile(self, definition: &HybridMockDefinition) -> TestProfile {
        match self {
            Self::Uniform | Self::SpatialShear => TestProfile::baseline(InputCase::SmoothChanged),
            Self::UpperStaticAlpha => {
                let polar = definition.polar().unwrap();
                let upper = polar.alpha_interval_rad()[1];
                let last_segment_start = polar.rows()[polar.rows().len() - 2].alpha_rad();
                assert!(upper - UPPER_ALPHA_MARGIN_RAD > last_segment_start);
                TestProfile {
                    input_case: InputCase::Neutral,
                    observation_intervals: NEAR_BOUND_OBSERVATION_INTERVALS,
                    datum_alpha_interval: [last_segment_start, upper],
                }
            }
        }
    }
}

fn state_at_static_alpha(
    definition: &HybridMockDefinition,
    trim: HybridMockTrim,
    alpha: f64,
) -> TailFlightTickState {
    let steady = trim
        .initial_state_for_ground_launch(NedPoint::try_new(0.0, 0.0, -50.0).unwrap(), 0.0)
        .unwrap();
    let body_velocity = BodyVector::try_new(
        AIRSPEED_MPS * libm::cos(alpha),
        0.0,
        AIRSPEED_MPS * libm::sin(alpha),
    )
    .unwrap();
    let state = FlightState::try_new(
        steady.datum_position_ned(),
        steady
            .attitude_body_to_ned()
            .body_to_ned(body_velocity)
            .unwrap(),
        steady.attitude_body_to_ned(),
        BodyVector::zero(),
        steady.pilot_position_m(),
        0.0,
    )
    .unwrap();
    TailFlightTickState::try_new(
        &definition.aircraft(),
        0,
        state,
        TailIncidence::neutral(),
        trim.pilot_mapping().unwrap().trim_target(),
    )
    .unwrap()
}

fn check_wind_step_halving(cadence: Cadence) {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let aircraft = definition.aircraft();
    for wind_case in [
        WindCase::Uniform,
        WindCase::SpatialShear,
        WindCase::UpperStaticAlpha,
    ] {
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            HybridMockTrim::AIR_DENSITY_KG_M3,
            wind_case.wind(),
        )
        .unwrap();
        let profile = wind_case.profile(&definition);
        for mode in [
            ControlMode::Manual,
            ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
            ControlMode::Automatic,
        ] {
            let config = TailFlightTickConfig::new(
                mode,
                wind_case.control_profile(),
                trim.pilot_mapping().unwrap(),
                Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
            );
            let initial = match wind_case {
                WindCase::Uniform | WindCase::SpatialShear => {
                    initial_state(&definition, trim, config)
                }
                WindCase::UpperStaticAlpha => state_at_static_alpha(
                    &definition,
                    trim,
                    definition.polar().unwrap().alpha_interval_rad()[1] - UPPER_ALPHA_MARGIN_RAD,
                ),
            };
            loads
                .evaluate_hybrid(&initial.flight_state, initial.incidence)
                .unwrap();
            verify_step_halving(&aircraft, initial, config, profile, cadence, &loads);
        }
    }
}

#[test]
fn uniform_shear_and_upper_alpha_use_physics_only_step_halving_gates() {
    check_wind_step_halving(Cadence::PhysicsOnly);
}

#[test]
fn uniform_shear_and_upper_alpha_use_coupled_step_halving_gates() {
    check_wind_step_halving(Cadence::Coupled);
}

#[test]
fn outside_static_alpha_is_a_typed_atomic_first_stage_refusal() {
    let definition = HybridMockDefinition::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let trim = HybridMockTrim::try_new(&definition).unwrap();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
        HybridMockTrim::AIR_DENSITY_KG_M3,
        WindField::uniform(NedVector::zero()),
    )
    .unwrap();
    let initial = state_at_static_alpha(
        &definition,
        trim,
        definition.polar().unwrap().alpha_interval_rad()[1] + OUTSIDE_ALPHA_INCREMENT_RAD,
    );
    let before = initial;
    for mode in [
        ControlMode::Manual,
        ControlMode::Shared(FbwAuthority::try_new(0.5).unwrap()),
        ControlMode::Automatic,
    ] {
        let config = TailFlightTickConfig::new(
            mode,
            TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap(),
            trim.pilot_mapping().unwrap(),
            Gravity::try_new(HybridMockTrim::GRAVITY_MPS2).unwrap(),
        );
        let error = advance_tail_flight_tick(
            &definition.aircraft(),
            initial,
            config,
            InputCase::Neutral.sample(0),
            &loads,
        )
        .unwrap_err();
        let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::Hybrid(error),
        ))) = error
        else {
            panic!("expected the original static polar error");
        };
        assert_eq!(error.site(), HybridSite::StaticPolar);
        assert_eq!(error.cause(), AeroError::OutsideEnvelope);
        assert_eq!(error.limit(), Some(HybridLimit::StaticAlpha));
        assert_eq!(error.stage(), Some(AerodynamicStage::First));
        assert_eq!(initial, before);
    }
}
