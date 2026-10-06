use birdman_game_core::{
    ActuatorConfig, ActuatorState, AeroError, AerodynamicEvaluationError, AerodynamicLoadProvider,
    AerodynamicStage, AircraftModel, BodyPoint, BodyVector, ControlMode, DynamicsError,
    ElementOrientation, ElementReference, ExternalLoadProvider, FlightState, FlightTickConfig,
    FlightTickError, FlightTickInput, FlightTickState, Gravity, HybridAerodynamicLoad,
    HybridAnchor, HybridLimit, HybridModel, HybridProxy, HybridSection, HybridSite, HybridSurface,
    HybridSurfaceGeometry, HybridSurfaceRole, InertiaTensor, LoadError, NedPoint, NedVector,
    PHYSICS_DT_SECONDS, PilotAcceleration, PilotPositionTarget, PlanformSymmetry,
    PolarAnalysisMethod, PolarMomentAxes, StaticPolar, StaticPolarCoefficients,
    StaticPolarMetadata, StaticPolarRow, SurfaceCommands, SurfaceDeflections, UnitQuaternion,
    WindField, advance_flight_tick, advance_with_surface_deflections,
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

fn measure<T>(name: &str, mut operation: impl FnMut() -> T) -> T {
    let mut result = black_box(operation());
    begin();
    for _ in 0..ITERATIONS {
        result = black_box(operation());
    }
    let counts = end();
    assert_eq!(counts, [0; 3], "{name} allocated in the measured window");
    println!("{name}: iterations={ITERATIONS} alloc=0 alloc_zeroed=0 realloc=0");
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
            HybridSurface::try_new(Self::geometry(&self.sections, index), &self.proxies[index])
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

fn main() {
    calibrate();
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
    let valid = state([11.0, 0.2, 0.4]);
    let invalid = state([11.0, 3.0, 0.4]);
    let controls = SurfaceDeflections::try_new(0.0, 0.01, -0.01).unwrap();
    let gravity = Gravity::try_new(9.80665).unwrap();
    let pilot_acceleration = PilotAcceleration::try_new(0.1).unwrap();
    let limits = [ActuatorConfig::try_new(0.2, 1.0).unwrap(); 3];
    let actuator = ActuatorState::try_new(limits, SurfaceDeflections::neutral()).unwrap();
    let tick_config = FlightTickConfig::new(ControlMode::Manual, limits, gravity);
    let input = FlightTickInput::new(
        SurfaceCommands::try_new(0.0, 0.1, -0.1).unwrap(),
        SurfaceCommands::try_new(0.0, 0.0, 0.0).unwrap(),
        PilotPositionTarget::try_new(&aircraft, 0.1).unwrap(),
    );
    let previous = FlightTickState::try_new(&aircraft, limits, 7, valid, actuator).unwrap();
    let failed_previous =
        FlightTickState::try_new(&aircraft, limits, 7, invalid, actuator).unwrap();
    let before = failed_previous;
    let expected = provider
        .evaluate_with_surface_deflections(&aircraft, &valid, controls)
        .unwrap();
    assert_eq!(
        measure("hybrid provider success", || {
            provider.evaluate_with_surface_deflections(
                black_box(&aircraft),
                black_box(&valid),
                black_box(controls),
            )
        }),
        Ok(expected)
    );
    let rk = |current: &FlightState| {
        advance_with_surface_deflections(
            black_box(&aircraft),
            black_box(current),
            black_box(pilot_acceleration),
            black_box(gravity),
            black_box(&provider),
            black_box(controls),
            black_box(PHYSICS_DT_SECONDS),
        )
    };
    let expected = rk(&valid).unwrap();
    assert_ne!(expected, valid);
    assert_eq!(measure("hybrid RK4 success", || rk(&valid)), Ok(expected));
    let tick = |current: FlightTickState| {
        advance_flight_tick(
            black_box(&aircraft),
            black_box(current),
            black_box(tick_config),
            black_box(input),
            black_box(&provider),
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
            provider.evaluate_with_surface_deflections(
                black_box(&aircraft),
                black_box(&invalid),
                black_box(controls),
            )
        })
        .unwrap_err(),
        None,
    );
    outside_dynamics(measure("hybrid RK4 outside", || rk(&invalid)).unwrap_err());
    let FlightTickError::Dynamics(error) =
        measure("hybrid tick outside", || tick(failed_previous)).unwrap_err()
    else {
        panic!("expected a typed dynamics error");
    };
    outside_dynamics(error);
    assert_eq!(failed_previous, before);
    assert_eq!(failed_previous.tick_index(), 7);
    assert_eq!(failed_previous.actuator_state(), actuator);
    assert_eq!(failed_previous.flight_state(), invalid);
}
