export interface Vec2 {
  readonly x: number;
  readonly y: number;
}

export interface Vec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface Quaternion {
  readonly w: number;
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface Pose {
  readonly position: Vec3;
  readonly orientation: Quaternion;
}

export const IDENTITY_POSE = pose(vec3(0, 0, 0), quaternion(1, 0, 0, 0));

export function vec2(x: number, y: number): Vec2 {
  return Object.freeze({ x: finite(x, "x"), y: finite(y, "y") });
}

export function vec3(x: number, y: number, z: number): Vec3 {
  return Object.freeze({ x: finite(x, "x"), y: finite(y, "y"), z: finite(z, "z") });
}

export function quaternion(w: number, x: number, y: number, z: number): Quaternion {
  const scalar = finite(w, "w");
  const first = finite(x, "x");
  const second = finite(y, "y");
  const third = finite(z, "z");
  const magnitude = Math.hypot(scalar, first, second, third);
  if (magnitude <= Number.EPSILON) throw new RangeError("A quaternion must have non-zero magnitude");
  return Object.freeze({
    w: scalar / magnitude,
    x: first / magnitude,
    y: second / magnitude,
    z: third / magnitude
  });
}

export function pose(position: Vec3, orientation: Quaternion): Pose {
  return Object.freeze({ position, orientation });
}

export function addVec3(left: Vec3, right: Vec3): Vec3 {
  return vec3(left.x + right.x, left.y + right.y, left.z + right.z);
}

export function subtractVec3(left: Vec3, right: Vec3): Vec3 {
  return vec3(left.x - right.x, left.y - right.y, left.z - right.z);
}

export function scaleVec3(value: Vec3, scale: number): Vec3 {
  return vec3(value.x * finite(scale, "scale"), value.y * scale, value.z * scale);
}

export function multiplyQuaternion(left: Quaternion, right: Quaternion): Quaternion {
  return quaternion(
    left.w * right.w - left.x * right.x - left.y * right.y - left.z * right.z,
    left.w * right.x + left.x * right.w + left.y * right.z - left.z * right.y,
    left.w * right.y - left.x * right.z + left.y * right.w + left.z * right.x,
    left.w * right.z + left.x * right.y - left.y * right.x + left.z * right.w
  );
}

export function rotateVec3(rotation: Quaternion, value: Vec3): Vec3 {
  const crossX = rotation.y * value.z - rotation.z * value.y;
  const crossY = rotation.z * value.x - rotation.x * value.z;
  const crossZ = rotation.x * value.y - rotation.y * value.x;
  const twiceCrossX = 2 * crossX;
  const twiceCrossY = 2 * crossY;
  const twiceCrossZ = 2 * crossZ;
  return vec3(
    value.x + rotation.w * twiceCrossX + rotation.y * twiceCrossZ - rotation.z * twiceCrossY,
    value.y + rotation.w * twiceCrossY + rotation.z * twiceCrossX - rotation.x * twiceCrossZ,
    value.z + rotation.w * twiceCrossZ + rotation.x * twiceCrossY - rotation.y * twiceCrossX
  );
}

export function composePose(parent: Pose, child: Pose): Pose {
  return pose(
    addVec3(parent.position, rotateVec3(parent.orientation, child.position)),
    multiplyQuaternion(parent.orientation, child.orientation)
  );
}

export function inversePose(value: Pose): Pose {
  const orientation = quaternion(value.orientation.w, -value.orientation.x, -value.orientation.y, -value.orientation.z);
  return pose(rotateVec3(orientation, scaleVec3(value.position, -1)), orientation);
}

export function transformPoint(transform: Pose, point: Vec3): Vec3 {
  return addVec3(transform.position, rotateVec3(transform.orientation, point));
}

function finite(value: number, name: string): number {
  if (!Number.isFinite(value)) throw new RangeError(`${name} must be finite`);
  return value;
}
