import { pose, quaternion, vec3 } from "../contracts/math.js";
import type { Pose, Quaternion, Vec3 } from "../contracts/math.js";
import type { CameraTrackPoint, CinematicCameraView, FixedCameraPoint } from "../contracts/camera.js";
import type { FlightCameraMode, FlightRenderPose } from "../contracts/runtime.js";

export type CinematicCameraMode = Exclude<FlightCameraMode, "pilot" | "chase">;
const MAX_TRACK_ANGULAR_RATE_RADIANS_PER_SECOND = 2 * Math.PI / 3;

export function isCinematicCameraMode(mode: FlightCameraMode): mode is CinematicCameraMode {
  return mode !== "pilot" && mode !== "chase";
}

export function cinematicCameraView(
  mode: CinematicCameraMode,
  flight: FlightRenderPose,
  recordTimeSeconds: number,
  fixedPoints: readonly FixedCameraPoint[],
  track: readonly CameraTrackPoint[] = []
): CinematicCameraView | null {
  if (!Number.isFinite(recordTimeSeconds) || recordTimeSeconds < 0) {
    throw new RangeError("Record time must be finite and non-negative");
  }
  const target = vec3(
    flight.datumPositionNed.east,
    -flight.datumPositionNed.down,
    -flight.datumPositionNed.north
  );
  let position: Vec3;
  let verticalFieldOfViewDegrees: number;
  switch (mode) {
    case "platform":
    case "shore":
    case "telephoto": {
      const fixedPoint = fixedPoints.find((point) => point.id === mode);
      if (fixedPoint === undefined) return null;
      position = vec3(fixedPoint.eastMeters, fixedPoint.altitudeMeters, -fixedPoint.northMeters);
      verticalFieldOfViewDegrees = mode === "telephoto" ? 24 : mode === "shore" ? 40 : 60;
      break;
    }
    case "overhead":
      position = vec3(target.x, target.y + 30, target.z + 2);
      verticalFieldOfViewDegrees = 65;
      break;
    case "side":
      position = vec3(target.x + 30, target.y + 9, target.z - 4);
      verticalFieldOfViewDegrees = 50;
      break;
    case "front":
      position = vec3(target.x, target.y + 7, target.z - 28);
      verticalFieldOfViewDegrees = 55;
      break;
    case "orbit": {
      const angle = recordTimeSeconds * 0.35;
      position = vec3(target.x + 28 * Math.sin(angle), target.y + 10, target.z + 28 * Math.cos(angle));
      verticalFieldOfViewDegrees = 50;
      break;
    }
  }
  const cameraPose = worldLookAtPose(position, target);
  if (cameraPose === null) return null;
  const fixedShot = mode === "platform" || mode === "shore" || mode === "telephoto";
  const trackedOrientation = fixedShot ? boundedFixedOrientation(position, track, recordTimeSeconds) : null;
  return Object.freeze({
    pose: trackedOrientation === null ? cameraPose : pose(position, trackedOrientation),
    verticalFieldOfViewDegrees
  });
}

export function worldLookAtPose(position: Vec3, target: Vec3): Pose | null {
  const backward = unit(vec3(position.x - target.x, position.y - target.y, position.z - target.z));
  if (backward === null) return null;
  const referenceUp = Math.abs(backward.y) > 0.98 ? vec3(0, 0, -1) : vec3(0, 1, 0);
  const right = unit(cross(referenceUp, backward));
  if (right === null) return null;
  const cameraUp = cross(backward, right);
  return pose(position, orientationFromBasis(right, cameraUp, backward));
}

function boundedFixedOrientation(
  position: Vec3,
  track: readonly CameraTrackPoint[],
  recordTimeSeconds: number
): Quaternion | null {
  const first = track[0];
  if (first === undefined) return null;
  const firstPose = worldLookAtPose(position, trackTarget(first));
  if (firstPose === null) return null;
  let previousOrientation = firstPose.orientation;
  let previousTimeSeconds = first.timeSeconds;
  if (recordTimeSeconds <= previousTimeSeconds) return previousOrientation;

  for (let index = 1; index < track.length; index += 1) {
    const sample = track[index];
    if (sample === undefined || sample.timeSeconds <= previousTimeSeconds) continue;
    const intervalSeconds = sample.timeSeconds - previousTimeSeconds;
    const desiredPose = worldLookAtPose(position, trackTarget(sample));
    const nextOrientation = desiredPose === null
      ? previousOrientation
      : rotateToward(previousOrientation, desiredPose.orientation, intervalSeconds);
    if (recordTimeSeconds <= sample.timeSeconds) {
      return slerp(previousOrientation, nextOrientation, (recordTimeSeconds - previousTimeSeconds) / intervalSeconds);
    }
    previousOrientation = nextOrientation;
    previousTimeSeconds = sample.timeSeconds;
  }
  return previousOrientation;
}

function trackTarget(sample: CameraTrackPoint): Vec3 {
  return vec3(sample.eastMeters, sample.altitudeMeters, -sample.northMeters);
}

function rotateToward(previous: Quaternion, desired: Quaternion, intervalSeconds: number): Quaternion {
  const cosine = Math.abs(quaternionDot(previous, desired));
  const angularDistance = 2 * Math.acos(Math.min(1, cosine));
  if (angularDistance < 1.0e-12) return desired;
  const fraction = Math.min(1, MAX_TRACK_ANGULAR_RATE_RADIANS_PER_SECOND * intervalSeconds / angularDistance);
  return slerp(previous, desired, fraction);
}

function slerp(start: Quaternion, end: Quaternion, fraction: number): Quaternion {
  const sign = quaternionDot(start, end) < 0 ? -1 : 1;
  const cosine = Math.min(1, Math.max(-1, quaternionDot(start, end) * sign));
  if (cosine > 0.9995) {
    return quaternion(
      start.w + fraction * (sign * end.w - start.w),
      start.x + fraction * (sign * end.x - start.x),
      start.y + fraction * (sign * end.y - start.y),
      start.z + fraction * (sign * end.z - start.z)
    );
  }
  const angle = Math.acos(cosine);
  const denominator = Math.sin(angle);
  const startWeight = Math.sin((1 - fraction) * angle) / denominator;
  const endWeight = sign * Math.sin(fraction * angle) / denominator;
  return quaternion(
    startWeight * start.w + endWeight * end.w,
    startWeight * start.x + endWeight * end.x,
    startWeight * start.y + endWeight * end.y,
    startWeight * start.z + endWeight * end.z
  );
}

function quaternionDot(left: Quaternion, right: Quaternion): number {
  return left.w * right.w + left.x * right.x + left.y * right.y + left.z * right.z;
}

function orientationFromBasis(right: Vec3, cameraUp: Vec3, backward: Vec3): Quaternion {
  const trace = right.x + cameraUp.y + backward.z;
  if (trace > 0) {
    const scale = 2 * Math.sqrt(trace + 1);
    return quaternion(scale / 4, (cameraUp.z - backward.y) / scale,
      (backward.x - right.z) / scale, (right.y - cameraUp.x) / scale);
  }
  if (right.x > cameraUp.y && right.x > backward.z) {
    const scale = 2 * Math.sqrt(1 + right.x - cameraUp.y - backward.z);
    return quaternion((cameraUp.z - backward.y) / scale, scale / 4,
      (cameraUp.x + right.y) / scale, (backward.x + right.z) / scale);
  }
  if (cameraUp.y > backward.z) {
    const scale = 2 * Math.sqrt(1 + cameraUp.y - right.x - backward.z);
    return quaternion((backward.x - right.z) / scale, (cameraUp.x + right.y) / scale,
      scale / 4, (backward.y + cameraUp.z) / scale);
  }
  const scale = 2 * Math.sqrt(1 + backward.z - right.x - cameraUp.y);
  return quaternion((right.y - cameraUp.x) / scale, (backward.x + right.z) / scale,
    (backward.y + cameraUp.z) / scale, scale / 4);
}

function cross(left: Vec3, right: Vec3): Vec3 {
  return vec3(
    left.y * right.z - left.z * right.y,
    left.z * right.x - left.x * right.z,
    left.x * right.y - left.y * right.x
  );
}

function unit(value: Vec3): Vec3 | null {
  const magnitude = Math.hypot(value.x, value.y, value.z);
  return magnitude < 1.0e-9 ? null : vec3(value.x / magnitude, value.y / magnitude, value.z / magnitude);
}
