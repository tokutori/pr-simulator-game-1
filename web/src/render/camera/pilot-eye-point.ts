import { pose, vec3 } from "../contracts/math.js";
import { IDENTITY_POSE, multiplyQuaternion, quaternion } from "../contracts/math.js";
import type { Pose, Quaternion, Vec3 } from "../contracts/math.js";

const FRD_TO_THREE = quaternion(0.5, -0.5, 0.5, -0.5);
const THREE_TO_FRD = quaternion(FRD_TO_THREE.w, -FRD_TO_THREE.x, -FRD_TO_THREE.y, -FRD_TO_THREE.z);

export const SYNTHETIC_PILOT_EYE_POINT = createPilotEyePoint(
  vec3(0.55, 0, -0.15),
  0,
  IDENTITY_POSE.orientation
);

export interface PilotEyePoint {
  readonly initialOffsetFrd: Vec3;
  readonly initialPilotPositionMeters: number;
  readonly orientationFrd: Quaternion;
}

export function createPilotEyePoint(
  initialOffsetFrd: Vec3,
  initialPilotPositionMeters: number,
  orientationFrd: Quaternion
): PilotEyePoint {
  if (!Number.isFinite(initialPilotPositionMeters)) throw new RangeError("Initial pilot position must be finite");
  return Object.freeze({ initialOffsetFrd, initialPilotPositionMeters, orientationFrd });
}

export function pilotEyePoseFrd(
  point: PilotEyePoint,
  pilotPositionMeters: number,
  initialPilotPositionMeters = point.initialPilotPositionMeters
): Pose {
  if (!Number.isFinite(pilotPositionMeters)) throw new RangeError("Pilot position must be finite");
  if (!Number.isFinite(initialPilotPositionMeters)) throw new RangeError("Initial pilot position must be finite");
  const displacement = pilotPositionMeters - initialPilotPositionMeters;
  return pose(
    vec3(point.initialOffsetFrd.x + displacement, point.initialOffsetFrd.y, point.initialOffsetFrd.z),
    point.orientationFrd
  );
}

export function pilotEyePoseThree(
  point: PilotEyePoint,
  pilotPositionMeters: number,
  initialPilotPositionMeters = point.initialPilotPositionMeters
): Pose {
  const eyeFrd = pilotEyePoseFrd(point, pilotPositionMeters, initialPilotPositionMeters);
  return poseFrdToThree(eyeFrd);
}

export function poseFrdToThree(value: Pose): Pose {
  return pose(
    vec3(value.position.y, -value.position.z, -value.position.x),
    multiplyQuaternion(
      multiplyQuaternion(FRD_TO_THREE, value.orientation),
      THREE_TO_FRD
    )
  );
}
