use super::{ActuatorError, TailActuatorLimits, TailControlError, TailIncidence};

/// Finite final angle requests before the aircraft's structural stops and slew.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailIncidenceRequest {
    elevator_rad: f64,
    rudder_rad: f64,
}

impl TailIncidenceRequest {
    /// Accepts every finite request, including values outside physical principal angles.
    pub fn try_new(elevator_rad: f64, rudder_rad: f64) -> Result<Self, TailControlError> {
        if !elevator_rad.is_finite() || !rudder_rad.is_finite() {
            return Err(TailControlError::NonFinite);
        }
        Ok(Self {
            elevator_rad,
            rudder_rad,
        })
    }

    /// Horizontal tail angle request in radians.
    pub const fn elevator_rad(self) -> f64 {
        self.elevator_rad
    }
    /// Vertical tail angle request in radians.
    pub const fn rudder_rad(self) -> f64 {
        self.rudder_rad
    }
}

impl From<TailIncidence> for TailIncidenceRequest {
    fn from(incidence: TailIncidence) -> Self {
        Self {
            elevator_rad: incidence.elevator_rad(),
            rudder_rad: incidence.rudder_rad(),
        }
    }
}

/// One shared structural saturation and slew evaluation, reused for adoption and reporting.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct TailActuatorUpdate {
    requested: TailIncidenceRequest,
    structural_target: TailIncidence,
    incidence: TailIncidence,
    obstruction_mask: u8,
}

impl TailActuatorUpdate {
    /// Original finite request before aircraft limits.
    pub const fn requested(self) -> TailIncidenceRequest {
        self.requested
    }
    /// Target after aircraft structural saturation, before slew.
    pub const fn structural_target(self) -> TailIncidence {
        self.structural_target
    }
    /// Held physical angle selected for the candidate interval.
    pub const fn incidence(self) -> TailIncidence {
        self.incidence
    }
    /// Per axis: structural lower/upper then slew lower/upper; elevator occupies bits0–3.
    pub const fn obstruction_mask(self) -> u8 {
        self.obstruction_mask
    }

    /// Restores saved finite command data without re-running hardware or controller rules.
    pub fn try_from_recorded(
        requested: TailIncidenceRequest,
        structural_target: TailIncidence,
        incidence: TailIncidence,
        obstruction_mask: u8,
    ) -> Result<Self, TailControlError> {
        for shift in [0, 4] {
            let bits = obstruction_mask >> shift;
            if bits & 3 == 3 || bits & 12 == 12 {
                return Err(TailControlError::InvalidController);
            }
        }
        Ok(Self {
            requested,
            structural_target,
            incidence,
            obstruction_mask,
        })
    }
}

/// Applies only aircraft-owned stops and slew; software mixing and protection precede this call.
pub fn advance_tail_actuators(
    previous: TailIncidence,
    requested: TailIncidenceRequest,
    limits: [TailActuatorLimits; 2],
    seconds: f64,
) -> Result<TailActuatorUpdate, TailControlError> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(TailControlError::Actuator(ActuatorError::InvalidTimeStep));
    }
    let inputs = [requested.elevator_rad(), requested.rudder_rad()];
    let current = [previous.elevator_rad(), previous.rudder_rad()];
    let mut target = [0.0; 2];
    let mut applied = [0.0; 2];
    let mut mask = 0;
    for axis in 0..2 {
        let [low, high] = limits[axis].incidence_bounds_rad();
        if current[axis] < low || current[axis] > high {
            return Err(TailControlError::Actuator(ActuatorError::InvalidLimit));
        }
        target[axis] = inputs[axis].clamp(low, high);
        if inputs[axis] < low {
            mask |= 1 << (axis * 4);
        }
        if inputs[axis] > high {
            mask |= 2 << (axis * 4);
        }
        let maximum_step = limits[axis].maximum_slew_rad_per_second() * seconds;
        let difference = target[axis] - current[axis];
        applied[axis] = if difference.abs() <= maximum_step {
            target[axis]
        } else {
            if difference < 0.0 {
                mask |= 4 << (axis * 4);
            } else {
                mask |= 8 << (axis * 4);
            }
            (current[axis] + difference.signum() * maximum_step).clamp(low, high)
        };
    }
    Ok(TailActuatorUpdate {
        requested,
        structural_target: TailIncidence::try_new(target[0], target[1])
            .map_err(TailControlError::Incidence)?,
        incidence: TailIncidence::try_new(applied[0], applied[1])
            .map_err(TailControlError::Incidence)?,
        obstruction_mask: mask,
    })
}
