import { PerspectiveCamera, Quaternion, Vector3 } from "three";
import type { StereoCamera } from "three";
import { xr_capture_viewer_frame_json } from "../../../../pkg/birdman_game_wasm.js";
import type { XrViewerObservation } from "../../../generated/xr-viewer-observation.js";
import type { Pose } from "../../contracts/math.js";
import type { ViewerFrame } from "../../contracts/viewer-frame.js";

export function captureXrViewerFrame(viewer: XRViewerPose | null): ViewerFrame {
  return capture({ source: "runtime-derived", trackingFromHead: viewer === null ? null : copyXrPose(viewer.transform),
    views: viewer?.views.map(view => ({ eye: view.eye === "none" ? null : view.eye,
      trackingFromEye: copyXrPose(view.transform), projection: Array.from(view.projectionMatrix),
      depthRange: "negative-one-to-one" })) ?? [] });
}

export function captureConfiguredViewerFrame(camera: PerspectiveCamera, stereo: StereoCamera): ViewerFrame {
  stereo.update(camera);
  const position = new Vector3();
  const orientation = new Quaternion();
  const scale = new Vector3();
  const readPose = (matrix: PerspectiveCamera["matrix"]): Pose => {
    matrix.decompose(position, orientation, scale);
    return { position: { x: position.x, y: position.y, z: position.z },
      orientation: { w: orientation.w, x: orientation.x, y: orientation.y, z: orientation.z } };
  };
  return capture({ source: "configured", trackingFromHead: readPose(camera.matrixWorld),
    views: [stereo.cameraL, stereo.cameraR].map((eyeCamera, index) => ({
      eye: index === 0 ? "left" : "right", trackingFromEye: readPose(eyeCamera.matrix),
      projection: Array.from(eyeCamera.projectionMatrix.elements), depthRange: "negative-one-to-one"
    })) });
}

function capture(observation: XrViewerObservation): ViewerFrame {
  return JSON.parse(xr_capture_viewer_frame_json(JSON.stringify(observation))) as ViewerFrame;
}

function copyXrPose(value: XRRigidTransform): Pose {
  return { position: { x: value.position.x, y: value.position.y, z: value.position.z },
    orientation: { w: value.orientation.w, x: value.orientation.x, y: value.orientation.y, z: value.orientation.z } };
}
