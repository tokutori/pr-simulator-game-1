import { describe, expect, it } from "vitest";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { createPilotEyePoint, pilotEyePoseFrd, pilotEyePoseThree } from "../../web/src/render/camera/pilot-eye-point.js";
import { replayCameraPoseFrd } from "../../web/src/render/camera/replay-camera.js";
import { MenuAnchorPlacement, resolveAnchorPose } from "../../web/src/render/anchors.js";
import type { AnchorFrames } from "../../web/src/render/anchors.js";
import { IDENTITY_POSE, pose, quaternion, rotateVec3, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, PresentationBackendAdapter, PresentationMode, RendererAdapter, ViewportSize } from "../../web/src/render/contracts/runtime.js";
import { GAME_SCENES, validateUiViewModel } from "../../web/src/render/contracts/ui.js";
import type { UiAction, UiViewModel } from "../../web/src/render/contracts/ui.js";
import { createAllSceneFixtures, createSceneFixture, SCENE_FIXTURE_OVERLAYS } from "../../web/src/presentation/fixtures.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { GazeDwellSelector } from "../../web/src/presentation/gaze-dwell.js";
import { actionForControl, hitTestControl, intersectPanel, rangeAction } from "../../web/src/presentation/panel-interaction.js";
import { drawVrPanel } from "../../web/src/presentation/vr-panel-canvas.js";
import type { PanelDrawingContext } from "../../web/src/presentation/vr-panel-canvas.js";
import type { UiPanel } from "../../web/src/render/contracts/ui.js";

describe("scene and overlay fixtures", () => {
  it("provides valid view models for all eight GameScenes", () => {
    const fixtures = createAllSceneFixtures();
    expect(GAME_SCENES).toHaveLength(8);
    expect(fixtures.map((fixture) => fixture.scene)).toEqual(GAME_SCENES);
    for (const fixture of fixtures) {
      validateUiViewModel(fixture);
      expect(fixture.panels).not.toHaveLength(0);
    }
  });

  it("covers each permitted overlay and rejects scene-incompatible overlays", () => {
    for (const scene of GAME_SCENES) {
      for (const overlay of SCENE_FIXTURE_OVERLAYS[scene]) {
        const fixture = createSceneFixture(scene, overlay);
        validateUiViewModel(fixture);
        expect(fixture.activeOverlay).toBe(overlay);
      }
    }
    expect(() => createSceneFixture("Countdown", "Settings")).toThrow("not available");
  });

  it("anchors Flight HUD to Cockpit and paused overlays to Menu", () => {
    expect(requiredPanel(createSceneFixture("Flight")).anchor).toBe("cockpit");
    for (const overlay of ["Pause", "Pause/Settings", "Pause/Help"]) {
      expect(requiredPanel(createSceneFixture("Flight", overlay)).anchor).toBe("menu");
    }
  });

  it("uses the same control actions for screen and spatial panel interactions", () => {
    for (const scene of GAME_SCENES) {
      const fixture = createSceneFixture(scene);
      const panel = requiredPanel(fixture);
      const button = panel.controls.find((control) => control.kind === "button");
      const toggle = panel.controls.find((control) => control.kind === "toggle");
      const range = panel.controls.find((control) => control.kind === "range");
      expect(button === undefined ? null : actionForControl(button)).toEqual({ type: "activate", controlId: `${scene.toLowerCase()}-action` });
      expect(toggle === undefined ? null : actionForControl(toggle)).toEqual({
        type: "set-toggle", controlId: `${scene.toLowerCase()}-toggle`, value: true
      });
      if (range === undefined) throw new Error(`Missing range fixture for ${scene}`);
      const rightEdge = normalizedPoint(panel, range.rect.x + range.rect.width, range.rect.y + range.rect.height / 2);
      expect(rangeAction(range, panel, rightEdge)).toEqual({
        type: "set-range", controlId: `${scene.toLowerCase()}-range`, value: 1
      });
    }
  });

  it("draws the common panel model into the VR Canvas surface", () => {
    const panel = requiredPanel(createSceneFixture("Title"));
    const labels: string[] = [];
    const context: PanelDrawingContext = {
      clearRect: () => undefined,
      fillRect: () => undefined,
      fillText: (text: string) => labels.push(text),
      strokeRect: () => undefined,
      beginPath: () => undefined,
      closePath: () => undefined,
      rect: () => undefined,
      clip: () => undefined,
      save: () => undefined,
      restore: () => undefined,
      fill: () => undefined,
      moveTo: () => undefined,
      lineTo: () => undefined,
      stroke: () => undefined,
      setFillStyle: () => undefined,
      setStrokeStyle: () => undefined,
      setFont: () => undefined,
      setTextBaseline: () => undefined,
      setLineWidth: () => undefined,
      setGlobalAlpha: () => undefined
    };
    drawVrPanel(context, panel, 1024, 768);
    expect(labels).toContain(panel.title);
    expect(labels).toContain("共通actionを送信");
  });

  it("renders multiline Result metadata inside an enlarged VR panel row", () => {
    const panel = requiredPanel(createSceneFixture("Result"));
    const labels: string[] = [];
    const fonts: string[] = [];
    const context: PanelDrawingContext = {
      clearRect: () => undefined,
      fillRect: () => undefined,
      fillText: (text: string) => labels.push(text),
      strokeRect: () => undefined,
      beginPath: () => undefined,
      closePath: () => undefined,
      rect: () => undefined,
      clip: () => undefined,
      save: () => undefined,
      restore: () => undefined,
      fill: () => undefined,
      moveTo: () => undefined,
      lineTo: () => undefined,
      stroke: () => undefined,
      setFillStyle: () => undefined,
      setStrokeStyle: () => undefined,
      setFont: (value: string) => fonts.push(value),
      setTextBaseline: () => undefined,
      setLineWidth: () => undefined,
      setGlobalAlpha: () => undefined
    };
    const metadata = {
      kind: "button" as const,
      id: "game-result-configuration",
      label: "Selected axes\nScenario and model versions",
      enabled: false,
      rect: { x: 0.08, y: 0.6, width: 0.84, height: 0.11 }
    };
    drawVrPanel(context, { ...panel, controls: [metadata] }, 1024, 768);
    expect(labels).toContain("Selected axes");
    expect(labels).toContain("Scenario and model versions");
    expect(fonts).toContain("500 17px system-ui, sans-serif");
  });
});

describe("same-frame view projection boundary", () => {
  it("passes the exact immutable frame into pure view derivation and its raw head into the backend", async () => {
    const renderer = new FakeRenderer();
    const backend = new FakeBackend("webxr", new Set());
    const head = pose(vec3(1, 2, 3), IDENTITY_POSE.orientation);
    const frame = unavailableViewerFrame("invalid-view-geometry", head);
    let projectedFrame: ViewerFrame | null = null;
    let backendPose: Pose | null = null;
    const originalFrame = backend.currentFrame.bind(backend);
    backend.currentFrame = (timestampMs, _view, viewerPose) => {
      backendPose = viewerPose ?? null;
      return originalFrame(timestampMs, _view, viewerPose);
    };
    const runtime = new PresentationRuntime(renderer, [backend], (viewer) => {
      projectedFrame = viewer;
      return createSceneFixture("Flight");
    });
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    renderer.tick(25, frame);
    expect(projectedFrame).toBe(frame);
    expect(backendPose).toBe(head);
    renderer.tick(26);
    expect(projectedFrame).toEqual(unavailableViewerFrame("not-stereo"));
    expect(backendPose).toBeNull();
    await runtime.dispose();
  });
});

describe("anchor and camera transforms", () => {
  it("keeps Menu fixed while Head changes and lets Cockpit follow the aircraft", () => {
    const frames: AnchorFrames = {
      world: IDENTITY_POSE,
      cockpit: pose(vec3(4, 0, 0), IDENTITY_POSE.orientation),
      menu: pose(vec3(1, 2, 3), IDENTITY_POSE.orientation),
      head: pose(vec3(0, 1.6, 0), IDENTITY_POSE.orientation)
    };
    const world = resolveAnchorPose({ kind: "world", localPose: IDENTITY_POSE }, frames);
    const menu = resolveAnchorPose({ kind: "menu", localPose: IDENTITY_POSE }, frames);
    const cockpit = resolveAnchorPose({ kind: "cockpit", localPose: IDENTITY_POSE }, frames);
    const headMoved: AnchorFrames = { ...frames, head: pose(vec3(10, 4, 0), IDENTITY_POSE.orientation) };
    expect(resolveAnchorPose({ kind: "world", localPose: IDENTITY_POSE }, headMoved)).toEqual(world);
    expect(resolveAnchorPose({ kind: "menu", localPose: IDENTITY_POSE }, headMoved)).toEqual(menu);
    expect(resolveAnchorPose({ kind: "cockpit", localPose: IDENTITY_POSE }, headMoved)).toEqual(cockpit);
    expect(resolveAnchorPose({ kind: "head", localPose: IDENTITY_POSE }, headMoved).position.x).toBe(10);
  });

  it("places a menu once and changes it only after explicit recenter", () => {
    const placement = new MenuAnchorPlacement();
    const initialHead = pose(vec3(0, 1.6, 0), IDENTITY_POSE.orientation);
    const opened = placement.open(initialHead, 2);
    const laterHead = pose(vec3(1, 1.6, 0), quaternion(Math.cos(Math.PI / 4), 0, Math.sin(Math.PI / 4), 0));
    expect(placement.open(laterHead, 2)).toEqual(opened);
    expect(placement.recenter(laterHead, 2)).not.toEqual(opened);
  });

  it("moves PilotEyePoint only by the pilot displacement along FRD x", () => {
    const eyePoint = createPilotEyePoint(vec3(1.2, 0.04, -0.08), 0.35, IDENTITY_POSE.orientation);
    const eye = pilotEyePoseFrd(eyePoint, 0.65);
    expect(eye.position).toEqual(vec3(1.5, 0.04, -0.08));
    expect(eye.orientation).toEqual(IDENTITY_POSE.orientation);
  });

  it("maps the pilot eye point from FRD into the renderer's Three.js axes", () => {
    const eyePoint = createPilotEyePoint(vec3(1.2, 0.04, -0.08), 0.35, IDENTITY_POSE.orientation);
    const eye = pilotEyePoseThree(eyePoint, 0.65, 0.35);

    expect(eye.position).toEqual(vec3(0.04, 0.08, -1.5));
    expect(eye.orientation).toEqual(IDENTITY_POSE.orientation);
  });

  it("defines an engine-independent Chase pose behind and above the aircraft", () => {
    const chase = replayCameraPoseFrd("chase");
    const forward = rotateVec3(chase.orientation, vec3(1, 0, 0));
    expect(chase.position).toEqual(vec3(-12, 0, -4));
    expect(forward.x).toBeGreaterThan(0);
    expect(forward.z / forward.x).toBeCloseTo(1 / 3, 10);
    expect(replayCameraPoseFrd("pilot")).toEqual(IDENTITY_POSE);
  });

  it("maps binocular rays to the same control hit position", () => {
    const panel = requiredPanel(createSceneFixture("Title"));
    const button = panel.controls.find((control) => control.kind === "button");
    if (button === undefined) throw new Error("Missing button fixture");
    const normalized = { x: button.rect.x + button.rect.width / 2, y: button.rect.y + button.rect.height / 2 };
    const localPoint = normalizedPoint(panel, normalized.x, normalized.y);
    const target = vec3(localPoint.x, localPoint.y, -2);
    const worldFromPanel = pose(vec3(0, 0, -2), IDENTITY_POSE.orientation);
    const leftRay = { origin: vec3(-0.032, 0, 0), direction: vec3(target.x + 0.032, target.y, target.z) };
    const rightRay = { origin: vec3(0.032, 0, 0), direction: vec3(target.x - 0.032, target.y, target.z) };
    const leftHit = intersectPanel(leftRay, worldFromPanel);
    const rightHit = intersectPanel(rightRay, worldFromPanel);
    expect(leftHit).not.toBeNull();
    expect(rightHit).not.toBeNull();
    if (leftHit === null || rightHit === null) throw new Error("Expected both eye rays to hit the panel");
    expect(leftHit.x).toBeCloseTo(rightHit.x, 10);
    expect(leftHit.y).toBeCloseTo(rightHit.y, 10);
    expect(hitTestControl(panel, leftHit)?.id).toBe(button.id);
    expect(hitTestControl(panel, rightHit)?.id).toBe(button.id);
  });
});

describe("head-gaze selection", () => {
  it("focuses a control, exposes dwell progress, activates once, and clears on cancellation", () => {
    const panel = requiredPanel(createSceneFixture("Title"));
    const button = panel.controls.find((control) => control.kind === "button");
    if (button === undefined) throw new Error("Missing button fixture");
    const point = normalizedPoint(panel, button.rect.x + button.rect.width / 2, button.rect.y + button.rect.height / 2);
    const actions: UiAction[] = [];
    const selector = new GazeDwellSelector((action) => { actions.push(action); }, 1000);
    expect(selector.update(panel, point, 100)?.progress).toBe(0);
    expect(selector.update(panel, point, 600)?.progress).toBe(0.5);
    expect(selector.update(panel, point, 1100)?.progress).toBe(1);
    expect(selector.update(panel, point, 1600)?.progress).toBe(1);
    expect(actions.filter((action) => action.type === "activate")).toHaveLength(1);
    expect(selector.wasActivatedRecently(button.id, {
      origin: vec3(0, 0, 0), direction: vec3(0, 0, -1), timestampMs: 1200
    })).toBe(true);
    expect(selector.update(panel, null, 1700)).toBeNull();
    expect(actions.at(-1)).toEqual({ type: "focus", controlId: null });
  });

  it("converts a dwell on a range control to the selected value", () => {
    const panel = requiredPanel(createSceneFixture("Title"));
    const range = panel.controls.find((control) => control.kind === "range");
    if (range === undefined) throw new Error("Missing range fixture");
    const point = normalizedPoint(panel, range.rect.x + range.rect.width, range.rect.y + range.rect.height / 2);
    const actions: UiAction[] = [];
    const selector = new GazeDwellSelector((action) => { actions.push(action); }, 500);
    selector.update(panel, point, 200);
    selector.update(panel, point, 700);
    expect(actions).toContainEqual({ type: "set-range", controlId: range.id, value: 1 });
  });
});

describe("presentation runtime", () => {
  it("keeps one engine loop while switching backend and disposes owned resources", async () => {
    const active = new Set<PresentationMode>();
    const renderer = new FakeRenderer();
    const backends = ["screen", "webxr", "phone-vr"].map((mode) => new FakeBackend(mode as PresentationMode, active));
    const runtime = new PresentationRuntime(renderer, backends, () => createSceneFixture("Title"));
    expect(await runtime.start("screen")).toEqual({ ok: true });
    renderer.tick(10);
    expect(await runtime.switchTo("webxr")).toEqual({ ok: true });
    renderer.tick(20);
    expect(await runtime.switchTo("phone-vr")).toEqual({ ok: true });
    renderer.tick(30);
    expect(await runtime.switchTo("screen")).toEqual({ ok: true });
    expect(renderer.startCount).toBe(1);
    expect(renderer.frames).toHaveLength(3);
    expect(active.size).toBe(1);
    expect(await runtime.dispose()).toEqual({ ok: true });
    expect(renderer.stopCount).toBe(1);
    expect(renderer.disposeCount).toBe(1);
    expect(active.size).toBe(0);
  });

  it("falls back to Screen after a rejected backend start", async () => {
    const active = new Set<PresentationMode>();
    const renderer = new FakeRenderer();
    const screen = new FakeBackend("screen", active);
    const webxr = new FakeBackend("webxr", active, "permission denied");
    const runtime = new PresentationRuntime(renderer, [screen, webxr], () => createSceneFixture("Flight"));
    expect(await runtime.start("screen")).toEqual({ ok: true });
    const switched = await runtime.switchTo("webxr");
    expect(switched.ok).toBe(false);
    if (switched.ok) throw new Error("Expected backend failure");
    expect(switched.error).toEqual({ type: "backend-failed", mode: "webxr", message: "permission denied" });
    expect(runtime.currentMode).toBe("screen");
    expect(active).toEqual(new Set(["screen"]));
    expect(screen.startCount).toBe(2);
    expect(webxr.startCount).toBe(1);
    expect(renderer.startCount).toBe(1);
    await runtime.dispose();
  });

  it("falls back to Screen instead of restarting a stopped WebXR backend", async () => {
    const active = new Set<PresentationMode>();
    const renderer = new FakeRenderer();
    const screen = new FakeBackend("screen", active);
    const webxr = new FakeBackend("webxr", active);
    const phoneVr = new FakeBackend("phone-vr", active, "sensor startup failed");
    const runtime = new PresentationRuntime(renderer, [screen, webxr, phoneVr], () => createSceneFixture("Boot"));
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    const switched = await runtime.switchTo("phone-vr");
    expect(switched.ok).toBe(false);
    expect(runtime.currentMode).toBe("screen");
    expect(active).toEqual(new Set(["screen"]));
    expect(webxr.startCount).toBe(1);
    expect(screen.startCount).toBe(1);
    expect(renderer.startCount).toBe(1);
    expect(renderer.stopCount).toBe(0);
    await runtime.dispose();
  });

  it("stops the render loop when Screen recovery also fails", async () => {
    const active = new Set<PresentationMode>();
    const renderer = new FakeRenderer();
    const screen = new FakeBackend("screen", active, "screen recovery failed", 2);
    const webxr = new FakeBackend("webxr", active, "session startup failed");
    const runtime = new PresentationRuntime(renderer, [screen, webxr], () => createSceneFixture("Boot"));
    expect(await runtime.start("screen")).toEqual({ ok: true });
    const switched = await runtime.switchTo("webxr");
    expect(switched.ok).toBe(false);
    expect(runtime.currentMode).toBeNull();
    expect(active).toEqual(new Set());
    expect(renderer.stopCount).toBe(1);
    renderer.tick(42);
    expect(renderer.frames).toHaveLength(0);
    await runtime.dispose();
  });

  it("returns an explicit unsupported result when no backend is registered", async () => {
    const renderer = new FakeRenderer();
    const runtime = new PresentationRuntime(renderer, [], () => createSceneFixture("Boot"));
    expect(await runtime.start("phone-vr")).toEqual({ ok: false, error: { type: "unsupported", mode: "phone-vr" } });
    expect(renderer.startCount).toBe(0);
    await runtime.dispose();
  });
});

class FakeRenderer implements RendererAdapter {
  beginViewFrame(): void {}
  startCount = 0;
  stopCount = 0;
  disposeCount = 0;
  readonly frames: BackendFrame[] = [];
  lastViewport: ViewportSize | null = null;
  private callback: Parameters<RendererAdapter["startLoop"]>[0] | null = null;

  startLoop(callback: Parameters<RendererAdapter["startLoop"]>[0]): void {
    if (this.callback !== null) throw new Error("Only one frame loop may run");
    this.callback = callback;
    this.startCount++;
  }

  stopLoop(): void {
    if (this.callback === null) return;
    this.callback = null;
    this.stopCount++;
  }

  render(frame: BackendFrame): void {
    this.frames.push(frame);
  }

  setFlightPose(): void {}
  setLakeVisualCondition(): void {}

  setFlightCameraMode(): void {}

  setCinematicCameraView(): void {}

  transformTrackingPose(pose: Parameters<RendererAdapter["transformTrackingPose"]>[0]): typeof pose {
    return pose;
  }

  resize(viewport: ViewportSize): void {
    this.lastViewport = viewport;
  }

  setStereoPresentation(): void {}

  setSelectRayHandler(): void {}

  dispose(): void {
    this.disposeCount++;
  }

  tick(timestampMs: number, viewer: ViewerFrame = unavailableViewerFrame("not-stereo")): void {
    this.callback?.(timestampMs, viewer);
  }
}

class FakeBackend implements PresentationBackendAdapter {
  private running = false;
  startCount = 0;

  constructor(
    readonly mode: PresentationMode,
    private readonly active: Set<PresentationMode>,
    private readonly failure: string | null = null,
    private readonly failOnAttempt = 1
  ) {}

  start(): Promise<void> {
    this.startCount++;
    if (this.running) throw new Error(`${this.mode} already running`);
    if (this.failure !== null && this.startCount === this.failOnAttempt) throw new Error(this.failure);
    if (this.active.size > 0) throw new Error("Multiple presentation backends became active");
    this.running = true;
    this.active.add(this.mode);
    return Promise.resolve();
  }

  stop(): Promise<void> {
    if (!this.running) return Promise.resolve();
    this.running = false;
    this.active.delete(this.mode);
    return Promise.resolve();
  }

  currentFrame(timestampMs: number, _view?: UiViewModel, viewerPose: Pose | null = null): BackendFrame {
    if (!this.running) throw new Error(`${this.mode} is inactive`);
    return Object.freeze({
      timestampMs,
      headHud: { kind: "absent" as const },
      cameraPose: viewerPose ?? IDENTITY_POSE,
      panelPose: IDENTITY_POSE,
      panel: null,
      panelVisible: this.mode !== "screen",
      gazeCursor: null,
      viewport: Object.freeze({ x: 800, y: 600, pixelRatio: 1 })
    });
  }
}

function requiredPanel(viewModel: UiViewModel): UiPanel {
  const panel = viewModel.panels[0];
  if (panel === undefined) throw new Error(`Missing panel for ${viewModel.scene}`);
  return panel;
}

function normalizedPoint(panel: UiPanel, x: number, y: number): { readonly x: number; readonly y: number } {
  return { x: (x - 0.5) * panel.size.width, y: (0.5 - y) * panel.size.height };
}
