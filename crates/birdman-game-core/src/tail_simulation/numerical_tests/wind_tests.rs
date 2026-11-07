use super::*;

const UNIFORM_WIND_MPS: [f64; 3] = [0.3, 0.1, -0.04];
const WIND_GRADIENT_PER_METER: [[f64; 3]; 3] =
    [[0.0, 0.0, 0.015], [0.012, 0.0, 0.0], [0.0, -0.003, 0.0]];
const UPPER_ALPHA_MARGIN_RAD: f64 = 0.002;
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
        crate::tail_control::test_profile(pitch_gain, 0.2)
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

    fn profile(self, definition: &TestAircraft) -> TestProfile {
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
    definition: &TestAircraft,
    trim: SteadyGlideTrim,
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
    let definition = TestAircraft::try_new().unwrap();
    let surfaces = definition.surfaces().unwrap();
    let trim = definition.trim().unwrap();
    let aircraft = definition.aircraft();
    for wind_case in [
        WindCase::Uniform,
        WindCase::SpatialShear,
        WindCase::UpperStaticAlpha,
    ] {
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            TestAircraft::TRIM_AIR_DENSITY_KG_M3,
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
                Gravity::try_new(TestAircraft::TRIM_GRAVITY_MPS2).unwrap(),
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
