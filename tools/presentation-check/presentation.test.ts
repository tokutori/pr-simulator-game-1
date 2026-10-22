import "./lake-model-fixture.js";
import { referenceFrameFixture } from "./menu-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { fixturePresentation } from "./menu-fixture.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { createPilotEyePoint, pilotEyePoseFrd, pilotEyePoseRender } from "../../web/src/render/camera/spatial.js";
import { chaseCameraPoseFrd } from "../../web/src/render/camera/spatial.js";
import { IDENTITY_POSE, pose, rotateVec3, vec3 } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, MenuPresentation, PresentationBackendAdapter, PresentationMode, RendererAdapter, ViewportSize } from "../../web/src/render/contracts/runtime.js";

import type { UiViewModel } from "../../web/src/render/contracts/ui.js";
import { createSceneFixture } from "./ui-adapter-fixture.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { sharedPanelHit } from "./menu-fixture.js";
import type { UiPanel } from "../../web/src/render/contracts/ui.js";

describe("same-frame view projection boundary", () => {
  it("passes the same immutable binocular frame into pure view derivation and the backend", async () => {
    const renderer = new FakeRenderer();
    const backend = new FakeBackend("webxr", new Set());
    const head = pose(vec3(1, 2, 3), IDENTITY_POSE.orientation);
    const frame = unavailableViewerFrame("invalid-view-geometry", head);
    let projectedFrame: ViewerFrame | null = null;
    let backendViewer: ViewerFrame | null = null;
    const originalFrame = backend.currentFrame.bind(backend);
    backend.currentFrame = (timestampMs, frameView, viewerPose, menu) => {
      backendViewer = viewerPose;
      return originalFrame(timestampMs, frameView, viewerPose, menu);
    };
    const runtime = new PresentationRuntime(renderer, [backend], (viewer) => {
      projectedFrame = viewer;
      return fixturePresentation(createSceneFixture("Flight"));
    });
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    renderer.tick(25, frame);
    expect(projectedFrame).toBe(frame);
    expect(backendViewer).toBe(frame);
    renderer.tick(26);
    expect(projectedFrame).toEqual(unavailableViewerFrame("not-stereo"));
    expect(backendViewer).toEqual(unavailableViewerFrame("not-stereo"));
    await runtime.dispose();
  });
});

describe("anchor and camera transforms", () => {
  it("moves PilotEyePoint only by the pilot displacement along FRD x", () => {
    const eyePoint = createPilotEyePoint(vec3(1.2, 0.04, -0.08), 0.35, IDENTITY_POSE.orientation);
    const eye = pilotEyePoseFrd(eyePoint, 0.65);
    expect(eye.position).toEqual(vec3(1.5, 0.04, -0.08));
    expect(eye.orientation).toEqual(IDENTITY_POSE.orientation);
  });

  it("maps the pilot eye point from FRD into the renderer's Three.js axes", () => {
    const eyePoint = createPilotEyePoint(vec3(1.2, 0.04, -0.08), 0.35, IDENTITY_POSE.orientation);
    const eye = pilotEyePoseRender(eyePoint, 0.65, 0.35);

    expect(eye.position).toEqual(vec3(0.04, 0.08, -1.5));
    expect(eye.orientation).toEqual(IDENTITY_POSE.orientation);
  });

  it("defines an engine-independent Chase pose behind and above the aircraft", () => {
    const chase = chaseCameraPoseFrd("chase");
    const forward = rotateVec3(chase.orientation, vec3(1, 0, 0));
    expect(chase.position).toEqual(vec3(-12, 0, -4));
    expect(forward.x).toBeGreaterThan(0);
    expect(forward.z / forward.x).toBeCloseTo(1 / 3, 10);
    expect(chaseCameraPoseFrd("pilot")).toEqual(IDENTITY_POSE);
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
    for (const ray of [leftRay, rightRay]) {
      const length = Math.hypot(ray.direction.x, ray.direction.y, ray.direction.z);
      ray.direction = vec3(ray.direction.x / length, ray.direction.y / length, ray.direction.z / length);
    }
    const leftHit = sharedPanelHit(panel, worldFromPanel, leftRay);
    const rightHit = sharedPanelHit(panel, worldFromPanel, rightRay);
    expect(leftHit).not.toBeNull();
    expect(rightHit).not.toBeNull();
    if (leftHit === null || rightHit === null) throw new Error("Expected both eye rays to hit the panel");
    expect(leftHit.x).toBeCloseTo(rightHit.x, 10);
    expect(leftHit.y).toBeCloseTo(rightHit.y, 10);
    expect(leftHit.controlId).toBe(button.id);
    expect(rightHit.controlId).toBe(button.id);
  });
});


describe("presentation runtime", () => {
  it.each(["screen", "phone-vr", "webxr"] as const)("observes one successfully rendered frame per %s callback and resets on backend changes", async (mode) => {
    const renderer = new FakeRenderer();
    const active = new Set<PresentationMode>();
    const callbacks: number[] = [];
    const backends = ["screen", "phone-vr", "webxr"].map((backend) => new FakeBackend(backend as PresentationMode, active));
    const runtime = new PresentationRuntime(renderer, backends, () => fixturePresentation(createSceneFixture("Title")),
      (timestamp) => { callbacks.push(timestamp); });
    expect(await runtime.start(mode)).toEqual({ ok: true });
    for (let frame = 0; frame <= 60; frame++) renderer.tick(frame * 1000 / 60);
    expect(runtime.framesPerSecond).toBeCloseTo(60, 10);
    expect(renderer.frames).toHaveLength(61);
    expect(callbacks).toHaveLength(61);
    expect(await runtime.switchTo(mode === "screen" ? "phone-vr" : "screen")).toEqual({ ok: true });
    expect(runtime.framesPerSecond).toBeNull();
    expect(renderer.startCount).toBe(1);
    await runtime.dispose();
    expect(runtime.framesPerSecond).toBeNull();
  });

  it("does not count a renderer failure as a completed frame", async () => {
    const renderer = new FakeRenderer();
    const backend = new FakeBackend("screen", new Set());
    const runtime = new PresentationRuntime(renderer, [backend], () => fixturePresentation(createSceneFixture("Title")));
    await runtime.start("screen");
    renderer.tick(0);
    renderer.render = () => { throw new Error("render failed"); };
    expect(() => { renderer.tick(750); }).toThrow("render failed");
    expect(runtime.framesPerSecond).toBeNull();
    await runtime.dispose();
  });

  it("keeps one engine loop while switching backend and disposes owned resources", async () => {
    const active = new Set<PresentationMode>();
    const renderer = new FakeRenderer();
    const backends = ["screen", "webxr", "phone-vr"].map((mode) => new FakeBackend(mode as PresentationMode, active));
    const runtime = new PresentationRuntime(renderer, backends, () => fixturePresentation(createSceneFixture("Title")));
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
    const runtime = new PresentationRuntime(renderer, [screen, webxr], () => fixturePresentation(createSceneFixture("Flight")));
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

  it.each([
    ["stop-loop"], ["stop-backend"], ["dispose-renderer"],
    ["stop-loop", "stop-backend"], ["stop-loop", "dispose-renderer"], ["stop-backend", "dispose-renderer"],
    ["stop-loop", "stop-backend", "dispose-renderer"]
  ] as const)("attempts every independent cleanup and preserves each cause for %j", async (...failedOperations) => {
    for (const mode of ["screen", "webxr", "phone-vr"] as const) {
      const active = new Set<PresentationMode>();
      const renderer = new FakeRenderer();
      const backend = new FakeBackend(mode, active);
      const calls: string[] = [];
      const causes = {
        "stop-loop": new Error("Injected loop stop failure"),
        "stop-backend": new Error("Injected backend stop failure"),
        "dispose-renderer": new Error("Injected renderer disposal failure")
      };
      const failed: readonly string[] = failedOperations;
      const stopLoop = renderer.stopLoop.bind(renderer);
      vi.spyOn(renderer, "stopLoop").mockImplementation(() => {
        calls.push("stop-loop");
        if (failed.includes("stop-loop")) throw causes["stop-loop"];
        stopLoop();
      });
      const stopBackend = backend.stop.bind(backend);
      vi.spyOn(backend, "stop").mockImplementation(async () => {
        calls.push("stop-backend");
        await stopBackend();
        if (failed.includes("stop-backend")) throw causes["stop-backend"];
      });
      const disposeRenderer = renderer.dispose.bind(renderer);
      vi.spyOn(renderer, "dispose").mockImplementation(() => {
        calls.push("dispose-renderer");
        disposeRenderer();
        if (failed.includes("dispose-renderer")) throw causes["dispose-renderer"];
      });
      const runtime = new PresentationRuntime(renderer, [backend], () => fixturePresentation(createSceneFixture("Title")));
      expect(await runtime.start(mode)).toEqual({ ok: true });
      renderer.tick(0);
      const disposal = runtime.dispose();
      const result = await disposal;
      expect(calls).toEqual(["stop-loop", "stop-backend", "dispose-renderer"]);
      expect(result).toEqual({ ok: false, error: { type: "cleanup-failed", failures: failedOperations.map((operation) => ({
        type: operation === "stop-backend" ? "backend-failed" : "renderer-failed",
        operation,
        ...(operation === "stop-backend" ? { mode } : {}),
        message: causes[operation].message,
        cause: causes[operation]
      })) } });
      if (result.ok || result.error.type !== "cleanup-failed") throw new Error("Expected retained cleanup causes");
      result.error.failures.forEach((failure, index) => {
        const operation = failedOperations[index];
        if (operation === undefined) throw new Error("Unexpected cleanup failure index");
        expect(failure.cause).toBe(causes[operation]);
      });
      expect(runtime.currentMode).toBeNull();
      expect(runtime.framesPerSecond).toBeNull();
      renderer.tick(1_000);
      expect(renderer.frames).toHaveLength(1);
      expect(await runtime.start(mode)).toEqual({ ok: false, error: { type: "disposed" } });
      expect(await runtime.switchTo(mode)).toEqual({ ok: false, error: { type: "disposed" } });
      expect(runtime.resize({ x: 800, y: 600, pixelRatio: 1 })).toEqual({ ok: false, error: { type: "disposed" } });
      expect(runtime.dispose()).toBe(disposal);
      expect(await runtime.dispose()).toBe(result);
      expect(calls).toEqual(["stop-loop", "stop-backend", "dispose-renderer"]);
      expect(renderer.disposeCount).toBe(1);
      expect(active.size).toBe(0);
    }
  });

  it("shares the disposal result before synchronous reentry and while backend cleanup is pending", async () => {
    const renderer = new FakeRenderer();
    const backend = new FakeBackend("phone-vr", new Set());
    const runtime = new PresentationRuntime(renderer, [backend], () => fixturePresentation(createSceneFixture("Title")));
    await runtime.start("phone-vr");
    let reentrantDisposal: ReturnType<PresentationRuntime["dispose"]> | null = null;
    let completeStop: () => void = () => { throw new Error("Backend stop gate is not initialized"); };
    const stopGate = new Promise<void>((resolve) => { completeStop = resolve; });
    const originalStopLoop = renderer.stopLoop.bind(renderer);
    vi.spyOn(renderer, "stopLoop").mockImplementation(() => {
      reentrantDisposal = runtime.dispose();
      originalStopLoop();
    });
    const stopBackend = vi.spyOn(backend, "stop").mockImplementation(() => stopGate);
    const disposal = runtime.dispose();
    expect(runtime.dispose()).toBe(disposal);
    const pendingStart = runtime.start("screen");
    await Promise.resolve();
    expect(reentrantDisposal).toBe(disposal);
    expect(runtime.currentMode).toBeNull();
    expect(renderer.disposeCount).toBe(0);
    expect(stopBackend).toHaveBeenCalledTimes(1);
    completeStop();
    expect(await disposal).toEqual({ ok: true });
    expect(await pendingStart).toEqual({ ok: false, error: { type: "disposed" } });
    expect(runtime.dispose()).toBe(disposal);
    expect(renderer.stopCount).toBe(1);
    expect(renderer.disposeCount).toBe(1);
    expect(stopBackend).toHaveBeenCalledTimes(1);
  });

  it("retains an unprintable backend cause and continues renderer disposal", async () => {
    const renderer = new FakeRenderer();
    const backend = new FakeBackend("webxr", new Set());
    const runtime = new PresentationRuntime(renderer, [backend], () => fixturePresentation(createSceneFixture("Title")));
    await runtime.start("webxr");
    const cause: unknown = Object.create(null);
    vi.spyOn(backend, "stop").mockImplementation(() => { throw cause; });
    const result = await runtime.dispose();
    expect(result).toEqual({ ok: false, error: { type: "cleanup-failed", failures: [{ type: "backend-failed", operation: "stop-backend", mode: "webxr",
      message: "Presentation operation failed with an unprintable cause", cause }] } });
    expect(renderer.disposeCount).toBe(1);
    if (result.ok || result.error.type !== "cleanup-failed") throw new Error("Expected retained backend cause");
    expect(result.error.failures[0].cause).toBe(cause);
  });

  it("preserves both backend and loop-stop causes when a failed switch requires cleanup", async () => {
    const renderer = new FakeRenderer();
    const active = new Set<PresentationMode>();
    const screen = new FakeBackend("screen", active);
    const webxr = new FakeBackend("webxr", active);
    const runtime = new PresentationRuntime(renderer, [screen, webxr], () => fixturePresentation(createSceneFixture("Title")));
    await runtime.start("webxr");
    const backendCause = new Error("Session end failed");
    const loopCause = new Error("Loop stop failed");
    vi.spyOn(webxr, "stop").mockRejectedValue(backendCause);
    const stopLoop = vi.spyOn(renderer, "stopLoop").mockImplementation(() => { throw loopCause; });
    const result = await runtime.switchTo("screen");
    expect(result).toEqual({ ok: false, error: { type: "cleanup-failed", failures: [
      { type: "backend-failed", operation: "stop-backend", mode: "webxr", message: backendCause.message, cause: backendCause },
      { type: "renderer-failed", operation: "stop-loop", message: loopCause.message, cause: loopCause }
    ] } });
    expect(runtime.currentMode).toBeNull();
    renderer.tick(1_000);
    expect(renderer.frames).toHaveLength(0);
    expect(screen.startCount).toBe(0);
    await runtime.dispose();
    expect(stopLoop).toHaveBeenCalledTimes(1);
    expect(renderer.disposeCount).toBe(1);
  });

  it("retains startup and backend-cleanup causes before stopping a failed target switch", async () => {
    const renderer = new FakeRenderer();
    const screen = new FakeBackend("screen", new Set());
    const phoneVr = new FakeBackend("phone-vr", new Set());
    const runtime = new PresentationRuntime(renderer, [screen, phoneVr], () => fixturePresentation(createSceneFixture("Title")));
    await runtime.start("screen");
    const startCause = new Error("Phone VR tracking startup failed");
    const cleanupCause = new Error("Phone VR sensor cleanup failed");
    vi.spyOn(phoneVr, "start").mockRejectedValue(startCause);
    vi.spyOn(phoneVr, "stop").mockRejectedValue(cleanupCause);
    const result = await runtime.switchTo("phone-vr");
    expect(result).toEqual({ ok: false, error: { type: "cleanup-failed", failures: [
      { type: "backend-failed", operation: "start-backend", mode: "phone-vr", message: startCause.message, cause: startCause },
      { type: "backend-failed", operation: "stop-backend", mode: "phone-vr", message: cleanupCause.message, cause: cleanupCause }
    ] } });
    expect(runtime.currentMode).toBeNull();
    expect(renderer.stopCount).toBe(1);
    await runtime.dispose();
    expect(renderer.disposeCount).toBe(1);
  });

  it("falls back to Screen instead of restarting a stopped WebXR backend", async () => {
    const active = new Set<PresentationMode>();
    const renderer = new FakeRenderer();
    const screen = new FakeBackend("screen", active);
    const webxr = new FakeBackend("webxr", active);
    const phoneVr = new FakeBackend("phone-vr", active, "sensor startup failed");
    const runtime = new PresentationRuntime(renderer, [screen, webxr, phoneVr], () => fixturePresentation(createSceneFixture("Boot")));
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
    const runtime = new PresentationRuntime(renderer, [screen, webxr], () => fixturePresentation(createSceneFixture("Boot")));
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
    const runtime = new PresentationRuntime(renderer, [], () => fixturePresentation(createSceneFixture("Boot")));
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
  setPreparedFlightPose(): void {}
  setLakeVisualCondition(): void {}
  setLakeWaterQuality(): Promise<{ readonly kind: "complete" }> { return Promise.resolve({ kind: "complete" }); }
  setLakeSkyCondition(): void {}
  setLakeVenueVisible(): void {}

  setFlightCameraMode(): void {}

  setCinematicCameraView(): void {}

  projectTrackingFrame(pose: Parameters<RendererAdapter["projectTrackingFrame"]>[0]) {
    return referenceFrameFixture(pose);
  }

  resize(viewport: ViewportSize): void {
    this.lastViewport = viewport;
  }

  setStereoPresentation(): void {}
  setScreenRenderScale(): void {}
  setScreenRenderAuto(): void {}

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

  currentFrame(timestampMs: number, view: UiViewModel, viewer: ViewerFrame, menu: MenuPresentation): BackendFrame {
    if (!this.running) throw new Error(`${this.mode} is inactive`);
    return Object.freeze({
      timestampMs,
      headHud: { kind: "absent" as const },
      cameraPose: viewer.trackingFromHead ?? IDENTITY_POSE,
      panel: this.mode === "screen" ? Object.freeze({ kind: "absent" as const }) : Object.freeze({ kind: "visible" as const,
        panel: menu.kind === "ready" ? menu.panel : requiredPanel(view), pose: IDENTITY_POSE, cursor: null }),
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
