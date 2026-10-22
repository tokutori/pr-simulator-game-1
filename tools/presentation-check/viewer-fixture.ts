import { Matrix4, PerspectiveCamera, Quaternion, StereoCamera, Vector3, Vector4 } from "three";
import { initializeWasmFixture } from "../game-check/wasm-fixture.js";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import type { Pose, Vec3 } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, PanelFrame } from "../../web/src/render/contracts/runtime.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { EyeGeometry, ProjectionMatrix, ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";

initializeWasmFixture();

export function configuredViewerFixture(width = 1280, height = 720, near = 0.1, far = 100, verticalFov = 60): ViewerFrame {
  const camera = new PerspectiveCamera(verticalFov, width / height, near, far);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  return captureConfiguredViewerFrame(camera, stereo);
}

export function runtimeViewerFixture(head: Pose | null = IDENTITY_POSE): ViewerFrame {
  if (head === null) return unavailableViewerFrame("viewer-unavailable");
  const configured = configuredViewerFixture();
  return configured.source === "unavailable" ? configured : Object.freeze({ ...configured, source: "runtime-derived", trackingFromHead: head });
}

export function visiblePanelFrame(frame: BackendFrame): Extract<PanelFrame, { readonly kind: "visible" }> {
  if (frame.panel.kind !== "visible") throw new Error(`Expected a visible panel, received ${frame.panel.kind}`);
  return frame.panel;
}

export function panelFrameCursor(frame: BackendFrame) {
  return frame.panel.kind === "visible" ? frame.panel.cursor : null;
}

export function copyProjectionMatrix(values: ArrayLike<number>): ProjectionMatrix | null {
  const copied = Array.from(values);
  return copied.length === 16 && copied.every(Number.isFinite) && new Matrix4().fromArray(copied).determinant() !== 0
    ? new Matrix4().fromArray(copied).elements : null;
}

function poseMatrix(value: Pose): Matrix4 {
  return new Matrix4().compose(new Vector3(value.position.x, value.position.y, value.position.z),
    new Quaternion(value.orientation.x, value.orientation.y, value.orientation.z, value.orientation.w), new Vector3(1, 1, 1));
}

export function projectHeadPoint(eye: EyeGeometry, point: Vec3): Vec3 | null {
  const projected = new Vector4(point.x, point.y, point.z, 1).applyMatrix4(poseMatrix(eye.headFromEye).invert())
    .applyMatrix4(new Matrix4().fromArray(eye.projection));
  if (![projected.x, projected.y, projected.z, projected.w].every(Number.isFinite) || projected.w <= 0) return null;
  return { x: projected.x / projected.w, y: projected.y / projected.w, z: projected.z / projected.w };
}

export function headPlaneFitsViews(frame: ViewerFrame, headFromPlane: Pose, width: number, height: number, margin = 0): boolean {
  if (frame.source === "unavailable") return false;
  const plane = poseMatrix(headFromPlane);
  return frame.eyes.every(eye => [-1, 1].every(horizontal => [-1, 1].every(vertical => {
    const corner = new Vector3(horizontal * width / 2, vertical * height / 2, 0).applyMatrix4(plane);
    const point = projectHeadPoint(eye, corner);
    return point !== null && Math.abs(point.x) <= 1 - margin && Math.abs(point.y) <= 1 - margin &&
      point.z >= (eye.depthRange === "negative-one-to-one" ? -1 : 0) && point.z <= 1;
  })));
}
