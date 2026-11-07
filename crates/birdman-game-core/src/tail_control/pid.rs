use super::TailControlError;

/// Fixed-size gains and output-contribution limits for one controller stage.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct PidConfig {
    kp: f64,
    ki: f64,
    kd: f64,
    integral_bounds: [f64; 2],
    output_bounds: [f64; 2],
    derivative_filter_seconds: f64,
}

impl PidConfig {
    /// Validates finite, nonnegative gains and ordered intervals containing zero.
    pub fn try_new(
        kp: f64,
        ki: f64,
        kd: f64,
        integral_bounds: [f64; 2],
        output_bounds: [f64; 2],
        derivative_filter_seconds: f64,
    ) -> Result<Self, TailControlError> {
        if ![kp, ki, kd, derivative_filter_seconds]
            .into_iter()
            .chain(integral_bounds)
            .chain(output_bounds)
            .all(f64::is_finite)
        {
            return Err(TailControlError::NonFinite);
        }
        if kp < 0.0
            || ki < 0.0
            || kd < 0.0
            || derivative_filter_seconds < 0.0
            || [integral_bounds, output_bounds]
                .into_iter()
                .any(|[low, high]| low > 0.0 || high < 0.0 || low > high)
        {
            return Err(TailControlError::InvalidController);
        }
        Ok(Self {
            kp,
            ki,
            kd,
            integral_bounds,
            output_bounds,
            derivative_filter_seconds,
        })
    }

    /// Proportional gain in the stage's output/error units.
    pub const fn kp(self) -> f64 {
        self.kp
    }
    /// Integral gain in the stage's output/(error second) units.
    pub const fn ki(self) -> f64 {
        self.ki
    }
    /// Derivative gain in the stage's output second/error units.
    pub const fn kd(self) -> f64 {
        self.kd
    }
    /// Limits on the accumulated output contribution.
    pub const fn integral_bounds(self) -> [f64; 2] {
        self.integral_bounds
    }
    /// Limits on the stage's commanded output.
    pub const fn output_bounds(self) -> [f64; 2] {
        self.output_bounds
    }
    /// First-order derivative filter time constant in seconds.
    pub const fn derivative_filter_seconds(self) -> f64 {
        self.derivative_filter_seconds
    }
}

/// Accepted history of one PID stage. Integral is an output contribution.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(verus_only, vstd::prelude::verus_verify)]
pub struct PidMemory {
    integral: f64,
    previous_measurement: Option<f64>,
    filtered_derivative: f64,
}

impl PidMemory {
    pub(super) const ZERO: Self = Self {
        integral: 0.0,
        previous_measurement: None,
        filtered_derivative: 0.0,
    };

    /// Accepted integral contribution used by the next output evaluation.
    pub const fn integral(self) -> f64 {
        self.integral
    }
    /// Last admitted observation, absent after an unobservable attitude.
    pub const fn previous_measurement(self) -> Option<f64> {
        self.previous_measurement
    }
    /// Accepted filtered observation derivative, with the error's sign.
    pub const fn filtered_derivative(self) -> f64 {
        self.filtered_derivative
    }

    pub(super) const fn unobserved(self) -> Self {
        Self {
            previous_measurement: None,
            filtered_derivative: 0.0,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PidEvaluation {
    pub(super) output: f64,
    next_history: PidMemory,
    integral_rate: f64,
    unclamped_output: f64,
    config: PidConfig,
}

/// Directions of output change prevented by downstream constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PidObstruction {
    positive: bool,
    negative: bool,
}

impl PidObstruction {
    pub(super) const NONE: Self = Self {
        positive: false,
        negative: false,
    };
    pub(super) fn difference(requested: f64, applied: f64) -> Self {
        Self {
            positive: requested > applied,
            negative: requested < applied,
        }
    }
    pub(super) const fn union(self, other: Self) -> Self {
        Self {
            positive: self.positive || other.positive,
            negative: self.negative || other.negative,
        }
    }
    /// Pulls a blocked direction back through one signed kinematic coefficient.
    pub(super) fn via(self, coefficient: f64) -> Self {
        if coefficient > 0.0 {
            self
        } else if coefficient < 0.0 {
            Self {
                positive: self.negative,
                negative: self.positive,
            }
        } else {
            Self::NONE
        }
    }
    fn blocks(self, direction: f64) -> bool {
        (direction > 0.0 && self.positive) || (direction < 0.0 && self.negative)
    }
}

impl PidEvaluation {
    pub(super) fn obstruction(self) -> PidObstruction {
        PidObstruction::difference(self.unclamped_output, self.output)
    }
    /// Evaluates I_n and the current derivative exactly once; I_next awaits admission.
    pub(super) fn evaluate(
        config: PidConfig,
        memory: PidMemory,
        error: f64,
        measurement: f64,
        observation_seconds: f64,
        periodic: bool,
        measurement_error_sign: f64,
    ) -> Result<Self, TailControlError> {
        let derivative = if let Some(previous) = memory.previous_measurement {
            if observation_seconds <= 0.0 || !observation_seconds.is_finite() {
                return Err(TailControlError::InvalidController);
            }
            let difference = if periodic {
                angle_difference(measurement, previous)
            } else {
                measurement - previous
            };
            let raw = measurement_error_sign * difference / observation_seconds;
            let alpha = config.derivative_filter_seconds
                / (config.derivative_filter_seconds + observation_seconds);
            alpha * memory.filtered_derivative + (1.0 - alpha) * raw
        } else {
            0.0
        };
        let unclamped_output = config.kp * error + memory.integral + config.kd * derivative;
        let integral_rate = config.ki * error;
        if ![
            error,
            measurement,
            derivative,
            unclamped_output,
            integral_rate,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(TailControlError::NonFinite);
        }
        let output = unclamped_output.clamp(config.output_bounds[0], config.output_bounds[1]);
        Ok(Self {
            output,
            next_history: PidMemory {
                previous_measurement: Some(measurement),
                filtered_derivative: derivative,
                ..memory
            },
            integral_rate,
            unclamped_output,
            config,
        })
    }

    /// Integrates only an adopted interval, with directional downstream anti-windup.
    pub(super) fn admitted(
        self,
        seconds: f64,
        authority: f64,
        downstream_obstruction: PidObstruction,
    ) -> Result<PidMemory, TailControlError> {
        if seconds == 0.0 {
            return Ok(self.next_history);
        }
        let own = PidObstruction {
            positive: self.unclamped_output >= self.config.output_bounds[1],
            negative: self.unclamped_output <= self.config.output_bounds[0],
        };
        let integral =
            if authority == 0.0 || own.union(downstream_obstruction).blocks(self.integral_rate) {
                self.next_history.integral
            } else {
                let candidate = self.next_history.integral + self.integral_rate * seconds;
                if !candidate.is_finite() {
                    return Err(TailControlError::NonFinite);
                }
                candidate.clamp(
                    self.config.integral_bounds[0],
                    self.config.integral_bounds[1],
                )
            };
        Ok(PidMemory {
            integral,
            ..self.next_history
        })
    }
}

/// A single half-open convention for heading error and heading observation differences.
pub(super) fn angle_difference(target: f64, observed: f64) -> f64 {
    let difference = target - observed;
    let angle = libm::atan2(libm::sin(difference), libm::cos(difference));
    if angle == core::f64::consts::PI {
        -core::f64::consts::PI
    } else {
        angle
    }
}
