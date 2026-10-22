import { afterEach, describe, expect, it, vi } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { XrSpatialBridge, XrCameraBridge, XrPointerBridge } from "../../web/pkg/birdman_game_wasm.js";
import { XrPointerSession } from "../../web/src/presentation/xr-pointer-session.js";
import { MenuSession } from "../../web/src/presentation/menu-session.js";
import { menuViewport, projectMenuViewport } from "../../web/src/presentation/menu-layout.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";
import { initializeWasmFixture } from "../game-check/wasm-fixture.js";
import { XrSpatialSession } from "../../web/src/presentation/xr-spatial-session.js";
import { XrCameraOrigin, projectXrReferenceFrame } from "../../web/src/render/camera/spatial.js";
import { composePose, IDENTITY_POSE, inversePose, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { headPlaneFitsViews } from "./viewer-fixture.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { createSceneFixture } from "./ui-adapter-fixture.js";
import { geometryMenuFixture, referenceFrameFixture } from "./menu-fixture.js";

initializeWasmFixture();
afterEach(() => { vi.restoreAllMocks(); });

describe("shared pointer adapter", () => {
  const panelPose = pose(vec3(0, 0, -2), IDENTITY_POSE.orientation);
  const ray = { source: { kind: "controller" as const, id: 5 }, origin: vec3(0.3, 0.3, 0), direction: vec3(0, 0, -1), timestampMs: 100 };

  it("retains the gamepad cursor and held-button history across actual scroll generations", () => {
    const view = createSceneFixture("Title");
    const original = view.panels[0];
    if (original === undefined) throw new Error("Missing panel");
    const panel = { ...original, size: { width: 2, height: 1 }, controls: [
      { kind: "button" as const, id: "start", label: "Start", enabled: true, rect: { x: 0, y: 0, width: 1, height: 1 } }
    ] };
    const measured = geometryMenuFixture({ ...view, panels: [panel] });
    if (measured.kind !== "ready") throw new Error("Missing menu");
    let model = createInitialAppModel();
    const scrollOwner = new MenuSession();
    model = updateApp(model, { type: "menu-owner-observed", state: scrollOwner.update({ type: "menu-scroll-synchronized", generation: 0,
      target: { kind: "active", scope: measured.context.scope } }) }).model;
    const actions: UiAction[] = [];
    const owner = new XrPointerSession(action => {
      actions.push(action);
      if (action.type === "menu-scroll") model = updateApp(model, { type: "menu-owner-observed", state: scrollOwner.update(action) }).model;
    });
    const frame = (timestampMs: number, pressed: boolean, horizontal: number) => {
      if (model.menuScroll.kind !== "active") throw new Error("Missing active scroll context");
      const viewport = menuViewport({ ...measured.viewport.document, contentHeightMeters: 1.8, footerHeightMeters: 0.2 }, model.menuScroll.progress);
      expect(viewport.maximumOffsetMeters).toBe(1);
      expect(viewport.pageProgress).toBe(0.8);
      expect(viewport.offsetMeters).toBe(model.menuScroll.progress);
      const menu = { ...measured, context: { scope: model.menuScroll.scope, generation: model.menuScroll.generation }, viewport };
      return owner.observe("title", panel, panelPose, menu, null, [], true, timestampMs,
        { index: 0, generation: 1, axes: [horizontal, 0, 0, 1], buttons: [pressed, false] });
    };
    try {
      frame(0, false, 0);
      const moved = frame(50, true, 1);
      expect(moved.cursor?.point.x).toBeCloseTo(0.12);
      expect(model.menuScroll.generation).toBe(2);
      expect(frame(100, true, 0).cursor?.point.x).toBeCloseTo(0.12);
      expect(model.menuScroll.generation).toBe(3);
      expect(actions.filter(action => action.type === "menu-control")).toHaveLength(1);
    } finally { owner.reset(); scrollOwner.dispose(); }
  });

  it("commits held gamepad observations before a changed-layout focus callback throws", () => {
    const view = createSceneFixture("Title");
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const measured = geometryMenuFixture(view);
    if (measured.kind !== "ready") throw new Error("Missing menu");
    const dispatch = vi.fn<(action: UiAction) => void>();
    const owner = new XrPointerSession(dispatch);
    const sample = (pressed: boolean) => ({ index: 0, generation: 1, axes: [0, 0, 0, 0] as const, buttons: [pressed, false] as const });
    try {
      owner.observe("title", panel, panelPose, measured, null, [], true, 0, sample(false));
      const changed = { ...measured, context: { ...measured.context, generation: measured.context.generation + 1 } };
      dispatch.mockImplementationOnce(() => { throw new Error("focus observer failed"); });
      expect(() => owner.observe("title", panel, panelPose, changed, null, [], true, 50, sample(true))).toThrow("focus observer failed");
      dispatch.mockClear();
      owner.observe("title", panel, panelPose, changed, null, [], true, 100, sample(true));
      expect(dispatch.mock.calls.some(([action]) => action.type === "menu-control")).toBe(false);
    } finally { owner.reset(); }
  });

  it("uses clipped measured menu regions for range selection and ignores disabled controls", () => {
    const fixture = createSceneFixture("Title");
    const original = fixture.panels[0];
    if (original === undefined) throw new Error("Missing panel");
    const panel = { ...original, size: { width: 2, height: 1 }, controls: [
      { kind: "button" as const, id: "disabled", label: "Disabled", enabled: false, rect: { x: 0, y: 0, width: 1, height: 1 } },
      { kind: "range" as const, id: "range", label: "Range", enabled: true, rect: { x: 0.1, y: 0.2, width: 0.8, height: 0.4 },
        minimum: 0, maximum: 10, step: 1, value: 0, displayValue: "0" }
    ] };
    const measured = geometryMenuFixture({ ...fixture, panels: [panel] });
    if (measured.kind !== "ready") throw new Error("Missing menu");
    const viewport = menuViewport({ ...measured.viewport.document, contentHeightMeters: 1.8, footerHeightMeters: 0.2 }, 0.3);
    const range = viewport.document.controls.find(layout => layout.control.id === "range");
    if (range === undefined) throw new Error("Missing range");
    const geometry = projectMenuViewport(viewport, [range.bounds]).rectangles[0];
    expect(geometry?.full.y).toBeCloseTo(-0.1);
    expect(geometry?.visible?.height).toBeCloseTo(0.3);
    const menu = { ...measured, viewport };
    const actions: UiAction[] = [];
    const owner = new XrPointerSession((action) => actions.push(action));
    try {
      const cursors = owner.observe("title", panel, panelPose, menu, null, [ray], true, 100);
      expect(cursors.cursor?.point).toEqual({ x: 0.3, y: 0.3 });
      owner.select({ ...ray, timestampMs: 101 });
      expect(actions).toContainEqual({ type: "menu-control", context: menu.context,
        action: { type: "set-range", controlId: "range", value: 7 } });
      expect(actions.some((action) => action.type === "activate" && action.controlId === "disabled")).toBe(false);
    } finally { owner.reset(); }
  });

  it("frees the Rust owner once even when focus cleanup throws", () => {
    const panel = createSceneFixture("Flight").panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const release = vi.spyOn(XrPointerBridge.prototype, "free");
    const owner = new XrPointerSession((action) => { if (action.type === "focus" && action.controlId === null) throw new Error("focus cleanup"); });
    owner.observe("flight", panel, panelPose, { kind: "absent" }, null, [ray], true, 100);
    expect(() => { owner.reset(); }).toThrow("focus cleanup");
    owner.reset();
    expect(release).toHaveBeenCalledTimes(1);
  });

  it("retires activation when a focus callback synchronously resets its owner", () => {
    const original = createSceneFixture("Flight").panels[0];
    if (original === undefined) throw new Error("Missing panel");
    const panel = { ...original, controls: [{ kind: "button" as const, id: "start", label: "Start", enabled: true,
      rect: { x: 0, y: 0, width: 1, height: 1 } }] };
    const release = vi.spyOn(XrPointerBridge.prototype, "free");
    const actions: UiAction[] = [];
    const owner = new XrPointerSession((action) => {
      actions.push(action);
      if (action.type === "focus" && action.controlId !== null) owner.reset();
    });
    expect(owner.observe("flight", panel, panelPose, { kind: "absent" }, null, [ray], true, 100).cursor).toBeNull();
    owner.select({ ...ray, timestampMs: 200 });
    owner.reset();
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    expect(release).toHaveBeenCalledTimes(1);
  });
});

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
