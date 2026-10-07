import { describe, expect, it } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { composePose, IDENTITY_POSE, inversePose, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import { headPlaneFitsViews, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { CLOSED_MENU_PLACEMENT, openMenuPlacement, recenterMenuPlacement, transformMenuPlacement } from "../../web/src/presentation/menu-placement.js";

describe("Pure Menu opening lifecycle", () => {
  it("retains the fixed opening pose during head motion and telemetry changes", () => {
    const view = createSceneFixture("Title");
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing Title panel");
    const viewer = configuredViewer();
    const initial = openMenuPlacement(CLOSED_MENU_PLACEMENT, view, panel, IDENTITY_POSE, viewer);
    if (initial.result.kind !== "ready") throw new Error("Missing initial Menu placement");
    const turned = pose(vec3(1, 0.5, -0.7), quaternion(Math.cos(0.3), 0, Math.sin(0.3), 0));
    const subsequent = openMenuPlacement(initial.model, view, { ...panel, title: "Changed text" }, turned, viewer);
    expect(subsequent.model).toBe(initial.model);
    expect(subsequent.result).toEqual(initial.result);
    expect(CLOSED_MENU_PLACEMENT).toEqual({ kind: "closed" });
  });

  it("opens Pause and each overlay from the mounted current head rather than the retained Title pose", () => {
    const title = createSceneFixture("Title");
    const titlePanel = title.panels[0];
    if (titlePanel === undefined) throw new Error("Missing Title panel");
    const viewer = configuredViewer();
    const initial = openMenuPlacement(CLOSED_MENU_PLACEMENT, title, titlePanel, IDENTITY_POSE, viewer);
    for (const longitudinalOffset of [-0.4, 0, 0.4]) {
      const head = pose(vec3(0, 0.15, -0.55 + longitudinalOffset), quaternion(Math.cos(0.04), Math.sin(0.04), 0, 0));
      const paused = openMenuPlacement(initial.model, { scene: "Flight", activeOverlay: "Pause" }, titlePanel, head, viewer);
      if (paused.result.kind !== "ready") throw new Error("Missing Pause placement");
      expect(paused.model).not.toBe(initial.model);
      const headFromMenu = composePose(inversePose(head), paused.result.referenceFromMenu);
      expect(headPlaneFitsViews(viewer, headFromMenu, titlePanel.size.width, titlePanel.size.height, 0.035)).toBe(true);
      const settings = openMenuPlacement(paused.model, { scene: "Flight", activeOverlay: "PauseSettings" }, titlePanel, IDENTITY_POSE, viewer);
      expect(settings.model).not.toBe(paused.model);
    }
  });

  it("refits real geometry changes while ignoring rigid-transform floating-point noise", () => {
    const view = createSceneFixture("Title");
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const viewer = configuredViewer();
    if (viewer.source === "unavailable") throw new Error("Missing viewer");
    const initial = openMenuPlacement(CLOSED_MENU_PLACEMENT, view, panel, IDENTITY_POSE, viewer);
    const noise = { ...viewer, eyes: [
      { ...viewer.eyes[0], headFromEye: pose(vec3(viewer.eyes[0].headFromEye.position.x + 1e-12, 0, 0), viewer.eyes[0].headFromEye.orientation) }, viewer.eyes[1]
    ] as const };
    expect(openMenuPlacement(initial.model, view, panel, pose(vec3(10, 0, 0), IDENTITY_POSE.orientation), noise).model).toBe(initial.model);
    const equivalentQuaternion = { ...viewer, eyes: [
      { ...viewer.eyes[0], headFromEye: pose(viewer.eyes[0].headFromEye.position, quaternion(-1, 0, 0, 0)) }, viewer.eyes[1]
    ] as const };
    expect(openMenuPlacement(initial.model, view, panel, IDENTITY_POSE, equivalentQuaternion).model).toBe(initial.model);
    const rotatedViewport = openMenuPlacement(initial.model, view, panel, IDENTITY_POSE, configuredViewer(720 / 1280));
    expect(rotatedViewport.model).not.toBe(initial.model);
    if (rotatedViewport.model.kind !== "placed" || initial.model.kind !== "placed") throw new Error("Missing placements");
    expect(rotatedViewport.model.referenceFromMenu.position.z).toBeLessThan(initial.model.referenceFromMenu.position.z);
  });

  it("retains only placement data during unavailable frames and refits a changed pending overlay", () => {
    const view = createSceneFixture("Title");
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const initial = openMenuPlacement(CLOSED_MENU_PLACEMENT, view, panel, IDENTITY_POSE, configuredViewer());
    const waiting = openMenuPlacement(initial.model, { scene: "Flight", activeOverlay: "Pause" }, panel,
      IDENTITY_POSE, unavailableViewerFrame("viewer-unavailable"));
    expect(waiting.model).toBe(initial.model);
    expect(waiting.result).toEqual({ kind: "unavailable", reason: "viewer-unavailable" });
    const next = openMenuPlacement(waiting.model, { scene: "Flight", activeOverlay: "Pause" }, panel,
      pose(vec3(0, 0.15, -0.55), IDENTITY_POSE.orientation), configuredViewer());
    expect(next.model).not.toBe(initial.model);
    expect(next.result.kind).toBe("ready");
  });

  it("preserves fitted distance across explicit Menu recenter and reference reset", () => {
    const view = createSceneFixture("Title");
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const initial = openMenuPlacement(CLOSED_MENU_PLACEMENT, view, panel, IDENTITY_POSE, configuredViewer());
    const newHead = pose(vec3(2, 3, 4), quaternion(Math.cos(0.2), 0, Math.sin(0.2), 0));
    const recentered = recenterMenuPlacement(initial.model, newHead);
    if (initial.model.kind !== "placed" || recentered.kind !== "placed") throw new Error("Missing placement");
    expect(recentered.referenceFromMenu).toEqual(composePose(newHead, initial.model.opening.headFromMenu));
    const reference = pose(vec3(-1, 5, 2), quaternion(Math.cos(0.15), Math.sin(0.15), 0, 0));
    const transformed = transformMenuPlacement(recentered, reference);
    if (transformed.kind !== "placed") throw new Error("Missing transformed placement");
    expect(transformed.referenceFromMenu).toEqual(composePose(reference, recentered.referenceFromMenu));
    expect(transformed.opening).toBe(recentered.opening);
    expect(recenterMenuPlacement(CLOSED_MENU_PLACEMENT, newHead)).toBe(CLOSED_MENU_PLACEMENT);
    expect(transformMenuPlacement(CLOSED_MENU_PLACEMENT, reference)).toBe(CLOSED_MENU_PLACEMENT);
  });
});

function configuredViewer(aspect = 1280 / 720) {
  const camera = new PerspectiveCamera(60, aspect, 0.1, 100);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  return captureConfiguredViewerFrame(camera, stereo);
}
