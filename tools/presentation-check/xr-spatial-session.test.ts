import { afterEach, describe, expect, it, vi } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { XrSpatialBridge, XrCameraBridge } from "../../web/pkg/birdman_game_wasm.js";
import { initializeWasmFixture } from "../game-check/wasm-fixture.js";
import { XrSpatialSession } from "../../web/src/presentation/xr-spatial-session.js";
import { XrCameraOrigin, projectXrReferenceFrame } from "../../web/src/render/camera/xr-spatial.js";
import { composePose, IDENTITY_POSE, inversePose, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import { headPlaneFitsViews, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { referenceFrameFixture } from "./menu-fixture.js";

initializeWasmFixture();
afterEach(() => { vi.restoreAllMocks(); });

describe("shared spatial session", () => {
  it("retains the opening during head motion and refits scene, credits and optical changes", () => {
    const spatial = new XrSpatialSession();
    try {
      const view = createSceneFixture("Title");
      const panel = view.panels[0];
      if (panel === undefined) throw new Error("Missing panel");
      const viewer = configuredViewer();
      const initial = spatial.place(view, panel, viewer, referenceFrameFixture(IDENTITY_POSE));
      const turned = pose(vec3(1, 0.5, -0.7), quaternion(Math.cos(0.3), 0, Math.sin(0.3), 0));
      expect(spatial.place(view, { ...panel, title: "Changed text" }, viewer, referenceFrameFixture(turned))).toEqual(initial);
      const credits = spatial.place({ ...view, activeOverlay: "Credits" }, panel, viewer, referenceFrameFixture(turned));
      expect(credits).not.toEqual(initial);
      const paused = spatial.place({ scene: "Flight", activeOverlay: "Pause" }, panel, viewer, referenceFrameFixture(turned));
      expect(headPlaneFitsViews(viewer, composePose(inversePose(turned), panelPose(paused)), panel.size.width, panel.size.height, 0.035)).toBe(true);
      const portrait = spatial.place({ scene: "Flight", activeOverlay: "Pause" }, panel, configuredViewer(720 / 1280), referenceFrameFixture(turned));
      expect(portrait).not.toEqual(paused);
    } finally { spatial.reset(); }
  });

  it("retains an opening through float32-scale optical noise and refits meaningful changes", () => {
    const spatial = new XrSpatialSession();
    try {
      const view = createSceneFixture("Title");
      const panel = view.panels[0] ?? null;
      const viewer = configuredViewer();
      if (viewer.source === "unavailable") throw new Error("Missing views");
      const reference = referenceFrameFixture(IDENTITY_POSE);
      const initial = spatial.place(view, panel, viewer, reference);
      const moved = referenceFrameFixture(pose(vec3(10, 0, 0), IDENTITY_POSE.orientation));
      for (const noise of [-1e-5, -1e-7, 0, 1e-7, 1e-5]) {
        const projection = viewer.eyes[0].projection.map((value, index) => Math.fround(value + (index === 0 ? noise : 0))) as unknown as (typeof viewer.eyes)[0]["projection"];
        const noisy = { ...viewer, eyes: [{ ...viewer.eyes[0], projection,
          headFromEye: pose(vec3(Math.fround(viewer.eyes[0].headFromEye.position.x + noise), noise, 0), quaternion(-Math.cos(noise / 2), 0, -Math.sin(noise / 2), 0)) }, viewer.eyes[1]] as const };
        expect(spatial.place(view, panel, noisy, moved)).toEqual(initial);
      }
      expect(spatial.place(view, panel, unavailableViewerFrame("viewer-unavailable"), null)).toEqual({ kind: "unavailable", reason: "viewer-unavailable" });
      expect(spatial.place(view, panel, viewer, moved)).toEqual(initial);
      const changedIpd = { ...viewer, eyes: [{ ...viewer.eyes[0], headFromEye: pose(vec3(viewer.eyes[0].headFromEye.position.x - 0.002, 0, 0), IDENTITY_POSE.orientation) }, viewer.eyes[1]] as const };
      expect(spatial.place(view, panel, changedIpd, moved)).not.toEqual(initial);
      expect(spatial.place(view, panel, viewer, { ...moved, basis: "aircraft" })).not.toEqual(initial);
    } finally { spatial.reset(); }
  });

  it("separates explicit tracking recenter, menu recenter and physical anchors", () => {
    const spatial = new XrSpatialSession();
    try {
      const view = createSceneFixture("Title");
      const panel = view.panels[0];
      if (panel === undefined) throw new Error("Missing panel");
      const viewer = configuredViewer();
      const reference = referenceFrameFixture(IDENTITY_POSE);
      const initial = panelPose(spatial.place(view, panel, viewer, reference));
      const change = pose(vec3(-1, 5, 2), quaternion(Math.cos(0.15), Math.sin(0.15), 0, 0));
      spatial.recenterTracking(change);
      expectPose(panelPose(spatial.place(view, panel, viewer, reference)), composePose(change, initial));
      expectPose(panelPose(spatial.place(view, { ...panel, anchor: "world" }, viewer, reference)), panel.localPose);
      const opened = panelPose(spatial.place(view, panel, viewer, reference));
      const head = pose(vec3(2, 3, 4), quaternion(Math.cos(0.2), 0, Math.sin(0.2), 0));
      spatial.recenterMenu(head);
      expectPose(panelPose(spatial.place(view, panel, viewer, referenceFrameFixture(head))), composePose(head, opened));
      spatial.closeMenu();
      expect(spatial.place(view, null, viewer, reference)).toEqual({ kind: "absent" });
    } finally { spatial.reset(); }
  });

  it("rejects a retained menu when a sub-tolerance depth change moves its far plane through the panel", () => {
    const spatial = new XrSpatialSession();
    try {
      const view = createSceneFixture("Title");
      const panel = view.panels[0] ?? null;
      const reference = referenceFrameFixture(IDENTITY_POSE);
      const initial = spatial.place(view, panel, configuredViewer(), reference);
      const distance = -panelPose(initial).position.z;
      const before = configuredViewer(1280 / 720, distance + 0.0001);
      const after = configuredViewer(1280 / 720, distance - 0.0001);
      if (before.source === "unavailable" || after.source === "unavailable") throw new Error("Missing clip geometry");
      for (const [index, value] of before.eyes[0].projection.entries()) expect(Math.abs(value - (after.eyes[0].projection[index] ?? Number.NaN))).toBeLessThan(1e-4);
      expect(spatial.place(view, panel, before, reference).kind).toBe("ready");
      expect(spatial.place(view, panel, after, reference).kind).toBe("unavailable");
    } finally { spatial.reset(); }
  });

  it("preserves both-eye world poses and the retained menu across passive runtime reset", () => {
    const origin = new XrCameraOrigin();
    const spatial = new XrSpatialSession();
    try {
      const aircraft = pose(vec3(10, 20, -30), quaternion(Math.cos(0.1), Math.sin(0.1), 0, 0));
      const mount = { kind: "external" as const, worldFromTracking: pose(vec3(1, 3, 5), quaternion(Math.cos(0.2), 0, Math.sin(0.2), 0)) };
      const rawHead = pose(vec3(0.1, 0.2, 0.3), IDENTITY_POSE.orientation);
      const before = origin.rig(aircraft, mount);
      const view = createSceneFixture("Title");
      const viewer = configuredViewer();
      const panel = view.panels[0] ?? null;
      const opening = spatial.place(view, panel, viewer, projectXrReferenceFrame(before, rawHead));
      const reset = pose(vec3(-1, 0.2, 0.1), quaternion(Math.cos(0.3), 0, Math.sin(0.3), 0));
      origin.resetReference(reset);
      const after = origin.rig(aircraft, mount);
      const nextHead = composePose(inversePose(reset), rawHead);
      for (const eye of viewer.source === "unavailable" ? [] : viewer.eyes) {
        expectPose(composePose(composePose(after.worldFromTracking, nextHead), eye.headFromEye), composePose(composePose(before.worldFromTracking, rawHead), eye.headFromEye));
      }
      expect(spatial.place(view, panel, viewer, projectXrReferenceFrame(after, nextHead))).toEqual(opening);
    } finally { spatial.reset(); origin.dispose(); }
  });

  it("releases each Rust owner exactly once and starts subsequent sessions from a fresh origin", () => {
    const spatial = new XrSpatialSession();
    const camera = new XrCameraOrigin();
    const freeMenu = vi.spyOn(XrSpatialBridge.prototype, "free");
    const freeCamera = vi.spyOn(XrCameraBridge.prototype, "free");
    const view = createSceneFixture("Title");
    spatial.place(view, view.panels[0] ?? null, configuredViewer(), referenceFrameFixture(IDENTITY_POSE));
    camera.resetReference(pose(vec3(2, 3, 4), IDENTITY_POSE.orientation));
    spatial.reset(); spatial.reset(); camera.dispose(); camera.dispose();
    expect(freeMenu).toHaveBeenCalledTimes(1);
    expect(freeCamera).toHaveBeenCalledTimes(1);
    expect(camera.rig(IDENTITY_POSE, { kind: "aircraft", aircraftFromTracking: IDENTITY_POSE }).worldFromTracking).toEqual(IDENTITY_POSE);
    camera.dispose();
    expect(freeCamera).toHaveBeenCalledTimes(2);
  });
});

function configuredViewer(aspect = 1280 / 720, far = 100) {
  const camera = new PerspectiveCamera(60, aspect, 0.1, far);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  return captureConfiguredViewerFrame(camera, stereo);
}

function panelPose(result: ReturnType<XrSpatialSession["place"]>): Pose {
  if (result.kind !== "ready") throw new Error(`Missing placement: ${result.kind}`);
  return result.referenceFromPanel;
}

function expectPose(actual: Pose, expected: Pose): void {
  expect(actual.position.x).toBeCloseTo(expected.position.x, 12);
  expect(actual.position.y).toBeCloseTo(expected.position.y, 12);
  expect(actual.position.z).toBeCloseTo(expected.position.z, 12);
  const first = actual.orientation;
  const second = expected.orientation;
  expect(Math.abs(first.w * second.w + first.x * second.x + first.y * second.y + first.z * second.z)).toBeCloseTo(1, 12);
}
