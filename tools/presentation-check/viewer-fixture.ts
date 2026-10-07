import { PerspectiveCamera, StereoCamera } from "three";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, PanelFrame } from "../../web/src/render/contracts/runtime.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";

export function configuredViewerFixture(width = 1280, height = 720, near = 0.1, far = 100): ViewerFrame {
  const camera = new PerspectiveCamera(60, width / height, near, far);
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
