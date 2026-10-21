import { pose, quaternion, vec3 } from "../contracts/math.js";
import type { ReplayCameraView } from "../../generated/replay-camera.js";
import type { CinematicCameraView } from "../contracts/camera.js";
import type { FlightCameraMode } from "../contracts/runtime.js";

export function projectReplayCamera(view: ReplayCameraView): Readonly<{ mode: FlightCameraMode; cinematic: CinematicCameraView | null }> {
  if (view.camera === "auto") throw new RangeError("Rust camera director must resolve Auto");
  const camera = view.cinematic;
  return Object.freeze({ mode: view.camera, cinematic: camera === null ? null : Object.freeze({
    pose: pose(vec3(...camera.position_m), quaternion(...camera.orientation_wxyz)),
    verticalFieldOfViewDegrees: camera.vertical_field_of_view_degrees
  }) });
}
