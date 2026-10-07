import { describe, expect, it } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { composePose, IDENTITY_POSE, pose, quaternion, transformPoint, vec3 } from "../../web/src/render/contracts/math.js";
import { headPlaneFitsViews, projectHeadPoint, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { fitMenuView } from "../../web/src/presentation/menu-view-fit.js";

const panelSize = Object.freeze({ width: 2.4, height: 1.8 });

describe("Same-frame binocular Menu fit", () => {
  it.each([[1280, 720], [720, 1280], [3840, 2160], [320, 640]])("fits the entire unchanged panel in both eyes at %i × %i", (width, height) => {
    const viewer = configuredViewer(width / height);
    const fit = fitMenuView(viewer, panelSize);
    expect(fit.kind).toBe("ready");
    if (fit.kind !== "ready") throw new Error("Missing Menu fit");
    expect(headPlaneFitsViews(viewer, fit.headFromMenu, 2.4, 1.8, 0.035)).toBe(true);
    expect(fit.distanceMeters).toBeGreaterThanOrEqual(2.4);
    expect(Object.isFrozen(fit)).toBe(true);
    expect(Object.isFrozen(fit.headFromMenu)).toBe(true);
    expect(panelSize).toEqual({ width: 2.4, height: 1.8 });
    if (viewer.source === "unavailable") throw new Error("Missing binocular geometry");
    for (const eye of viewer.eyes) for (const horizontal of [-1, 1]) for (const vertical of [-1, 1]) {
      const projected = projectHeadPoint(eye, transformPoint(fit.headFromMenu, vec3(horizontal * 1.2, vertical * 0.9, 0)));
      expect(projected).not.toBeNull();
      expect(Math.abs(projected?.x ?? Number.NaN)).toBeLessThanOrEqual(0.965);
      expect(Math.abs(projected?.y ?? Number.NaN)).toBeLessThanOrEqual(0.965);
    }
  });

  it("rejects the retained Title anchor seen from the real Flight eye mount and fits Pause independently", () => {
    const viewer = configuredViewer(1280 / 720);
    const oldHeadFromMenu = pose(vec3(0, -0.15, -1.85), IDENTITY_POSE.orientation);
    expect(headPlaneFitsViews(viewer, oldHeadFromMenu, 2.4, 1.8)).toBe(false);
    const fit = fitMenuView(viewer, panelSize);
    if (fit.kind !== "ready") throw new Error("Missing Pause fit");
    for (const longitudinalOffset of [-0.4, 0, 0.4]) {
      const mountedHead = pose(vec3(0, 0.15, -0.55 + longitudinalOffset), quaternion(Math.cos(0.04), Math.sin(0.04), 0, 0));
      const referenceFromMenu = composePose(mountedHead, fit.headFromMenu);
      expect(referenceFromMenu.position).not.toEqual({ x: 0, y: 0, z: -2.4 });
      expect(headPlaneFitsViews(viewer, fit.headFromMenu, 2.4, 1.8, 0.035)).toBe(true);
    }
  });

  it("uses both asymmetric and rotated eye projections including IPD exactly once", () => {
    const original = configuredViewer(1280 / 720);
    if (original.source === "unavailable") throw new Error("Missing binocular geometry");
    const leftProjection = [...original.eyes[0].projection];
    const rightProjection = [...original.eyes[1].projection];
    leftProjection[8] = 0.2;
    rightProjection[8] = -0.12;
    rightProjection[0] = (rightProjection[0] ?? 0) * 1.3;
    const viewer: ViewerFrame = { ...original, eyes: [
      { ...original.eyes[0], headFromEye: pose(vec3(-0.04, 0.006, 0.01), quaternion(Math.cos(0.02), 0, Math.sin(0.02), 0)), projection: leftProjection as unknown as typeof original.eyes[0]["projection"] },
      { ...original.eyes[1], projection: rightProjection as unknown as typeof original.eyes[1]["projection"] }
    ] };
    const fit = fitMenuView(viewer, panelSize);
    if (fit.kind !== "ready") throw new Error("Missing asymmetric Menu fit");
    expect(headPlaneFitsViews(viewer, fit.headFromMenu, 2.4, 1.8, 0.035)).toBe(true);
    const symmetric = fitMenuView(original, panelSize);
    if (symmetric.kind !== "ready") throw new Error("Missing symmetric fit");
    expect(fit.distanceMeters).toBeGreaterThan(symmetric.distanceMeters);
  });

  it("preserves local panel offsets and finite depth clipping", () => {
    const viewer = configuredViewer(1280 / 720);
    const offset = pose(vec3(0, -1.12, 0), IDENTITY_POSE.orientation);
    const fit = fitMenuView(viewer, { width: 0.62, height: 0.25 }, offset);
    if (fit.kind !== "ready") throw new Error("Missing offset fit");
    expect(headPlaneFitsViews(viewer, composePose(fit.headFromMenu, offset), 0.62, 0.25, 0.035)).toBe(true);
    expect(fitMenuView(configuredViewer(0.2, 0.1, 3), panelSize)).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    const beyondNear = fitMenuView(configuredViewer(2, 5, 100), panelSize);
    if (beyondNear.kind !== "ready") throw new Error("Missing near-clipped Menu fit");
    expect(beyondNear.distanceMeters).toBeGreaterThanOrEqual(5);
  });

  it("distinguishes unavailable and invalid geometry from invalid layout parameters", () => {
    expect(fitMenuView(unavailableViewerFrame("viewer-unavailable"), panelSize)).toEqual({ kind: "unavailable", reason: "viewer-unavailable" });
    const viewer = configuredViewer(1280 / 720);
    if (viewer.source === "unavailable") throw new Error("Missing binocular geometry");
    const invalid: ViewerFrame = { ...viewer, eyes: [{ ...viewer.eyes[0], projection: Array.from({ length: 16 }, () => Number.NaN) as unknown as typeof viewer.eyes[0]["projection"] }, viewer.eyes[1]] };
    expect(fitMenuView(invalid, panelSize)).toEqual({ kind: "unavailable", reason: "invalid-view-geometry" });
    const overflowing: ViewerFrame = { ...viewer, eyes: [
      { ...viewer.eyes[0], headFromEye: pose(vec3(-Number.MAX_VALUE, 0, 0), IDENTITY_POSE.orientation) }, viewer.eyes[1]
    ] };
    expect(fitMenuView(overflowing, panelSize, pose(vec3(Number.MAX_VALUE, 0, 0), IDENTITY_POSE.orientation)))
      .toEqual({ kind: "unavailable", reason: "invalid-view-geometry" });
    expect(() => fitMenuView(viewer, { width: 0, height: 1 })).toThrow(RangeError);
    expect(() => fitMenuView(viewer, panelSize, IDENTITY_POSE, 0)).toThrow(RangeError);
    expect(() => fitMenuView(viewer, panelSize, IDENTITY_POSE, 2.4, 1)).toThrow(RangeError);
  });
});

function configuredViewer(aspect: number, near = 0.1, far = 100): ViewerFrame {
  const camera = new PerspectiveCamera(60, aspect, near, far);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  return captureConfiguredViewerFrame(camera, stereo);
}
