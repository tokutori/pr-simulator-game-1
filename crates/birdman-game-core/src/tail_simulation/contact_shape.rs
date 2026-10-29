use super::*;

pub(super) enum TailTrialGeometry<'a> {
    None,
    Fixed(WaterContactGeometry<'a>),
    Deformed {
        provider: &'a mut dyn TailContactShapeProvider,
        factor: WingBendingFactor,
    },
}

impl TailTrialGeometry<'_> {
    pub(super) fn factor(&self, previous: Option<WingBendingFactor>) -> Option<WingBendingFactor> {
        match self {
            Self::Deformed { factor, .. } => Some(*factor),
            Self::None | Self::Fixed(_) => previous,
        }
    }

    pub(super) fn applies_shape(&self) -> bool {
        matches!(self, Self::Deformed { .. })
    }

    pub(super) fn prepare(
        &mut self,
        incidence: TailIncidence,
    ) -> Result<Option<WaterContactGeometry<'_>>, ContactError> {
        match self {
            Self::None => Ok(None),
            Self::Fixed(geometry) => Ok(Some(*geometry)),
            Self::Deformed { provider, factor } => {
                WaterContactGeometry::try_new(provider.points(*factor, incidence)?).map(Some)
            }
        }
    }
}

/// Applies a sampled structural coefficient and candidate tail geometry over one interval.
/// Existing contact is evaluated first, and the provider reuses caller-owned storage.
pub fn advance_tail_flight_tick_with_shape_provider(
    aircraft: &AircraftModel,
    previous: TailFlightTickState,
    config: TailFlightTickConfig,
    input: TailFlightTickInput,
    loads: &HybridAerodynamicLoad<'_>,
    provider: &mut dyn TailContactShapeProvider,
) -> Result<TailFlightTickReport, TailFlightTickError> {
    let previous_factor = previous
        .wing_bending_factor
        .ok_or(TailFlightTickError::Contact(
            ContactError::MissingWingBendingFactor,
        ))?;
    let geometry = WaterContactGeometry::try_new(
        provider
            .points(previous_factor, previous.incidence)
            .map_err(TailFlightTickError::Contact)?,
    )
    .map_err(TailFlightTickError::Contact)?;
    if let Some(index) = geometry
        .initial_contact_point(previous.flight_state)
        .map_err(TailFlightTickError::Contact)?
    {
        return Ok(contact_at_start_report(previous, 0.0, index));
    }
    let telemetry =
        crate::scenario::derive_flight_telemetry(aircraft, previous.flight_state, |position| {
            loads.wind_velocity_at(position)
        })
        .map_err(|error| {
            TailFlightTickError::Contact(match error {
                crate::FlightTelemetryError::Dynamics(cause) => ContactError::Dynamics(cause),
                crate::FlightTelemetryError::Wind(cause) => ContactError::Wind(cause),
            })
        })?;
    let factor = provider
        .wing_bending_factor(telemetry.airspeed_mps)
        .map_err(TailFlightTickError::Contact)?;
    let evaluation = evaluate_tail_flight_tick(
        aircraft,
        previous,
        config,
        input,
        loads,
        TailTrialGeometry::Deformed { provider, factor },
    )?;
    let mut report = evaluated_tick_report(
        previous,
        evaluation.next,
        evaluation.controls,
        evaluation.contact,
    );
    report.tail_wrenches = evaluation.tail_wrenches;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tail_simulation::protection_tests::{
        commands, config, input_holding_incidence, state, tail_boundary_body_rates, trial,
    };
    use crate::{BodyPoint, HybridModel, MathError, TestAircraft, WindField};
    use core::cell::Cell;

    struct SampledShape {
        points: [BodyPoint; 1],
        factor_calls: Cell<usize>,
        point_calls: usize,
        base_down: f64,
        factor_down: f64,
        elevator_down: f64,
        factor_error: Option<ContactError>,
        fail_point_call: Option<usize>,
    }

    impl SampledShape {
        fn new(factor_down: f64, elevator_down: f64) -> Self {
            Self {
                points: [BodyPoint::try_new(0.0, 0.0, 0.0).unwrap()],
                factor_calls: Cell::new(0),
                point_calls: 0,
                base_down: 0.0,
                factor_down,
                elevator_down,
                factor_error: None,
                fail_point_call: None,
            }
        }
    }

    impl TailContactShapeProvider for SampledShape {
        fn wing_bending_factor(&self, speed: f64) -> Result<WingBendingFactor, ContactError> {
            self.factor_calls.set(self.factor_calls.get() + 1);
            if let Some(error) = self.factor_error {
                return Err(error);
            }
            WingBendingFactor::try_new(speed / 10.0).map_err(ContactError::Math)
        }

        fn points(
            &mut self,
            factor: WingBendingFactor,
            incidence: TailIncidence,
        ) -> Result<&[BodyPoint], ContactError> {
            self.point_calls += 1;
            if self.fail_point_call == Some(self.point_calls) {
                return Err(ContactError::Math(MathError::NonFinite));
            }
            self.points[0] = BodyPoint::try_new(
                0.0,
                0.0,
                self.base_down
                    + self.factor_down * factor.value()
                    + self.elevator_down * incidence.elevator_rad(),
            )
            .map_err(ContactError::Math)?;
            Ok(&self.points)
        }
    }

    #[test]
    fn shape_update_contact_commits_the_right_side_coefficient_and_input() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        for (previous_factor, factor_down, elevator_down) in [(0.2, 1.0, 0.0), (1.0, 0.0, 100.0)] {
            let previous = state(
                &aircraft,
                [10.0, 0.0, 0.0],
                [0.0; 3],
                TailIncidence::neutral(),
                -0.5,
            )
            .with_wing_bending_factor(WingBendingFactor::try_new(previous_factor).unwrap());
            let input = input_holding_incidence(TailIncidence::try_new(0.02, 0.0).unwrap());
            let mut shape = SampledShape::new(factor_down, elevator_down);
            let report = advance_tail_flight_tick_with_shape_provider(
                &aircraft,
                previous,
                config(&aircraft, ControlMode::Manual, 1.0),
                input,
                &loads,
                &mut shape,
            )
            .unwrap();
            let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
                panic!("the new held shape must touch at the boundary");
            };
            assert_eq!(contact.moment(), TailWaterContactMoment::AppliedBoundary);
            assert_eq!(contact.fraction(), 0.0);
            assert_eq!(contact.flight_state(), previous.flight_state());
            assert_eq!(contact.interval_start_tick(), previous.tick_index());
            assert_eq!(contact.wing_bending_factor().unwrap().value(), 1.0);
            assert!(contact.incidence().elevator_rad() > previous.incidence().elevator_rad());
            assert_eq!(report.applied_controls().unwrap().input(), input);
            assert_eq!(shape.factor_calls.get(), 1);
            assert_eq!(shape.point_calls, 2);
        }
    }

    #[test]
    fn existing_contact_precedes_a_new_load_law_and_retains_the_accepted_shape() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        let previous = state(
            &aircraft,
            [10.0, 0.0, 0.0],
            [0.0; 3],
            TailIncidence::neutral(),
            -0.5,
        )
        .with_wing_bending_factor(WingBendingFactor::try_new(0.7).unwrap());
        let mut shape = SampledShape::new(1.0, 0.0);
        shape.factor_error = Some(ContactError::ShapeProjection);
        let report = advance_tail_flight_tick_with_shape_provider(
            &aircraft,
            previous,
            config(&aircraft, ControlMode::Manual, 1.0),
            input_holding_incidence(TailIncidence::try_new(0.02, 0.0).unwrap()),
            &loads,
            &mut shape,
        )
        .unwrap();
        let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
            panic!("the accepted shape already touches water");
        };
        assert_eq!(contact.moment(), TailWaterContactMoment::ExistingBoundary);
        assert_eq!(
            contact.wing_bending_factor(),
            previous.wing_bending_factor()
        );
        assert_eq!(contact.flight_state(), previous.flight_state());
        assert_eq!(contact.incidence(), previous.incidence());
        assert_eq!(report.applied_controls(), None);
        assert_eq!(shape.factor_calls.get(), 0);
        assert_eq!(shape.point_calls, 1);
    }

    #[test]
    fn applied_boundary_contact_precedes_a_future_rk_wind_domain_failure() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let previous = state(
            &aircraft,
            [10.0, 0.0, 1.0],
            [0.0; 3],
            TailIncidence::neutral(),
            0.0,
        )
        .with_wing_bending_factor(WingBendingFactor::try_new(0.2).unwrap());
        let maximum_down = surfaces
            .iter()
            .flat_map(|surface| surface.proxies())
            .map(|proxy| proxy.point().components()[2])
            .fold(0.0, f64::max);
        let wind_samples = [crate::NedVector::zero(); 8];
        let wind = WindField::grid(
            crate::NedPoint::try_new(-32.0, -32.0, -32.0).unwrap(),
            crate::NedVector::try_new(64.0, 64.0, 32.0 + maximum_down + 0.001).unwrap(),
            [2; 3],
            &wind_samples,
        )
        .unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            wind,
        )
        .unwrap();
        let settings = config(&aircraft, ControlMode::Manual, 1.0);
        let input = input_holding_incidence(TailIncidence::neutral());
        let error =
            advance_tail_flight_tick(&aircraft, previous, settings, input, &loads).unwrap_err();
        let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
            AerodynamicEvaluationError::Hybrid(cause),
        ))) = error
        else {
            panic!("the ordinary future interval must reach the wind boundary");
        };
        assert_eq!(
            cause.cause(),
            AeroError::Wind(crate::WindError::OutsideGrid)
        );
        assert_ne!(cause.stage(), Some(crate::AerodynamicStage::First));
        let mut shape = SampledShape::new(1.0, 0.0);
        shape.base_down = -0.5;
        let report = advance_tail_flight_tick_with_shape_provider(
            &aircraft, previous, settings, input, &loads, &mut shape,
        )
        .unwrap();
        let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
            panic!("the applied shape must touch before any future stage");
        };
        assert_eq!(contact.moment(), TailWaterContactMoment::AppliedBoundary);
        assert_eq!(contact.flight_state(), previous.flight_state());
        assert_eq!(contact.fraction(), 0.0);
        assert!(report.applied_controls().is_some());
    }

    #[test]
    fn positive_fraction_contact_precedes_a_future_rk_wind_domain_failure() {
        for (margin, expected_stage) in [
            (0.001, crate::AerodynamicStage::Second),
            (0.0051, crate::AerodynamicStage::Third),
            (0.008, crate::AerodynamicStage::Fourth),
        ] {
            let definition = TestAircraft::try_new().unwrap();
            let aircraft = definition.aircraft();
            let surfaces = definition.surfaces().unwrap();
            let mut previous = state(
                &aircraft,
                [10.0, 0.0, 1.0],
                [0.01, 0.0, 0.0],
                TailIncidence::neutral(),
                0.0,
            )
            .with_wing_bending_factor(WingBendingFactor::try_new(0.2).unwrap());
            previous.tick_index = 0;
            let maximum_down = surfaces
                .iter()
                .flat_map(|surface| surface.proxies())
                .map(|proxy| proxy.point().components()[2])
                .fold(0.0, f64::max);
            let wind_samples = [crate::NedVector::zero(); 8];
            let wind = WindField::grid(
                crate::NedPoint::try_new(-32.0, -32.0, -32.0).unwrap(),
                crate::NedVector::try_new(64.0, 64.0, 32.0 + maximum_down + margin).unwrap(),
                [2; 3],
                &wind_samples,
            )
            .unwrap();
            let loads = HybridAerodynamicLoad::try_new(
                HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
                0.01,
                wind,
            )
            .unwrap();
            let mut settings = config(&aircraft, ControlMode::Manual, 1.0);
            settings.gravity = Gravity::try_new(9.80665).unwrap();
            let input = TailFlightTickInput::new(
                crate::TailPilotIntent::try_new(0.0, 0.0).unwrap(),
                crate::TailRateTarget::try_new(0.0, 0.0).unwrap(),
                crate::TailPilotPositionCommand::Set(
                    crate::TailPilotPositionIntent::try_new(0.1).unwrap(),
                ),
            );
            let pilot_acceleration =
                trial(&aircraft, previous, settings, input, &loads).pilot_acceleration;
            assert!(pilot_acceleration.meters_per_second_squared() > 0.0);
            let original_error =
                advance_tail_flight_tick(&aircraft, previous, settings, input, &loads).unwrap_err();
            let TailFlightTickError::Dynamics(DynamicsError::Load(LoadError::Aerodynamic(
                AerodynamicEvaluationError::Hybrid(cause),
            ))) = original_error
            else {
                panic!("the full interval must leave the wind grid");
            };
            assert_eq!(
                cause.cause(),
                AeroError::Wind(crate::WindError::OutsideGrid)
            );
            assert_eq!(cause.stage(), Some(expected_stage));

            let reference_duration = 0.0005 / 128.0;
            let points = [BodyPoint::try_new(0.0, 0.0, -0.00025).unwrap()];
            let mut reference = previous.flight_state();
            let (reference_contact, reference_time) = (0..128)
                .find_map(|index| {
                    let next = advance(
                        &aircraft,
                        &reference,
                        pilot_acceleration,
                        settings.gravity,
                        &HeldTailLoad {
                            loads: &loads,
                            incidence: TailIncidence::neutral(),
                        },
                        reference_duration,
                    )
                    .unwrap();
                    let contact = detect_flight_state_water_contact(
                        &aircraft,
                        previous.tick_index(),
                        reference,
                        previous.tick_index() + 1,
                        next,
                        WaterContactGeometry::try_new(&points).unwrap(),
                    )
                    .unwrap();
                    reference = next;
                    contact.map(|contact| {
                        let time = (f64::from(index) + contact.fraction) * reference_duration;
                        (contact, time)
                    })
                })
                .unwrap();
            assert!(reference_time > 0.0 && reference_time < 0.0005);
            loads
                .evaluate_hybrid(&reference_contact.flight_state, TailIncidence::neutral())
                .unwrap();

            let mut shape = SampledShape::new(0.0, 0.0);
            shape.base_down = -0.00025;
            let report = advance_tail_flight_tick_with_shape_provider(
                &aircraft, previous, settings, input, &loads, &mut shape,
            )
            .unwrap();
            let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
                panic!("the positive-time contact precedes the invalid wind stage");
            };
            assert_eq!(contact.moment(), TailWaterContactMoment::WithinInterval);
            assert_eq!(contact.interval_start_tick(), previous.tick_index());
            assert!(
                (contact.fraction() * PHYSICS_DT_SECONDS - reference_time).abs() < 5.0e-8,
                "actual {} reference {reference_time}",
                contact.fraction() * PHYSICS_DT_SECONDS
            );
            assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
            assert_eq!(contact.incidence(), TailIncidence::neutral());
            let time = contact.fraction() * PHYSICS_DT_SECONDS;
            let acceleration = pilot_acceleration.meters_per_second_squared();
            assert!(
                (contact.flight_state().pilot_position_m() - 0.5 * acceleration * time * time)
                    .abs()
                    < 1.0e-12
            );
            assert!(
                (contact.flight_state().pilot_velocity_mps() - acceleration * time).abs() < 1.0e-12
            );
            assert_eq!(
                contact.pilot_position_target(),
                trial(&aircraft, previous, settings, input, &loads).pilot_position_target
            );
            assert!(
                (contact.wing_bending_factor().unwrap().value() - libm::sqrt(101.0) / 10.0).abs()
                    < 1.0e-14
            );
            assert_eq!(report.applied_controls().unwrap().input(), input);
            assert_eq!(shape.factor_calls.get(), 1);
            assert_eq!(shape.point_calls, 2);
            loads
                .evaluate_hybrid(&contact.flight_state(), contact.incidence())
                .unwrap();
            let header = crate::FlightRecordHeader::try_new(
                crate::SessionScenarioIdentity {
                    catalog_version: 1,
                    scenario_id: 1,
                    scenario_version: 1,
                    aircraft_model_version: 1,
                    environment_version: 1,
                    controller_profile_version: 1,
                    seed: 17,
                },
                1,
            )
            .unwrap();
            let mut record = crate::FlightRecord::try_new(header).unwrap();
            let telemetry = |state| {
                crate::scenario::derive_flight_telemetry(&aircraft, state, |position| {
                    loads.wind_velocity_at(position)
                })
                .unwrap()
            };
            record
                .begin_tail(previous, telemetry(previous.flight_state()), None)
                .unwrap();
            record
                .append_tail_report(report, telemetry(contact.flight_state()))
                .unwrap();
            record
                .finalize(
                    crate::SessionEndReason::WaterContact,
                    0,
                    contact.fraction(),
                    None,
                )
                .unwrap();
            assert_eq!(record.sample_count(), 2);
            assert_eq!(record.duration_seconds().unwrap(), time);
            let playback = record.sample_at_seconds(time).unwrap();
            assert_eq!(playback.tick_index, 0);
            assert_eq!(playback.fraction, contact.fraction());
            assert_eq!(playback.flight_state, contact.flight_state());
            assert_eq!(playback.actuators, contact.incidence());
            assert_eq!(playback.wing_bending_factor, contact.wing_bending_factor());
            let mut airborne_shape = SampledShape::new(0.0, 0.0);
            airborne_shape.base_down = -0.1;
            assert_eq!(
                advance_tail_flight_tick_with_shape_provider(
                    &aircraft,
                    previous,
                    settings,
                    input,
                    &loads,
                    &mut airborne_shape,
                ),
                Err(original_error)
            );
            assert_eq!(airborne_shape.factor_calls.get(), 1);
            assert_eq!(airborne_shape.point_calls, 2);
        }
    }

    #[test]
    fn missing_accepted_coefficient_is_rejected_before_projection() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        let unrecorded = state(
            &aircraft,
            [10.0, 0.0, 0.0],
            [0.0; 3],
            TailIncidence::neutral(),
            -10.0,
        );
        let input = input_holding_incidence(TailIncidence::neutral());
        let settings = config(&aircraft, ControlMode::Manual, 1.0);
        let mut shape = SampledShape::new(1.0, 0.0);
        assert_eq!(
            advance_tail_flight_tick_with_shape_provider(
                &aircraft, unrecorded, settings, input, &loads, &mut shape
            ),
            Err(TailFlightTickError::Contact(
                ContactError::MissingWingBendingFactor
            ))
        );
        assert_eq!(shape.factor_calls.get(), 0);
        assert_eq!(shape.point_calls, 0);
    }

    #[test]
    fn projection_failure_retains_the_accepted_coefficient() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        let previous = state(
            &aircraft,
            [10.0, 0.0, 0.0],
            [0.0; 3],
            TailIncidence::neutral(),
            -10.0,
        )
        .with_wing_bending_factor(WingBendingFactor::try_new(0.2).unwrap());
        let input = input_holding_incidence(TailIncidence::neutral());
        let settings = config(&aircraft, ControlMode::Manual, 1.0);
        let mut shape = SampledShape::new(1.0, 0.0);
        shape.fail_point_call = Some(2);
        assert_eq!(
            advance_tail_flight_tick_with_shape_provider(
                &aircraft, previous, settings, input, &loads, &mut shape
            ),
            Err(TailFlightTickError::Contact(ContactError::Math(
                MathError::NonFinite
            )))
        );
        assert_eq!(previous.wing_bending_factor().unwrap().value(), 0.2);
    }

    #[test]
    fn fallback_candidate_projection_failure_is_propagated_atomically() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let tails = [surfaces[1], surfaces[2]];
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &tails).unwrap(),
            1.2,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        let mut previous = state(
            &aircraft,
            [10.0, 0.0, 0.0],
            tail_boundary_body_rates(tails[0], 10.0, 1.0),
            TailIncidence::neutral(),
            -50.0,
        )
        .with_wing_bending_factor(WingBendingFactor::try_new(0.2).unwrap());
        let intervals = loads
            .tail_incidence_intervals(&previous.flight_state())
            .unwrap();
        previous.incidence =
            TailIncidence::try_new(intervals[0][1] - 0.005, intervals[1][1] - 0.005).unwrap();
        let input = TailFlightTickInput::new(
            crate::TailPilotIntent::try_new(-1.0, -1.0).unwrap(),
            crate::TailRateTarget::try_new(-0.2, -0.2).unwrap(),
            crate::TailPilotPositionCommand::Hold,
        );
        let settings = config(&aircraft, ControlMode::Manual, 1.0);
        let nominal = commands(previous, settings, input);
        let Err(original) =
            trial(&aircraft, previous, settings, input, &loads).evaluate(nominal.incidence())
        else {
            panic!("the nominal request must require a protected fallback");
        };
        assert!(controlled_tail_failure(original));
        let mut shape = SampledShape::new(0.0, 0.0);
        shape.fail_point_call = Some(3);
        assert_eq!(
            advance_tail_flight_tick_with_shape_provider(
                &aircraft, previous, settings, input, &loads, &mut shape
            ),
            Err(TailFlightTickError::Contact(ContactError::Math(
                MathError::NonFinite
            )))
        );
        assert_eq!(shape.point_calls, 3);
        assert_eq!(shape.factor_calls.get(), 1);
    }

    #[test]
    fn airborne_interval_holds_its_initial_coefficient() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        let previous = state(
            &aircraft,
            [10.0, 0.0, 0.0],
            [0.0; 3],
            TailIncidence::neutral(),
            -10.0,
        )
        .with_wing_bending_factor(WingBendingFactor::try_new(0.2).unwrap());
        let input = input_holding_incidence(TailIncidence::neutral());
        let settings = config(&aircraft, ControlMode::Manual, 1.0);
        let mut shape = SampledShape::new(1.0, 0.0);
        let report = advance_tail_flight_tick_with_shape_provider(
            &aircraft, previous, settings, input, &loads, &mut shape,
        )
        .unwrap();
        let TailFlightTickOutcome::Advanced(next) = report.outcome() else {
            panic!("the complete interval must remain airborne");
        };
        assert_eq!(next.wing_bending_factor().unwrap().value(), 1.0);
        let next_telemetry =
            crate::scenario::derive_flight_telemetry(&aircraft, next.flight_state(), |_| {
                Ok(crate::NedVector::zero())
            })
            .unwrap();
        assert_ne!(next_telemetry.airspeed_mps / 10.0, 1.0);
        assert_eq!(shape.factor_calls.get(), 1);
        assert_eq!(shape.point_calls, 2);
    }

    #[test]
    fn positive_contact_uses_the_held_shape_at_the_same_fractional_body_time() {
        let definition = TestAircraft::try_new().unwrap();
        let aircraft = definition.aircraft();
        let surfaces = definition.surfaces().unwrap();
        let loads = HybridAerodynamicLoad::try_new(
            HybridModel::try_new(definition.polar().unwrap(), &surfaces).unwrap(),
            1.225,
            WindField::uniform(crate::NedVector::zero()),
        )
        .unwrap();
        let previous = state(
            &aircraft,
            [10.0, 0.0, 1.0],
            [0.0; 3],
            TailIncidence::neutral(),
            -1.01,
        )
        .with_wing_bending_factor(WingBendingFactor::try_new(0.2).unwrap());
        let mut shape = SampledShape::new(1.0, 0.0);
        let report = advance_tail_flight_tick_with_shape_provider(
            &aircraft,
            previous,
            config(&aircraft, ControlMode::Manual, 1.0),
            input_holding_incidence(TailIncidence::neutral()),
            &loads,
            &mut shape,
        )
        .unwrap();
        let TailFlightTickOutcome::WaterContact(contact) = report.outcome() else {
            panic!("the descending support point must reach water within the interval");
        };
        assert_eq!(contact.moment(), TailWaterContactMoment::WithinInterval);
        assert!(contact.fraction() > 0.0 && contact.fraction() < 1.0);
        assert_eq!(
            contact.wing_bending_factor().unwrap().value(),
            libm::sqrt(101.0) / 10.0
        );
        let point = contact
            .flight_state()
            .attitude_body_to_ned()
            .body_to_ned(
                crate::BodyVector::try_new(
                    0.0,
                    0.0,
                    contact.wing_bending_factor().unwrap().value(),
                )
                .unwrap(),
            )
            .unwrap();
        assert!(
            (contact.flight_state().datum_position_ned().components()[2] + point.components()[2])
                .abs()
                < 1.0e-9
        );
        assert!(report.applied_controls().is_some());
        assert_eq!(shape.factor_calls.get(), 1);
    }
}
