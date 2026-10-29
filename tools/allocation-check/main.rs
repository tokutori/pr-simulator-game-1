use birdman_game_core::{
    AeroError, AerodynamicEvaluationError, AerodynamicLoadProvider, AerodynamicStage,
    AircraftModel, BodyPoint, BodyVector, ControlMode, DynamicsError, ElementOrientation,
    ElementReference, ExternalLoadProvider, FlightState, Gravity, HybridAerodynamicLoad,
    HybridAnchor, HybridLimit, HybridModel, HybridProxy, HybridSection, HybridSite, HybridSurface,
    HybridSurfaceGeometry, HybridSurfaceRole, InertiaTensor, LoadError, NedPoint, NedVector,
    PHYSICS_DT_SECONDS, PilotAcceleration, PilotPositionTarget, PlanformSymmetry,
    PolarAnalysisMethod, PolarMomentAxes, StaticPolar, StaticPolarCoefficients,
    StaticPolarMetadata, StaticPolarRow, TailAerodynamics, TailControlProfile,
    TailFlightTickConfig, TailFlightTickError, TailFlightTickInput, TailFlightTickOutcome,
    TailFlightTickState, TailIncidence, TailPilotIntent, TailPilotPositionCommand,
    TailPilotPositionIntent, TailPilotPositionMapping, TailRateTarget, UnitQuaternion,
    WaterContactGeometry, WindError, WindField, advance, advance_tail_flight_tick,
    advance_tail_flight_tick_with_contact,
};
use std::alloc::{GlobalAlloc, Layout, System, alloc, alloc_zeroed, dealloc, realloc};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOC: AtomicUsize = AtomicUsize::new(0);
static ZEROED: AtomicUsize = AtomicUsize::new(0);
static REALLOC: AtomicUsize = AtomicUsize::new(0);
const ITERATIONS: usize = 10_000;

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(&ALLOC);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(&ZEROED);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(&REALLOC);
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn count(counter: &AtomicUsize) {
    if MEASURING.load(Ordering::SeqCst) {
        counter.fetch_add(1, Ordering::SeqCst);
    }
}

fn begin() {
    ALLOC.store(0, Ordering::SeqCst);
    ZEROED.store(0, Ordering::SeqCst);
    REALLOC.store(0, Ordering::SeqCst);
    MEASURING.store(true, Ordering::SeqCst);
}

fn end() -> [usize; 3] {
    MEASURING.store(false, Ordering::SeqCst);
    [
        ALLOC.load(Ordering::SeqCst),
        ZEROED.load(Ordering::SeqCst),
        REALLOC.load(Ordering::SeqCst),
    ]
}

fn calibrate() {
    let layout = Layout::from_size_align(64, 8).unwrap();
    let larger = Layout::from_size_align(128, 8).unwrap();
    begin();
    let first = unsafe { alloc(black_box(layout)) };
    let zeroed = unsafe { alloc_zeroed(black_box(layout)) };
    let counts = end();
    assert!(!first.is_null() && !zeroed.is_null());
    assert_eq!(counts, [1, 1, 0]);
    begin();
    let resized = unsafe { realloc(black_box(first), black_box(layout), black_box(128)) };
    let counts = end();
    assert!(!resized.is_null());
    assert_eq!(counts, [0, 0, 1]);
    unsafe {
        dealloc(black_box(resized), larger);
        dealloc(black_box(zeroed), layout);
    }
    println!("calibration: alloc=1 alloc_zeroed=1 realloc=1");
}

fn registered_application_flight() {
    use birdman_game_app::application::{ApplicationSession, BackendReadiness, SessionCommand};
    use birdman_game_core::{SessionEndReason, SessionPhase};
    let mut application = ApplicationSession::default();
    for command in [
        SessionCommand::OpenSetup {},
        SessionCommand::Prepare {},
        SessionCommand::StartCountdown {},
    ] {
        application
            .execute(command, BackendReadiness::Ready)
            .unwrap();
    }
    let token = application.core_session().countdown_token().unwrap();
    application.observe_countdown(token, 3.0, true).unwrap();
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let mut failure = None;
    let mut ticks = 0;
    begin();
    for _ in 0..birdman_game_app::DEFAULT_MAXIMUM_FLIGHT_TICKS {
        ticks += 1;
        if let Err(error) = application.advance_tail_flight_tick(black_box(input)) {
            failure = Some(error);
            break;
        }
        if application.core_session().snapshot().phase() == SessionPhase::Result {
            break;
        }
    }
    let counts = end();
    assert_eq!(failure, None);
    assert_eq!(counts, [0, 0, 0]);
    let record = application.core_session().flight_record().unwrap();
    assert_eq!(
        record.finalization().unwrap().reason,
        SessionEndReason::WaterContact
    );
    assert!(
        record
            .samples()
            .iter()
            .all(|sample| sample.wing_bending_factor.is_some())
    );
    println!("registered application flight: ticks={ticks} alloc=0 alloc_zeroed=0 realloc=0");
}

fn measure<T>(name: &str, operation: impl FnMut() -> T) -> T {
    measure_iterations(name, ITERATIONS, operation)
}

fn measure_iterations<T>(name: &str, iterations: usize, mut operation: impl FnMut() -> T) -> T {
    let mut result = black_box(operation());
    begin();
    for _ in 0..iterations {
        result = black_box(operation());
    }
    let counts = end();
    assert_eq!(counts, [0; 3], "{name} allocated in the measured window");
    println!("{name}: iterations={iterations} alloc=0 alloc_zeroed=0 realloc=0");
    result
}

fn point(values: [f64; 3]) -> BodyPoint {
    BodyPoint::try_new(values[0], values[1], values[2]).unwrap()
}

fn body(values: [f64; 3]) -> BodyVector {
    BodyVector::try_new(values[0], values[1], values[2]).unwrap()
}

fn ned(values: [f64; 3]) -> NedVector {
    NedVector::try_new(values[0], values[1], values[2]).unwrap()
}

fn state(velocity: [f64; 3]) -> FlightState {
    FlightState::try_new(
        NedPoint::try_new(0.0, 0.0, -10.0).unwrap(),
        ned(velocity),
        UnitQuaternion::IDENTITY,
        body([0.01, 0.02, 0.01]),
        0.02,
        0.01,
    )
    .unwrap()
}

struct Fixture {
    sections: [[HybridSection; 2]; 3],
    proxies: [[HybridProxy; 4]; 3],
    rows: [StaticPolarRow; 3],
}

impl Fixture {
    fn new() -> Self {
        let sections = [
            [[0.0, -2.0, 0.0], [0.0, 2.0, 0.0]],
            [[-3.0, -1.0, 0.0], [-3.0, 1.0, 0.0]],
            [[-2.5, 0.0, -0.5], [-2.5, 0.0, 0.5]],
        ]
        .map(|edges| edges.map(|edge| HybridSection::try_new(point(edge), 0.5).unwrap()));
        let intervals = [[-2.0, 2.0], [-1.0, 1.0], [-0.5, 0.5]];
        let fin_frame = ElementOrientation::try_new(
            body([1.0, 0.0, 0.0]),
            body([0.0, 0.0, 1.0]),
            body([0.0, -1.0, 0.0]),
        )
        .unwrap();
        let proxies = core::array::from_fn(|surface| {
            let geometry = Self::geometry(&sections, surface);
            core::array::from_fn(|strip| {
                let station = |edge: usize| {
                    intervals[surface][0]
                        + (intervals[surface][1] - intervals[surface][0]) * edge as f64 / 4.0
                };
                HybridProxy::try_new(
                    geometry,
                    [station(strip), station(strip + 1)],
                    if surface == 2 {
                        fin_frame
                    } else {
                        ElementOrientation::IDENTITY
                    },
                    HybridAnchor::try_new(0.4, 0.02).unwrap(),
                )
                .unwrap()
            })
        });
        let rows = [-0.15, 0.0, 0.15].map(|alpha| {
            StaticPolarRow::try_new(
                alpha,
                StaticPolarCoefficients::try_new(
                    0.5 + 2.0 * alpha,
                    0.01 + alpha * alpha,
                    0.02,
                    0.0,
                    0.01,
                    -0.03,
                    0.02,
                )
                .unwrap(),
            )
            .unwrap()
        });
        Self {
            sections,
            proxies,
            rows,
        }
    }

    fn geometry(sections: &[[HybridSection; 2]; 3], index: usize) -> HybridSurfaceGeometry<'_> {
        HybridSurfaceGeometry::try_new(
            [
                HybridSurfaceRole::MainWing,
                HybridSurfaceRole::HorizontalTail,
                HybridSurfaceRole::VerticalTail,
            ][index],
            &sections[index],
            if index == 2 {
                PlanformSymmetry::Unrestricted
            } else {
                PlanformSymmetry::MirrorSpan
            },
        )
        .unwrap()
    }

    fn surfaces(&self) -> [HybridSurface<'_>; 3] {
        core::array::from_fn(|index| {
            let tail = (index != 0).then(|| TailAerodynamics::try_new(0.004, 0.7, 0.0).unwrap());
            HybridSurface::try_new(
                Self::geometry(&self.sections, index),
                &self.proxies[index],
                tail,
            )
            .unwrap()
        })
    }

    fn polar(&self) -> StaticPolar<'_> {
        StaticPolar::try_new(
            &self.rows,
            ElementReference::try_new(2.0, 4.0, 0.5).unwrap(),
            point([0.3, 0.0, 0.1]),
            PolarMomentAxes::BodyFrd,
            StaticPolarMetadata::try_new(
                PolarAnalysisMethod::SoftwareFixture,
                "fictional-hybrid-allocation-probe",
                1,
            )
            .unwrap(),
        )
        .unwrap()
    }
}

fn outside(error: LoadError, stage: Option<AerodynamicStage>) {
    let LoadError::Aerodynamic(AerodynamicEvaluationError::Hybrid(error)) = error else {
        panic!("expected a typed hybrid error");
    };
    assert_eq!(error.cause(), AeroError::OutsideEnvelope);
    assert_eq!(error.site(), HybridSite::Datum);
    assert_eq!(error.limit(), Some(HybridLimit::GlobalBeta));
    assert_eq!(error.stage(), stage);
}

fn outside_dynamics(error: DynamicsError) {
    let DynamicsError::Load(error) = error else {
        panic!("expected a typed load error");
    };
    outside(error, Some(AerodynamicStage::First));
}

fn wind_boundary_contact(aircraft: &AircraftModel, fixture: &Fixture) {
    let surfaces = fixture.surfaces();
    let maximum_down = surfaces
        .iter()
        .flat_map(|surface| surface.proxies())
        .map(|proxy| proxy.point().components()[2])
        .fold(0.0, f64::max);
    let velocities = [NedVector::zero(); 8];
    let wind = WindField::grid(
        NedPoint::try_new(-32.0, -32.0, -32.0).unwrap(),
        ned([64.0, 64.0, 32.0 + maximum_down + 0.001]),
        [2; 3],
        &velocities,
    )
    .unwrap();
    let loads = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        0.01,
        wind,
    )
    .unwrap();
    let previous = TailFlightTickState::try_new(
        aircraft,
        7,
        FlightState::try_new(
            NedPoint::origin(),
            ned([10.0, 0.0, 1.0]),
            UnitQuaternion::IDENTITY,
            BodyVector::zero(),
            0.0,
            0.0,
        )
        .unwrap(),
        TailIncidence::neutral(),
        PilotPositionTarget::try_new(aircraft, 0.0).unwrap(),
    )
    .unwrap();
    let config = TailFlightTickConfig::new(
        ControlMode::Manual,
        TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap(),
        TailPilotPositionMapping::try_new(aircraft, 0.0).unwrap(),
        Gravity::try_new(9.80665).unwrap(),
    );
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.0, 0.0).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Hold,
    );
    let original = advance_tail_flight_tick(aircraft, previous, config, input, &loads).unwrap_err();
    let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
        AerodynamicEvaluationError::Hybrid(cause),
    ))) = original
    else {
        panic!("expected a future wind-domain failure");
    };
    assert_eq!(cause.cause(), AeroError::Wind(WindError::OutsideGrid));
    assert_eq!(cause.stage(), Some(AerodynamicStage::Second));
    for (name, down, contact_expected) in [
        ("wind boundary fractional contact", -0.00025, true),
        ("wind boundary original failure", -0.1, false),
    ] {
        let points = [point([0.0, 0.0, down])];
        let geometry = WaterContactGeometry::try_new(&points).unwrap();
        let result = measure_iterations(name, 8, || {
            advance_tail_flight_tick_with_contact(
                aircraft, previous, config, input, &loads, geometry,
            )
        });
        if contact_expected {
            let TailFlightTickOutcome::WaterContact(contact) = result.unwrap() else {
                panic!("expected a terminal partial interval");
            };
            assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
        } else {
            assert_eq!(result, Err(original));
        }
    }
}

fn main() {
    calibrate();
    registered_application_flight();
    let definition = birdman_game_content::playable_aircraft().unwrap();
    let mut points = definition.contact_points().to_vec();
    let mut deformation_case = 0;
    measure_iterations("registered airframe deformation", 100, || {
        let (speed, elevator, rudder) = match deformation_case % 3 {
            0 => (0.0, 0.2, -0.1),
            1 => (9.7, -0.1, 0.2),
            _ => (13.0, 0.0, 0.0),
        };
        deformation_case += 1;
        let factor = definition
            .shape()
            .wing_bending_factor(black_box(speed))
            .unwrap()
            .interpolate(definition.shape().wing_bending_factor(13.0).unwrap(), 0.25)
            .unwrap();
        definition
            .write_deformed_contact_points_with_factor(
                black_box(factor),
                black_box(TailIncidence::try_new(elevator, rudder).unwrap()),
                black_box(&mut points),
            )
            .unwrap();
        black_box(points.last().copied())
    });
    let fixture = Fixture::new();
    let surfaces = fixture.surfaces();
    let wind = WindField::linear_gradient(
        NedPoint::origin(),
        ned([1.0, 0.0, 0.0]),
        [[0.0, 0.01, 0.0], [0.0; 3], [0.0, 0.02, 0.0]],
    )
    .unwrap();
    let load = HybridAerodynamicLoad::try_new(
        HybridModel::try_new(fixture.polar(), &surfaces).unwrap(),
        1.2,
        wind,
    )
    .unwrap();
    let provider = AerodynamicLoadProvider::Hybrid(&load);
    let aircraft = AircraftModel::try_new(
        10.0,
        InertiaTensor::diagonal(20.0, 30.0, 40.0).unwrap(),
        3.0,
        0.0,
        -0.5,
        0.5,
        1.0,
        1.0,
    )
    .unwrap();
    wind_boundary_contact(&aircraft, &fixture);
    let valid = state([11.0, 0.2, 0.4]);
    let invalid = state([11.0, 3.0, 0.4]);
    let gravity = Gravity::try_new(9.80665).unwrap();
    let pilot_acceleration = PilotAcceleration::try_new(0.1).unwrap();
    let pilot_target = PilotPositionTarget::try_new(&aircraft, 0.0).unwrap();
    let profile = TailControlProfile::try_new(0.2, 0.2, 1.0).unwrap();
    let mapping = TailPilotPositionMapping::try_new(&aircraft, 0.0).unwrap();
    let tick_config = TailFlightTickConfig::new(ControlMode::Manual, profile, mapping, gravity);
    let input = TailFlightTickInput::new(
        TailPilotIntent::try_new(0.1, -0.1).unwrap(),
        TailRateTarget::try_new(0.0, 0.0).unwrap(),
        TailPilotPositionCommand::Set(TailPilotPositionIntent::try_new(0.2).unwrap()),
    );
    let previous =
        TailFlightTickState::try_new(&aircraft, 7, valid, TailIncidence::neutral(), pilot_target)
            .unwrap();
    let failed_previous = TailFlightTickState::try_new(
        &aircraft,
        7,
        invalid,
        TailIncidence::neutral(),
        pilot_target,
    )
    .unwrap();
    let before = failed_previous;
    let expected = provider.evaluate(&aircraft, &valid).unwrap();
    assert_eq!(
        measure("hybrid provider success", || {
            provider.evaluate(black_box(&aircraft), black_box(&valid))
        }),
        Ok(expected)
    );
    let rk = |current: &FlightState| {
        advance(
            black_box(&aircraft),
            black_box(current),
            black_box(pilot_acceleration),
            black_box(gravity),
            black_box(&provider),
            black_box(PHYSICS_DT_SECONDS),
        )
    };
    let expected = rk(&valid).unwrap();
    assert_ne!(expected, valid);
    assert_eq!(measure("hybrid RK4 success", || rk(&valid)), Ok(expected));
    let tick = |current: TailFlightTickState| {
        advance_tail_flight_tick(
            black_box(&aircraft),
            black_box(current),
            black_box(tick_config),
            black_box(input),
            black_box(&load),
        )
    };
    let expected = tick(previous).unwrap();
    assert_eq!(expected.tick_index(), 8);
    assert_ne!(expected.flight_state(), valid);
    assert_eq!(
        measure("hybrid tick success", || tick(previous)),
        Ok(expected)
    );
    outside(
        measure("hybrid provider outside", || {
            provider.evaluate(black_box(&aircraft), black_box(&invalid))
        })
        .unwrap_err(),
        None,
    );
    outside_dynamics(measure("hybrid RK4 outside", || rk(&invalid)).unwrap_err());
    let TailFlightTickError::Dynamics(error) =
        measure("hybrid tick outside", || tick(failed_previous)).unwrap_err()
    else {
        panic!("expected a typed dynamics error");
    };
    outside_dynamics(error);
    assert_eq!(failed_previous, before);
    assert_eq!(failed_previous.tick_index(), 7);
    assert_eq!(failed_previous.incidence(), TailIncidence::neutral());
    assert_eq!(failed_previous.flight_state(), invalid);
}
