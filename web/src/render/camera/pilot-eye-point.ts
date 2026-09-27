import { pose, vec3 } from "../contracts/math.js";
import type { Pose, Quaternion, Vec3 } from "../contracts/math.js";

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

export function pilotEyePoseFrd(point: PilotEyePoint, pilotPositionMeters: number): Pose {
  if (!Number.isFinite(pilotPositionMeters)) throw new RangeError("Pilot position must be finite");
  const displacement = pilotPositionMeters - point.initialPilotPositionMeters;
  return pose(
    vec3(point.initialOffsetFrd.x + displacement, point.initialOffsetFrd.y, point.initialOffsetFrd.z),
    point.orientationFrd
  );
}
