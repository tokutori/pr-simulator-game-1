import { IDENTITY_POSE, pose, quaternion, vec3 } from "../contracts/math.js";
import type { Pose } from "../contracts/math.js";
import type { FlightCameraMode } from "../contracts/runtime.js";

const CHASE_PITCH_RADIANS = Math.atan2(4, 12);

export function replayCameraPoseFrd(mode: FlightCameraMode): Pose {
  if (mode === "pilot") return IDENTITY_POSE;
  return pose(
    vec3(-12, 0, -4),
    quaternion(Math.cos(CHASE_PITCH_RADIANS / 2), 0, -Math.sin(CHASE_PITCH_RADIANS / 2), 0)
  );
}
