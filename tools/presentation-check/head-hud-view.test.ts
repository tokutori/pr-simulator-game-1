import { describe, expect, it } from "vitest";
import { Euler, Matrix4, PerspectiveCamera, Quaternion, StereoCamera, Vector3 } from "three";
import { createInitialAppModel, gameSessionState } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { createFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { createHeadHudView, DEFAULT_HEAD_HUD_PROFILE } from "../../web/src/presentation/head-hud-view.js";
import { validateHeadHudLayer } from "../../web/src/render/contracts/head-hud.js";
import { IDENTITY_POSE, pose, quaternion, transformPoint, vec3 } from "../../web/src/render/contracts/math.js";
import { copyProjectionMatrix, projectHeadPoint, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { validateUiViewModel, viewExposesAction } from "../../web/src/render/contracts/ui.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { convexQuadsOverlap } from "./hud-canvas-fixture.js";

const profile = Object.freeze({ telemetry: true, attitude: true, wind: true, flightPath: true, angleOfAttack: true, warnings: true });
const values = new Array<number>(33).fill(0);
values[4] = 8;
values[6] = -1;
values[7] = 1;
values[11] = 0.2;
values[19] = -1;
values[20] = 12;
values[21] = 8;
values[22] = 9;
values[23] = 2;
values[24] = -1;
values[26] = 0.1;
values[28] = 0.2;
values[29] = -0.15;
values[30] = 0.3;
values[31] = 1;
const snapshot = parseFlightSnapshot(values);

function configuredViewer(aspect = 1280 / 720, fieldOfView = 60, near = 0.05): ViewerFrame {
  const camera = new PerspectiveCamera(fieldOfView, aspect, near, 100);
  camera.updateMatrixWorld(true);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  return captureConfiguredViewerFrame(camera, stereo);
}

function sessionForPhase(phase: number) {
  const session = gameSessionState(phase, 0, phase === 5 || phase === 6 ? snapshot : null, true);
  if (session === null) throw new Error("Invalid session fixture");
  return session;
}

describe("Pure Head Flight HUD layout", () => {
  it.each([
    { code: 0, kinds: ["attitude", "heading", "pilot-position", "wind", "angle-of-attack"] },
    { code: 1, kinds: ["attitude", "heading"] },
    { code: 2, kinds: [] },
    { code: 3, kinds: ["attitude", "heading"] },
    { code: 4, kinds: ["attitude", "heading", "pilot-position", "wind", "angle-of-attack"] }
  ] as const)("uses the shared Information model for level $code", ({ code, kinds }) => {
    const model = createFlightHudModel(snapshot, code, profile);
    const before = JSON.stringify(model);
    const view = createHeadHudView(model, configuredViewer());
    if (view.kind !== "visible") throw new Error("Expected visible Flight HUD");
    expect(view.layer.elements.filter((element) => element.kind !== "text").map((element) => element.kind)).toEqual(kinds);
    expect(view.layer.elements.find((element) => element.kind === "attitude")).toMatchObject(model.attitude ?? {});
    expect(JSON.stringify(model)).toBe(before);
    expect(Object.isFrozen(view.layer.elements)).toBe(true);
    expect(() => { validateHeadHudLayer(view.layer); }).not.toThrow();
    if (code === 3) {
      const readouts = view.layer.elements.find((element) => element.kind === "text" && element.id === "head-readouts");
      expect(readouts?.kind === "text" && readouts.value.includes("対象機の実機構成は未確認")).toBe(true);
    }
  });

  it.each(Array.from({ length: 64 }, (_, mask) => ({ mask })))("keeps all Custom cues independent for mask $mask", ({ mask }) => {
    const custom = {
      telemetry: (mask & 1) !== 0, attitude: (mask & 2) !== 0, wind: (mask & 4) !== 0,
      flightPath: (mask & 8) !== 0, angleOfAttack: (mask & 16) !== 0, warnings: (mask & 32) !== 0
    };
    const failed = parseFlightSnapshot(values.map((value, index) => index === 16 ? 5 : value));
    const model = createFlightHudModel(failed, 4, custom);
    const view = createHeadHudView(model, configuredViewer());
    if (mask === 0) {
      expect(view).toEqual({ kind: "absent" });
      return;
    }
    if (view.kind !== "visible") throw new Error("Expected visible Custom HUD");
    const byId = new Map(view.layer.elements.map((element) => [element.id, element]));
    expect(byId.has("head-attitude")).toBe(custom.attitude);
    expect(byId.has("head-heading")).toBe(custom.attitude);
    expect(byId.has("head-pilot")).toBe(custom.telemetry);
    expect(byId.has("head-wind")).toBe(custom.wind);
    expect(byId.has("head-aoa")).toBe(custom.angleOfAttack);
    expect(byId.has("head-warning")).toBe(custom.warnings);
    expect(byId.has("head-flight-path")).toBe(custom.flightPath && !custom.attitude);
    if (custom.attitude) expect(byId.get("head-attitude")).toMatchObject({ flightPathAngleDegrees: custom.flightPath ? model.flightPathAngleDegrees : null });
  });

  it("preserves unavailable quantities rather than inventing instrument values", () => {
    const noTelemetry = { ...snapshot, telemetry: null };
    const view = createHeadHudView(createFlightHudModel(noTelemetry, 0), configuredViewer());
    if (view.kind !== "visible") throw new Error("Missing unavailable-data HUD");
    expect(view.layer.elements.find((element) => element.id === "head-wind")).toMatchObject({ kind: "text", value: "unavailable" });
    expect(view.layer.elements.find((element) => element.id === "head-aoa")).toMatchObject({ kind: "text", value: "unavailable" });
    expect(view.layer.elements.some((element) => element.kind === "attitude" || element.kind === "heading")).toBe(false);
  });

  it("fits asymmetric canted sheared views and keeps each card inside both eye clips", () => {
    const leftProjection = new Matrix4().makePerspective(-0.09, 0.1, 0.08, -0.075, 0.1, 100);
    leftProjection.elements[4] = 0.06;
    const rightProjection = new Matrix4().makePerspective(-0.1, 0.08, 0.085, -0.08, 0.1, 100);
    const copiedLeft = copyProjectionMatrix(leftProjection.elements);
    const copiedRight = copyProjectionMatrix(rightProjection.elements);
    if (copiedLeft === null || copiedRight === null) throw new Error("Invalid fixture projection");
    const leftRotation = new Quaternion().setFromEuler(new Euler(0.08, 0.05, 0.07));
    const rightRotation = new Quaternion().setFromEuler(new Euler(-0.07, -0.04, -0.08));
    const viewer: ViewerFrame = { source: "runtime-derived", trackingFromHead: IDENTITY_POSE, eyes: [
      { eye: "left", headFromEye: pose(vec3(-0.034, 0.003, 0.002), quaternion(leftRotation.w, leftRotation.x, leftRotation.y, leftRotation.z)), projection: copiedLeft },
      { eye: "right", headFromEye: pose(vec3(0.032, -0.003, 0.004), quaternion(rightRotation.w, rightRotation.x, rightRotation.y, rightRotation.z)), projection: copiedRight }
    ] };
    const model = createFlightHudModel(snapshot, 0);
    const view = createHeadHudView(model, viewer);
    if (view.kind !== "visible") throw new Error("Expected fitted native-style HUD");
    const corners = [[-1, 1], [1, 1], [1, -1], [-1, -1]] as const;
    const opticalWidth = Math.tan(15 * Math.PI / 180);
    const opticalHeight = Math.tan(10 * Math.PI / 180);
    for (const element of view.layer.elements) for (const eye of viewer.eyes) {
      const projection = new Matrix4().fromArray(eye.projection);
      const eyeFromHead = new Matrix4().compose(
        new Vector3(eye.headFromEye.position.x, eye.headFromEye.position.y, eye.headFromEye.position.z),
        new Quaternion(eye.headFromEye.orientation.x, eye.headFromEye.orientation.y, eye.headFromEye.orientation.z, eye.headFromEye.orientation.w),
        new Vector3(1, 1, 1)).invert();
      const opticalCone = corners.map(([horizontal, vertical]) => new Vector3(horizontal * opticalWidth, vertical * opticalHeight, -1).applyMatrix4(projection));
      const card = corners.map(([horizontal, vertical]) => new Vector3(
        (element.bounds.left + (horizontal + 1) * element.bounds.width / 2 - 0.5) * view.layer.size.width,
        (0.5 - element.bounds.top - (1 - vertical) * element.bounds.height / 2) * view.layer.size.height,
        -DEFAULT_HEAD_HUD_PROFILE.distanceMeters).applyMatrix4(eyeFromHead).applyMatrix4(projection));
      expect(convexQuadsOverlap(card, opticalCone)).toBe(false);
      for (const horizontal of [element.bounds.left, element.bounds.left + element.bounds.width]) {
        for (const vertical of [element.bounds.top, element.bounds.top + element.bounds.height]) {
          const point = transformPoint(view.layer.localPose, vec3((horizontal - 0.5) * view.layer.size.width, (0.5 - vertical) * view.layer.size.height, 0));
          const projected = projectHeadPoint(eye, point);
          expect(projected).not.toBeNull();
          expect(Math.abs(projected?.x ?? 2)).toBeLessThan(1 - DEFAULT_HEAD_HUD_PROFILE.clipMargin);
          expect(Math.abs(projected?.y ?? 2)).toBeLessThan(1 - DEFAULT_HEAD_HUD_PROFILE.clipMargin);
          expect(projected?.z).toBeGreaterThanOrEqual(-1);
          expect(projected?.z).toBeLessThanOrEqual(1);
        }
      }
    }
    expect(createHeadHudView(model, { ...viewer, trackingFromHead: pose(vec3(30, -7, 9), quaternion(0.7, 0.3, -0.4, 0.2)) })).toEqual(view);
  });

  it.each([1280 / 720, 720 / 1280])("keeps clear angles and minimum text height when aspect is %s", (aspect) => {
    const view = createHeadHudView(createFlightHudModel(snapshot, 0), configuredViewer(aspect));
    if (view.kind !== "visible") throw new Error("Expected phone profile fit");
    expect(Math.atan(view.layer.clearRegion.height * view.layer.size.height / (2 * DEFAULT_HEAD_HUD_PROFILE.distanceMeters)) * 180 / Math.PI).toBeCloseTo(10, 10);
    expect(Math.atan(view.textHeightMeters / DEFAULT_HEAD_HUD_PROFILE.distanceMeters) * 180 / Math.PI).toBeCloseTo(DEFAULT_HEAD_HUD_PROFILE.textHeightDegrees, 10);
    expect(() => { validateHeadHudLayer(view.layer); }).not.toThrow();
  });

  it("reports geometry loss and insufficient readable area without stale-layout fallback", () => {
    const model = createFlightHudModel(snapshot, 0);
    expect(createHeadHudView(model, configuredViewer()).kind).toBe("visible");
    expect(createHeadHudView(model, unavailableViewerFrame("unsupported-view-configuration", IDENTITY_POSE))).toEqual({ kind: "unavailable", reason: "unsupported-view-configuration" });
    expect(createHeadHudView(model, configuredViewer(0.05))).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    expect(createHeadHudView(model, configuredViewer(1, 20))).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    expect(createHeadHudView(model, configuredViewer(1, 60, 3))).toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
    expect(createHeadHudView(createFlightHudModel(snapshot, 4, { ...profile, telemetry: false, attitude: false, wind: false, flightPath: false, angleOfAttack: false, warnings: false }), unavailableViewerFrame("viewer-unavailable"))).toEqual({ kind: "absent" });
  });

  it("finalizes the Head layer and Menu explanation together while Screen and paused views remain absent", () => {
    const initial = createInitialAppModel();
    const running = { ...initial, gameSession: sessionForPhase(5), presentation: { type: "ready" as const, mode: "phone-vr" as const } };
    const head = createHeadHudView(createFlightHudModel(snapshot, 0), configuredViewer());
    if (head.kind !== "visible") throw new Error("Missing Head view");
    const view = createGameViewModel(running, snapshot, null, head);
    expect(view.headHud).toBe(head.layer);
    expect(view.panels[0]?.anchor).toBe("menu");
    expect(view.panels[0]?.controls.map((control) => control.id)).toEqual(["game-flight-pause"]);
    expect(viewExposesAction(view, { type: "activate", controlId: "head-attitude" })).toBe(false);
    expect(() => { validateUiViewModel(view); }).not.toThrow();
    const unavailable = createGameViewModel(running, snapshot, null, { kind: "unavailable", reason: "text-overflow" });
    expect(unavailable.headHud).toEqual({ kind: "absent" });
    expect(unavailable.panels[0]?.controls.some((control) => control.id === "game-head-hud-unavailable")).toBe(true);
    const intentionallyAbsent = createGameViewModel(running, snapshot, null, { kind: "absent" });
    expect(intentionallyAbsent.panels[0]?.controls.some((control) => control.id === "game-head-hud-unavailable")).toBe(false);
    expect(createGameViewModel({ ...running, presentation: { type: "ready", mode: "screen" } }, snapshot, null, head).headHud).toEqual({ kind: "absent" });
    for (const phase of [0, 1, 3, 4, 6, 7, 8, 10]) {
      const projected = createGameViewModel({ ...running, gameSession: sessionForPhase(phase) }, snapshot, null, head);
      expect(projected.headHud).toEqual({ kind: "absent" });
      expect(projected.panels[0]?.anchor).toBe("menu");
    }
  });
});
