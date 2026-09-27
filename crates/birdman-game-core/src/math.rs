use core::marker::PhantomData;

/// A coordinate frame supported by the simulation core.
pub trait Frame: sealed::Sealed + Copy + core::fmt::Debug + PartialEq {}

mod sealed {
    pub trait Sealed {}
}

/// Forward-right-down aircraft body frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyFrame;

impl sealed::Sealed for BodyFrame {}
impl Frame for BodyFrame {}

/// North-east-down navigation frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NedFrame;

impl sealed::Sealed for NedFrame {}
impl Frame for NedFrame {}

/// A finite three-dimensional vector whose frame is encoded in its type.
///
/// Checked operations return [`MathError::NonFinite`] if arithmetic overflows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector3<F: Frame> {
    components: [f64; 3],
    frame: PhantomData<F>,
}

/// A finite point whose coordinate frame is encoded in its type.
///
/// Checked translation and displacement return [`MathError::NonFinite`] if
/// arithmetic overflows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3<F: Frame> {
    components: [f64; 3],
    frame: PhantomData<F>,
}

/// Body-frame vector in forward-right-down axes.
pub type BodyVector = Vector3<BodyFrame>;
/// Navigation-frame vector in north-east-down axes.
pub type NedVector = Vector3<NedFrame>;
/// Body-frame point relative to the structural datum.
pub type BodyPoint = Point3<BodyFrame>;
/// Navigation-frame point relative to the local NED origin.
pub type NedPoint = Point3<NedFrame>;

/// An error produced by validated mathematical values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathError {
    /// At least one value is non-finite.
    NonFinite,
    /// A quaternion is zero or outside the permitted unit-length tolerance.
    InvalidQuaternion,
    /// A quaternion interpolation fraction is outside the inclusive unit interval.
    InvalidInterpolationFraction,
    /// A matrix is not exactly symmetric.
    AsymmetricTensor,
    /// A tensor is not positive definite.
    NonPositiveDefiniteTensor,
}

impl<F: Frame> Vector3<F> {
    /// Creates a vector from finite components.
    pub fn try_new(
        first_component: f64,
        second_component: f64,
        third_component: f64,
    ) -> Result<Self, MathError> {
        Self::try_from_components([first_component, second_component, third_component])
    }

    /// Returns the zero vector.
    pub const fn zero() -> Self {
        Self {
            components: [0.0; 3],
            frame: PhantomData,
        }
    }

    /// Returns the vector components in frame-axis order.
    pub const fn components(self) -> [f64; 3] {
        self.components
    }

    /// Returns the dot product with another vector in the same frame.
    pub fn dot(self, other: Self) -> Result<f64, MathError> {
        let value = self.components[0] * other.components[0]
            + self.components[1] * other.components[1]
            + self.components[2] * other.components[2];
        if value.is_finite() {
            Ok(value)
        } else {
            Err(MathError::NonFinite)
        }
    }

    /// Returns the right-handed cross product with another vector in the same frame.
    pub fn cross(self, other: Self) -> Result<Self, MathError> {
        let [first_component, second_component, third_component] = self.components;
        let [other_x, other_y, other_z] = other.components;
        Self::try_from_components([
            second_component * other_z - third_component * other_y,
            third_component * other_x - first_component * other_z,
            first_component * other_y - second_component * other_x,
        ])
    }

    /// Returns the squared Euclidean norm.
    pub fn norm_squared(self) -> Result<f64, MathError> {
        self.dot(self)
    }

    /// Returns the Euclidean norm.
    pub fn norm(self) -> Result<f64, MathError> {
        let scale = self
            .components
            .iter()
            .map(|component| component.abs())
            .fold(0.0, f64::max);
        if scale == 0.0 {
            return Ok(0.0);
        }
        let normalized = [
            self.components[0] / scale,
            self.components[1] / scale,
            self.components[2] / scale,
        ];
        let norm = scale * sqrt(normalized.iter().map(|value| value * value).sum());
        if norm.is_finite() {
            Ok(norm)
        } else {
            Err(MathError::NonFinite)
        }
    }

    /// Adds a vector in the same frame.
    pub fn plus(self, other: Self) -> Result<Self, MathError> {
        Self::try_from_components([
            self.components[0] + other.components[0],
            self.components[1] + other.components[1],
            self.components[2] + other.components[2],
        ])
    }

    /// Subtracts a vector in the same frame.
    pub fn minus(self, other: Self) -> Result<Self, MathError> {
        Self::try_from_components([
            self.components[0] - other.components[0],
            self.components[1] - other.components[1],
            self.components[2] - other.components[2],
        ])
    }

    /// Scales a vector by a scalar.
    pub fn scaled(self, scale: f64) -> Result<Self, MathError> {
        if !scale.is_finite() {
            return Err(MathError::NonFinite);
        }
        Self::try_from_components([
            self.components[0] * scale,
            self.components[1] * scale,
            self.components[2] * scale,
        ])
    }

    pub(crate) const fn from_components_unchecked(components: [f64; 3]) -> Self {
        Self {
            components,
            frame: PhantomData,
        }
    }

    fn try_from_components(components: [f64; 3]) -> Result<Self, MathError> {
        if components.iter().any(|value| !value.is_finite()) {
            return Err(MathError::NonFinite);
        }
        Ok(Self {
            components,
            frame: PhantomData,
        })
    }
}

impl<F: Frame> Point3<F> {
    /// Creates a point from finite coordinates.
    pub fn try_new(
        first_coordinate: f64,
        second_coordinate: f64,
        third_coordinate: f64,
    ) -> Result<Self, MathError> {
        if !first_coordinate.is_finite()
            || !second_coordinate.is_finite()
            || !third_coordinate.is_finite()
        {
            return Err(MathError::NonFinite);
        }
        Ok(Self {
            components: [first_coordinate, second_coordinate, third_coordinate],
            frame: PhantomData,
        })
    }

    /// Returns point coordinates in frame-axis order.
    pub const fn components(self) -> [f64; 3] {
        self.components
    }

    /// Translates the point by a vector in the same frame.
    pub fn translated(self, vector: Vector3<F>) -> Result<Self, MathError> {
        let components = [
            self.components[0] + vector.components[0],
            self.components[1] + vector.components[1],
            self.components[2] + vector.components[2],
        ];
        if components.iter().any(|value| !value.is_finite()) {
            return Err(MathError::NonFinite);
        }
        Ok(Self {
            components,
            frame: PhantomData,
        })
    }

    /// Returns the displacement from another point in the same frame.
    pub fn displacement_from(self, other: Self) -> Result<Vector3<F>, MathError> {
        Vector3::try_from_components([
            self.components[0] - other.components[0],
            self.components[1] - other.components[1],
            self.components[2] - other.components[2],
        ])
    }
}

/// A scalar-first Hamilton quaternion representing active body-to-NED rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitQuaternion {
    components: [f64; 4],
}

impl UnitQuaternion {
    /// Maximum absolute error from unit length accepted by [`Self::try_new`].
    pub const UNIT_TOLERANCE: f64 = 1.0e-10;

    /// The identity body-to-NED rotation.
    pub const IDENTITY: Self = Self {
        components: [1.0, 0.0, 0.0, 0.0],
    };

    /// Creates a quaternion from scalar-first `(w, x, y, z)` components.
    ///
    /// Values must be finite and within [`Self::UNIT_TOLERANCE`] of unit length.
    /// Accepted round-off is normalized before storage.
    pub fn try_new(
        scalar_part: f64,
        vector_x: f64,
        vector_y: f64,
        vector_z: f64,
    ) -> Result<Self, MathError> {
        let components = [scalar_part, vector_x, vector_y, vector_z];
        if components.iter().any(|value| !value.is_finite()) {
            return Err(MathError::NonFinite);
        }
        let norm = norm4(components);
        if !norm.is_finite() || norm == 0.0 || (norm - 1.0).abs() > Self::UNIT_TOLERANCE {
            return Err(MathError::InvalidQuaternion);
        }
        Ok(Self {
            components: scale4(components, 1.0 / norm),
        })
    }

    /// Returns scalar-first `(w, x, y, z)` components.
    pub const fn components(self) -> [f64; 4] {
        self.components
    }

    /// Rotates a body-frame vector into the navigation frame.
    ///
    /// Returns [`MathError::NonFinite`] if the rotated components overflow.
    pub fn body_to_ned(self, vector: BodyVector) -> Result<NedVector, MathError> {
        let rotated = rotate(self.components, vector.components);
        NedVector::try_from_components(rotated)
    }

    /// Rotates a navigation-frame vector into the body frame.
    ///
    /// Returns [`MathError::NonFinite`] if the rotated components overflow.
    pub fn ned_to_body(self, vector: NedVector) -> Result<BodyVector, MathError> {
        let [scalar_part, vector_x, vector_y, vector_z] = self.components;
        let inverse = [scalar_part, -vector_x, -vector_y, -vector_z];
        BodyVector::try_from_components(rotate(inverse, vector.components))
    }

    /// Interpolates the shortest rotation arc to another attitude.
    pub fn slerp(self, other: Self, fraction: f64) -> Result<Self, MathError> {
        if !fraction.is_finite() {
            return Err(MathError::NonFinite);
        }
        if !(0.0..=1.0).contains(&fraction) {
            return Err(MathError::InvalidInterpolationFraction);
        }
        if fraction == 0.0 {
            return Ok(self);
        }
        if fraction == 1.0 {
            return Ok(other);
        }

        let start = self.components;
        let mut end = other.components;
        let mut cosine = start
            .into_iter()
            .zip(end)
            .map(|(left, right)| left * right)
            .sum::<f64>();
        if cosine < 0.0 {
            end = end.map(|component| -component);
            cosine = -cosine;
        }
        cosine = cosine.clamp(-1.0, 1.0);

        let components = if cosine > 0.9995 {
            core::array::from_fn(|index| start[index] + fraction * (end[index] - start[index]))
        } else {
            let angle = libm::acos(cosine);
            let sine = libm::sin(angle);
            let start_weight = libm::sin((1.0 - fraction) * angle) / sine;
            let end_weight = libm::sin(fraction * angle) / sine;
            core::array::from_fn(|index| start_weight * start[index] + end_weight * end[index])
        };
        Self::from_integrated_components(components)
    }

    pub(crate) fn derivative(self, angular_rate: BodyVector) -> [f64; 4] {
        let [scalar_part, vector_x, vector_y, vector_z] = self.components;
        let [roll_rate, pitch_rate, yaw_rate] = angular_rate.components;
        [
            -0.5 * (vector_x * roll_rate + vector_y * pitch_rate + vector_z * yaw_rate),
            0.5 * (scalar_part * roll_rate + vector_y * yaw_rate - vector_z * pitch_rate),
            0.5 * (scalar_part * pitch_rate + vector_z * roll_rate - vector_x * yaw_rate),
            0.5 * (scalar_part * yaw_rate + vector_x * pitch_rate - vector_y * roll_rate),
        ]
    }

    pub(crate) fn from_integrated_components(components: [f64; 4]) -> Result<Self, MathError> {
        if components.iter().any(|value| !value.is_finite()) {
            return Err(MathError::NonFinite);
        }
        let norm = norm4(components);
        if !norm.is_finite() || norm <= f64::MIN_POSITIVE {
            return Err(MathError::InvalidQuaternion);
        }
        Ok(Self {
            components: scale4(components, 1.0 / norm),
        })
    }
}

/// A finite, symmetric, positive-definite inertia tensor about a fixed point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InertiaTensor {
    matrix: [[f64; 3]; 3],
}

impl InertiaTensor {
    /// Creates an inertia tensor from a symmetric matrix in kg m².
    pub fn try_new(matrix: [[f64; 3]; 3]) -> Result<Self, MathError> {
        if matrix.iter().flatten().any(|value| !value.is_finite()) {
            return Err(MathError::NonFinite);
        }
        if matrix[0][1] != matrix[1][0]
            || matrix[0][2] != matrix[2][0]
            || matrix[1][2] != matrix[2][1]
        {
            return Err(MathError::AsymmetricTensor);
        }
        let leading_minor_2 = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
        let determinant = matrix[0][0]
            * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
            - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
            + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);
        if matrix[0][0] <= 0.0
            || leading_minor_2 <= 0.0
            || determinant <= 0.0
            || !leading_minor_2.is_finite()
            || !determinant.is_finite()
        {
            return Err(MathError::NonPositiveDefiniteTensor);
        }
        Ok(Self { matrix })
    }

    /// Creates a diagonal positive-definite tensor in kg m².
    pub fn diagonal(ixx: f64, iyy: f64, izz: f64) -> Result<Self, MathError> {
        Self::try_new([[ixx, 0.0, 0.0], [0.0, iyy, 0.0], [0.0, 0.0, izz]])
    }

    /// Returns the matrix in body-axis order.
    pub const fn matrix(self) -> [[f64; 3]; 3] {
        self.matrix
    }

    pub(crate) fn multiply(self, vector: [f64; 3]) -> [f64; 3] {
        [
            self.matrix[0][0] * vector[0]
                + self.matrix[0][1] * vector[1]
                + self.matrix[0][2] * vector[2],
            self.matrix[1][0] * vector[0]
                + self.matrix[1][1] * vector[1]
                + self.matrix[1][2] * vector[2],
            self.matrix[2][0] * vector[0]
                + self.matrix[2][1] * vector[1]
                + self.matrix[2][2] * vector[2],
        ]
    }
}

fn rotate(quaternion: [f64; 4], vector: [f64; 3]) -> [f64; 3] {
    let [scalar_part, vector_x, vector_y, vector_z] = quaternion;
    let quaternion_vector = [vector_x, vector_y, vector_z];
    let first_cross = cross3(quaternion_vector, vector);
    let twice_cross = [
        2.0 * first_cross[0],
        2.0 * first_cross[1],
        2.0 * first_cross[2],
    ];
    let second_cross = cross3(quaternion_vector, twice_cross);
    [
        vector[0] + scalar_part * twice_cross[0] + second_cross[0],
        vector[1] + scalar_part * twice_cross[1] + second_cross[1],
        vector[2] + scalar_part * twice_cross[2] + second_cross[2],
    ]
}

pub(crate) fn cross3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn norm4(components: [f64; 4]) -> f64 {
    let scale = components
        .iter()
        .map(|component| component.abs())
        .fold(0.0, f64::max);
    if scale == 0.0 {
        return 0.0;
    }
    let normalized = scale4(components, 1.0 / scale);
    scale * sqrt(normalized.iter().map(|value| value * value).sum())
}

fn scale4(components: [f64; 4], scale: f64) -> [f64; 4] {
    [
        components[0] * scale,
        components[1] * scale,
        components[2] * scale,
        components[3] * scale,
    ]
}

fn sqrt(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let mut estimate = f64::from_bits((value.to_bits() >> 1) + 0x1ff8_0000_0000_0000);
    for _ in 0..8 {
        estimate = 0.5 * (estimate + value / estimate);
    }
    estimate
}

pub(crate) fn hypot2(first: f64, second: f64) -> f64 {
    let scale = first.abs().max(second.abs());
    if scale == 0.0 {
        return 0.0;
    }
    let normalized_first = first / scale;
    let normalized_second = second / scale;
    scale * sqrt(normalized_first * normalized_first + normalized_second * normalized_second)
}

pub(crate) fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}
