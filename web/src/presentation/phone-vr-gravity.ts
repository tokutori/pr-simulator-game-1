import { multiplyQuaternion, quaternion, rotateVec3, vec3 } from "../render/contracts/math.js";
import type { Quaternion, Vec3 } from "../render/contracts/math.js";

export const PHONE_GRAVITY_MINIMUM_HORIZONTAL_NORM = Math.sin(Math.PI / 180);
const ROTATION_NORM_TOLERANCE = 1e-10;
const CAMERA_FORWARD = vec3(0, 0, -1);

export type PhoneGravityEvidence =
  | { readonly kind: "earth-z-up" }
  | { readonly kind: "relative-reference-up"; readonly referenceUp: Vec3 }
  | { readonly kind: "unavailable" };

export interface PhoneGravityCalibrationInput {
  readonly referenceFromDevice: Quaternion;
  readonly screenAngleDegrees: number;
  readonly evidence: PhoneGravityEvidence;
}

export interface CoherentPhoneGravityObservation {
  readonly referenceFromDeviceAtObservation: Quaternion;
  readonly deviceUpAtObservation: Vec3;
}

export interface PhoneGravityCalibration {
  readonly trackingFromReference: Quaternion;
}

export type PhoneGravityInvalid = {
  readonly kind: "invalid";
  readonly reason: "nonfinite" | "invalid-rotation" | "invalid-up";
};

export type PhoneGravityEvidenceResult =
  | { readonly kind: "derived"; readonly evidence: Extract<PhoneGravityEvidence, { kind: "relative-reference-up" }> }
  | PhoneGravityInvalid;

export type PhoneGravityCalibrationResult =
  | { readonly kind: "calibrated"; readonly calibration: PhoneGravityCalibration; readonly viewer: Quaternion }
  | { readonly kind: "pending"; readonly reason: "missing-gravity" | "heading-degenerate" }
  | PhoneGravityInvalid;

export type PhoneGravityViewerResult =
  | { readonly kind: "oriented"; readonly viewer: Quaternion }
  | PhoneGravityInvalid;

export type PhoneGravityRecenterResult =
  | { readonly kind: "recentered"; readonly calibration: PhoneGravityCalibration; readonly viewer: Quaternion; readonly newTrackingFromOldTracking: Quaternion }
  | { readonly kind: "retained"; readonly reason: "heading-degenerate"; readonly calibration: PhoneGravityCalibration; readonly viewer: Quaternion }
  | PhoneGravityInvalid;

export function referenceUpFromCoherentPhoneObservation(observation: CoherentPhoneGravityObservation): PhoneGravityEvidenceResult {
  const rotation = checkedRotation(observation.referenceFromDeviceAtObservation);
  if (rotation.kind === "invalid") return rotation;
  const up = checkedUp(observation.deviceUpAtObservation);
  if (up.kind === "invalid") return up;
  return Object.freeze({
    kind: "derived",
    evidence: Object.freeze({ kind: "relative-reference-up", referenceUp: rotateVec3(rotation.value, up.value) })
  });
}

export function calibratePhoneGravity(input: PhoneGravityCalibrationInput): PhoneGravityCalibrationResult {
  const screen = referenceFromScreen(input.referenceFromDevice, input.screenAngleDegrees);
  if (screen.kind === "invalid") return screen;
  if (input.evidence.kind === "unavailable") return Object.freeze({ kind: "pending", reason: "missing-gravity" });
  const up = checkedUp(input.evidence.kind === "earth-z-up" ? vec3(0, 0, 1) : input.evidence.referenceUp);
  if (up.kind === "invalid") return up;
  const forward = rotateVec3(screen.value, CAMERA_FORWARD);
  const vertical = dot(forward, up.value);
  const horizontal = vec3(
    forward.x - up.value.x * vertical,
    forward.y - up.value.y * vertical,
    forward.z - up.value.z * vertical
  );
  const magnitude = Math.hypot(horizontal.x, horizontal.y, horizontal.z);
  if (magnitude < PHONE_GRAVITY_MINIMUM_HORIZONTAL_NORM) {
    return Object.freeze({ kind: "pending", reason: "heading-degenerate" });
  }
  const heading = vec3(horizontal.x / magnitude, horizontal.y / magnitude, horizontal.z / magnitude);
  const right = cross(heading, up.value);
  const back = vec3(-heading.x, -heading.y, -heading.z);
  const referenceFromTracking = rotationFromBasis(right, up.value, back);
  const trackingFromReference = quaternion(referenceFromTracking.w, -referenceFromTracking.x, -referenceFromTracking.y, -referenceFromTracking.z);
  return Object.freeze({
    kind: "calibrated",
    calibration: Object.freeze({ trackingFromReference }),
    viewer: multiplyQuaternion(trackingFromReference, screen.value)
  });
}

export function phoneGravityViewer(
  calibration: PhoneGravityCalibration,
  referenceFromDevice: Quaternion,
  screenAngleDegrees: number
): PhoneGravityViewerResult {
  const basis = checkedRotation(calibration.trackingFromReference);
  if (basis.kind === "invalid") return basis;
  const screen = referenceFromScreen(referenceFromDevice, screenAngleDegrees);
  if (screen.kind === "invalid") return screen;
  return Object.freeze({ kind: "oriented", viewer: multiplyQuaternion(basis.value, screen.value) });
}

export function recenterPhoneGravity(
  calibration: PhoneGravityCalibration,
  referenceFromDevice: Quaternion,
  screenAngleDegrees: number
): PhoneGravityRecenterResult {
  const current = phoneGravityViewer(calibration, referenceFromDevice, screenAngleDegrees);
  if (current.kind === "invalid") return current;
  const forward = rotateVec3(current.viewer, CAMERA_FORWARD);
  if (Math.hypot(forward.x, forward.z) < PHONE_GRAVITY_MINIMUM_HORIZONTAL_NORM) {
    return Object.freeze({ kind: "retained", reason: "heading-degenerate", calibration, viewer: current.viewer });
  }
  const yaw = Math.atan2(-forward.x, -forward.z);
  const newTrackingFromOldTracking = quaternion(Math.cos(yaw / 2), 0, -Math.sin(yaw / 2), 0);
  return Object.freeze({
    kind: "recentered",
    calibration: Object.freeze({ trackingFromReference: multiplyQuaternion(newTrackingFromOldTracking, calibration.trackingFromReference) }),
    viewer: multiplyQuaternion(newTrackingFromOldTracking, current.viewer),
    newTrackingFromOldTracking
  });
}

type Checked<Value> = { readonly kind: "valid"; readonly value: Value } | PhoneGravityInvalid;

function referenceFromScreen(referenceFromDevice: Quaternion, screenAngleDegrees: number): Checked<Quaternion> {
  const rotation = checkedRotation(referenceFromDevice);
  if (rotation.kind === "invalid") return rotation;
  if (!Number.isFinite(screenAngleDegrees)) return Object.freeze({ kind: "invalid", reason: "nonfinite" });
  const halfAngle = -(screenAngleDegrees % 360) * Math.PI / 360;
  return Object.freeze({ kind: "valid", value: multiplyQuaternion(rotation.value, quaternion(Math.cos(halfAngle), 0, 0, Math.sin(halfAngle))) });
}

function checkedRotation(value: Quaternion): Checked<Quaternion> {
  if (![value.w, value.x, value.y, value.z].every(Number.isFinite)) return Object.freeze({ kind: "invalid", reason: "nonfinite" });
  const magnitude = Math.hypot(value.w, value.x, value.y, value.z);
  if (Math.abs(magnitude - 1) > ROTATION_NORM_TOLERANCE) return Object.freeze({ kind: "invalid", reason: "invalid-rotation" });
  return Object.freeze({ kind: "valid", value: quaternion(value.w, value.x, value.y, value.z) });
}

function checkedUp(value: Vec3): Checked<Vec3> {
  if (![value.x, value.y, value.z].every(Number.isFinite)) return Object.freeze({ kind: "invalid", reason: "nonfinite" });
  const scale = Math.max(Math.abs(value.x), Math.abs(value.y), Math.abs(value.z));
  if (scale === 0) return Object.freeze({ kind: "invalid", reason: "invalid-up" });
  const scaled = vec3(value.x / scale, value.y / scale, value.z / scale);
  const magnitude = Math.hypot(scaled.x, scaled.y, scaled.z);
  return Object.freeze({ kind: "valid", value: vec3(scaled.x / magnitude, scaled.y / magnitude, scaled.z / magnitude) });
}

function dot(left: Vec3, right: Vec3): number {
  return left.x * right.x + left.y * right.y + left.z * right.z;
}

function cross(left: Vec3, right: Vec3): Vec3 {
  return vec3(left.y * right.z - left.z * right.y, left.z * right.x - left.x * right.z, left.x * right.y - left.y * right.x);
}

function rotationFromBasis(right: Vec3, up: Vec3, back: Vec3): Quaternion {
  const trace = right.x + up.y + back.z;
  if (trace > 0) {
    const scale = 2 * Math.sqrt(trace + 1);
    return quaternion(scale / 4, (up.z - back.y) / scale, (back.x - right.z) / scale, (right.y - up.x) / scale);
  }
  if (right.x > up.y && right.x > back.z) {
    const scale = 2 * Math.sqrt(1 + right.x - up.y - back.z);
    return quaternion((up.z - back.y) / scale, scale / 4, (up.x + right.y) / scale, (back.x + right.z) / scale);
  }
  if (up.y > back.z) {
    const scale = 2 * Math.sqrt(1 + up.y - right.x - back.z);
    return quaternion((back.x - right.z) / scale, (up.x + right.y) / scale, scale / 4, (back.y + up.z) / scale);
  }
  const scale = 2 * Math.sqrt(1 + back.z - right.x - up.y);
  return quaternion((right.y - up.x) / scale, (back.x + right.z) / scale, (back.y + up.z) / scale, scale / 4);
}
