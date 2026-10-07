import { PerspectiveCamera, Quaternion, Vector3 } from "three";
import type { StereoCamera } from "three";
import { composePose, inversePose, pose, quaternion, vec3 } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import { copyProjectionMatrix, unavailableViewerFrame } from "../../contracts/viewer-frame.js";
import type { BinocularGeometry, EyeGeometry, ViewerFrame } from "../../contracts/viewer-frame.js";

export function captureXrViewerFrame(viewer: XRViewerPose | null): ViewerFrame {
  if (viewer === null) return unavailableViewerFrame("viewer-unavailable");
  const head = copyXrPose(viewer.transform);
  if (head === null) return unavailableViewerFrame("invalid-view-geometry");
  const views = viewer.views;
  if (views.length !== 2 || views.filter((view) => view.eye === "left").length !== 1 ||
      views.filter((view) => view.eye === "right").length !== 1) {
    return unavailableViewerFrame("unsupported-view-configuration", head);
  }
  const captureEye = <Eye extends "left" | "right">(eye: Eye): EyeGeometry<Eye> | null => {
    const view = views.find((entry) => entry.eye === eye);
    if (view === undefined) return null;
    const trackingFromEye = copyXrPose(view.transform);
    const projection = copyProjectionMatrix(view.projectionMatrix);
    if (trackingFromEye === null || projection === null) return null;
    return Object.freeze({ eye, headFromEye: composePose(inversePose(head), trackingFromEye), projection });
  };
  const left = captureEye("left");
  const right = captureEye("right");
  if (left === null || right === null) return unavailableViewerFrame("invalid-view-geometry", head);
  const eyes: BinocularGeometry = Object.freeze([left, right]);
  return Object.freeze({ source: "runtime-derived", trackingFromHead: head, eyes });
}

export function captureConfiguredViewerFrame(camera: PerspectiveCamera, stereo: StereoCamera): ViewerFrame {
  stereo.update(camera);
  const position = new Vector3();
  const orientation = new Quaternion();
  const scale = new Vector3();
  camera.matrixWorld.decompose(position, orientation, scale);
  const trackingFromHead = pose(vec3(position.x, position.y, position.z), quaternion(orientation.w, orientation.x, orientation.y, orientation.z));
  const captureEye = <Eye extends "left" | "right">(eye: Eye, eyeCamera: PerspectiveCamera): EyeGeometry<Eye> | null => {
    eyeCamera.matrix.decompose(position, orientation, scale);
    const trackingFromEye = pose(vec3(position.x, position.y, position.z), quaternion(orientation.w, orientation.x, orientation.y, orientation.z));
    const projection = copyProjectionMatrix(eyeCamera.projectionMatrix.elements);
    if (projection === null) return null;
    return Object.freeze({ eye, headFromEye: composePose(inversePose(trackingFromHead), trackingFromEye), projection });
  };
  const left = captureEye("left", stereo.cameraL);
  const right = captureEye("right", stereo.cameraR);
  if (left === null || right === null) return unavailableViewerFrame("invalid-view-geometry");
  const eyes: BinocularGeometry = Object.freeze([left, right]);
  return Object.freeze({ source: "configured", trackingFromHead: null, eyes });
}

function copyXrPose(value: XRRigidTransform): Pose | null {
  const position = value.position;
  const orientation = value.orientation;
  if (![position.x, position.y, position.z, orientation.w, orientation.x, orientation.y, orientation.z].every(Number.isFinite) ||
      Math.abs(Math.hypot(orientation.w, orientation.x, orientation.y, orientation.z) - 1) > 1.0e-5) return null;
  return pose(vec3(position.x, position.y, position.z), quaternion(orientation.w, orientation.x, orientation.y, orientation.z));
}
