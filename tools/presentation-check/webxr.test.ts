import { runtimeViewerFixture, visiblePanelFrame } from "./viewer-fixture.js";
import { describe, expect, it } from "vitest";
import { composePose, IDENTITY_POSE, inversePose, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { headPlaneFitsViews, unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { RendererAdapter, SelectRay, ViewportSize } from "../../web/src/render/contracts/runtime.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";
import { createAnchorFixture, createHeadHudFixture, createSceneFixture, SCENE_FIXTURE_OVERLAYS } from "../../web/src/presentation/fixtures.js";
import { WebXrPresentationBackend } from "../../web/src/presentation/webxr-backend.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "../../web/src/presentation/webxr-contracts.js";
import { GAME_SCENES } from "../../web/src/render/contracts/ui.js";

describe("WebXR session backend", () => {
  it("supplies the runtime viewer center to Head HUD independently of the identity user camera and transformed Menu", async () => {
    const adapter = new FakeWebXrAdapter();
    const renderer = new FakeRenderer();
    renderer.transformTrackingPose = (head) => composePose(pose(vec3(10, 2, 5), IDENTITY_POSE.orientation), head);
    const backend = new WebXrPresentationBackend(adapter, renderer, viewport, () => undefined, () => undefined);
    const hud = createHeadHudFixture();
    const view = { ...createSceneFixture("Flight"), headHud: hud };
    const runtimeHead = pose(vec3(0.02, 0.05, -0.03), IDENTITY_POSE.orientation);
    await backend.requestSessionFromUserGesture();
    await backend.start();
    try {
      const frame = backend.currentFrame(10, view, runtimeViewerFixture(runtimeHead));
      expect(frame.cameraPose).toEqual(IDENTITY_POSE);
      expect(frame.headHud).toEqual({ kind: "visible", trackingFromHead: runtimeHead, view: hud });
      expect((frame.panel.kind === "visible")).toBe(true);
      expect(visiblePanelFrame(frame).panel.controls.length).toBeGreaterThan(0);
      expect(backend.currentFrame(20, view, runtimeViewerFixture(null)).headHud).toEqual({ kind: "absent" });
      expect(backend.currentFrame(30, createSceneFixture("Flight"), runtimeViewerFixture(runtimeHead)).headHud).toEqual({ kind: "absent" });
    } finally { await backend.stop(); }
  });

  it("checks immersive support and requests a session synchronously from the user action", async () => {
    const adapter = new FakeWebXrAdapter();
    const backend = createBackend(adapter);
    expect(await backend.checkAvailability()).toEqual({ supported: true, message: "WebXR immersive-vr is supported" });
    const request = backend.requestSessionFromUserGesture();
    expect(adapter.requestCount).toBe(1);
    expect(await request).toEqual({ ok: true });
    await backend.start();
    expect((backend.currentFrame(10, createSceneFixture("Title"), runtimeViewerFixture(IDENTITY_POSE)).panel.kind === "visible")).toBe(true);
    await backend.stop();
    expect(adapter.endCount).toBe(1);
  });

  it("preserves the current scene on an unexpected session end", async () => {
    const adapter = new FakeWebXrAdapter();
    const onEnd: string[] = [];
    const dispatch: UiAction[] = [];
    const renderer = new FakeRenderer();
    const backend = new WebXrPresentationBackend(
      adapter, renderer, viewport, (action) => { dispatch.push(action); }, () => { onEnd.push("ended"); }
    );
    await backend.requestSessionFromUserGesture();
    await backend.start();
    adapter.emitUnexpectedEnd();
    await new Promise<void>((resolve) => { queueMicrotask(resolve); });
    expect(onEnd).toEqual(["ended"]);
    expect(adapter.active).toBe(false);
    expect(renderer.selectHandler).toBeNull();
    expect(dispatch).toHaveLength(0);
  });

  it("uses the reference-space pose for Menu and forwards XR select rays as common UI actions", async () => {
    const adapter = new FakeWebXrAdapter();
    const actions: UiAction[] = [];
    const renderer = new FakeRenderer();
    const backend = new WebXrPresentationBackend(
      adapter, renderer, viewport, (action) => { actions.push(action); }, () => undefined
    );
    await backend.requestSessionFromUserGesture();
    await backend.start();
    const headPose = pose(vec3(1, 1.6, 3), IDENTITY_POSE.orientation);
    const frame = backend.currentFrame(10, createSceneFixture("Title"), runtimeViewerFixture(headPose));
    expect(visiblePanelFrame(frame).pose.position.x).toBe(1);
    expect(visiblePanelFrame(frame).pose.position.y).toBe(1.6);
    expect(visiblePanelFrame(frame).pose.position.z).toBeLessThanOrEqual(headPose.position.z - 2.4);
    expect(headPlaneFitsViews(runtimeViewerFixture(headPose), composePose(inversePose(headPose), visiblePanelFrame(frame).pose),
      visiblePanelFrame(frame).panel.size.width, visiblePanelFrame(frame).panel.size.height, 0.035)).toBe(true);
    const panel = visiblePanelFrame(frame).panel;
    const button = panel.controls.find((control) => control.kind === "button");
    if (button === undefined) throw new Error("Expected a panel button");
    const normalizedX = button.rect.x + button.rect.width / 2;
    const normalizedY = button.rect.y + button.rect.height / 2;
    const localPoint = vec3((normalizedX - 0.5) * panel.size.width, (0.5 - normalizedY) * panel.size.height, 0);
    renderer.selectHandler?.({
      origin: vec3(visiblePanelFrame(frame).pose.position.x + localPoint.x, visiblePanelFrame(frame).pose.position.y + localPoint.y, visiblePanelFrame(frame).pose.position.z + 2),
      direction: vec3(0, 0, -1),
      timestampMs: 2000
    });
    expect(actions).toContainEqual({ type: "activate", controlId: button.id });
    await backend.stop();
  });

  it("remaps world-locked anchors after a reference-space reset", async () => {
    const adapter = new FakeWebXrAdapter();
    const backend = createBackend(adapter);
    await backend.requestSessionFromUserGesture();
    await backend.start();
    const initial = backend.currentFrame(10, createSceneFixture("Title"), runtimeViewerFixture(IDENTITY_POSE));
    adapter.emitReferenceSpaceReset(pose(vec3(1, 0, 0), IDENTITY_POSE.orientation));
    const remappedMenu = backend.currentFrame(15, createSceneFixture("Title"), runtimeViewerFixture(pose(vec3(-1, 0, 0), IDENTITY_POSE.orientation)));
    const remapped = backend.currentFrame(20, {
      ...createSceneFixture("Title"),
      panels: [createAnchorFixture("world")]
    }, runtimeViewerFixture(pose(vec3(-1, 0, 0), IDENTITY_POSE.orientation)));
    expect(visiblePanelFrame(initial).pose.position.x).toBe(0);
    expect(visiblePanelFrame(remappedMenu).pose.position.x).toBe(-1);
    expect(visiblePanelFrame(remapped).pose.position.x).toBe(-1);
    await backend.stop();
  });

  it("exits WebXR when a reference-space reset provides no remapping transform", async () => {
    const adapter = new FakeWebXrAdapter();
    const renderer = new FakeRenderer();
    const resetErrors: string[] = [];
    const backend = new WebXrPresentationBackend(
      adapter, renderer, viewport, () => undefined, () => undefined,
      () => { resetErrors.push("unresolvable"); }
    );
    await backend.requestSessionFromUserGesture();
    await backend.start();
    adapter.emitReferenceSpaceReset(null);
    expect(resetErrors).toEqual(["unresolvable"]);
    expect(renderer.selectHandler).toBeNull();
    await backend.stop();
    expect(adapter.endCount).toBe(1);
  });

  it("forwards controls from every scene and permitted overlay through the same XR action path", async () => {
    const adapter = new FakeWebXrAdapter();
    const actions: UiAction[] = [];
    const renderer = new FakeRenderer();
    const backend = new WebXrPresentationBackend(
      adapter, renderer, viewport, (action) => { actions.push(action); }, () => undefined
    );
    await backend.requestSessionFromUserGesture();
    await backend.start();
    let timestampMs = 0;
    for (const scene of GAME_SCENES) {
      const overlays = [null, ...SCENE_FIXTURE_OVERLAYS[scene]];
      for (const overlay of overlays) {
        const viewModel = createSceneFixture(scene, overlay);
        const frame = backend.currentFrame(++timestampMs, viewModel, runtimeViewerFixture(IDENTITY_POSE));
        const panel = visiblePanelFrame(frame).panel;
        const button = panel.controls.find((control) => control.kind === "button");
        if (button === undefined) throw new Error(`Missing button for ${scene}`);
        const normalizedX = button.rect.x + button.rect.width / 2;
        const normalizedY = button.rect.y + button.rect.height / 2;
        const point = vec3((normalizedX - 0.5) * panel.size.width, (0.5 - normalizedY) * panel.size.height, 0);
        renderer.selectHandler?.({
          origin: vec3(visiblePanelFrame(frame).pose.position.x + point.x, visiblePanelFrame(frame).pose.position.y + point.y, visiblePanelFrame(frame).pose.position.z + 2),
          direction: vec3(0, 0, -1),
          timestampMs: timestampMs + 1000
        });
        expect(actions).toContainEqual({ type: "activate", controlId: button.id });
        actions.length = 0;
      }
    }
    await backend.stop();
  });

  it.each(["absent", "cockpit"] as const)("opens a new fixed Menu after %s and holds placement data through a missing viewer", async (closedView) => {
    const adapter = new FakeWebXrAdapter();
    const renderer = new FakeRenderer();
    const actions: UiAction[] = [];
    const backend = new WebXrPresentationBackend(adapter, renderer, viewport, (action) => { actions.push(action); }, () => undefined);
    await backend.requestSessionFromUserGesture();
    await backend.start();
    try {
      const pause = createSceneFixture("Flight", "Pause");
      const initial = visiblePanelFrame(backend.currentFrame(10, pause, runtimeViewerFixture(IDENTITY_POSE)));
      const moved = pose(vec3(2, 0.5, -0.3), IDENTITY_POSE.orientation);
      const missing = backend.currentFrame(20, pause, unavailableViewerFrame("viewer-unavailable"));
      expect(missing.panel).toEqual({ kind: "unavailable", reason: "viewer-unavailable" });
      actions.length = 0;
      renderer.selectHandler?.({ origin: vec3(0, -0.5, 0), direction: vec3(0, 0, -1), timestampMs: 2000 });
      expect(actions).toEqual([]);
      expect(visiblePanelFrame(backend.currentFrame(30, pause, runtimeViewerFixture(moved))).pose).toEqual(initial.pose);
      const closed = createSceneFixture("Flight");
      backend.currentFrame(40, closedView === "absent" ? { ...closed, panels: [] } : closed, runtimeViewerFixture(moved));
      const reopened = visiblePanelFrame(backend.currentFrame(50, pause, runtimeViewerFixture(moved)));
      expect(reopened.pose.position.x).toBeCloseTo(initial.pose.position.x + 2, 10);
      expect(reopened.pose.position.y).toBeCloseTo(initial.pose.position.y + 0.5, 10);
      expect(visiblePanelFrame(backend.currentFrame(60, pause, runtimeViewerFixture(IDENTITY_POSE))).pose).toEqual(reopened.pose);
    } finally { await backend.stop(); }
  });

  it("reports missing support and rejected user session requests", async () => {
    const missingAdapter = new FakeWebXrAdapter();
    missingAdapter.availability = { supported: false, message: "WebXR is unavailable in this browser" };
    missingAdapter.requestResult = { ok: false, message: "WebXR is unavailable in this browser" };
    const missing = createBackend(missingAdapter);
    expect(await missing.checkAvailability()).toEqual(missingAdapter.availability);
    expect(await missing.requestSessionFromUserGesture()).toEqual(missingAdapter.requestResult);

    const rejectedAdapter = new FakeWebXrAdapter();
    rejectedAdapter.requestResult = { ok: false, message: "WebXR session request failed: permission denied" };
    const rejected = createBackend(rejectedAdapter);
    expect(await rejected.checkAvailability()).toEqual({ supported: true, message: "WebXR immersive-vr is supported" });
    expect(await rejected.requestSessionFromUserGesture()).toEqual(rejectedAdapter.requestResult);
  });

  it("ends a session granted after its pending request was canceled", async () => {
    const adapter = new FakeWebXrAdapter();
    const pendingRequest = deferred<WebXrSessionRequest>();
    adapter.pendingRequest = pendingRequest.promise;
    const backend = createBackend(adapter);
    const request = backend.requestSessionFromUserGesture();
    await backend.cancelPendingRequest();
    pendingRequest.resolve({ ok: true });
    expect(await request).toEqual({ ok: false, message: "WebXR session request was canceled" });
    expect(adapter.endCallCount).toBe(2);
  });
});

class FakeWebXrAdapter implements WebXrSessionPort {
  availability: WebXrAvailability = { supported: true, message: "WebXR immersive-vr is supported" };
  requestResult: WebXrSessionRequest = { ok: true };
  requestCount = 0;
  endCount = 0;
  endCallCount = 0;
  pendingRequest: Promise<WebXrSessionRequest> | null = null;
  active = false;
  private endHandler: (() => void) | null = null;
  private referenceSpaceResetHandler: ((previousReferenceFromNew: Pose | null) => void) | null = null;

  transformTrackingPose(value: Pose): Pose { return value; }

  checkAvailability(): Promise<WebXrAvailability> {
    return Promise.resolve(this.availability);
  }

  requestSessionFromUserGesture(): Promise<WebXrSessionRequest> {
    this.requestCount++;
    return this.pendingRequest ?? Promise.resolve(this.requestResult);
  }

  startSession(): Promise<void> {
    if (!this.requestResult.ok) return Promise.reject(new Error("No pending session"));
    this.active = true;
    return Promise.resolve();
  }

  endSession(): Promise<void> {
    this.endCallCount++;
    if (this.active) this.endCount++;
    this.active = false;
    return Promise.resolve();
  }

  setSessionEndHandler(handler: (() => void) | null): void {
    this.endHandler = handler;
  }

  setReferenceSpaceResetHandler(handler: ((previousReferenceFromNew: Pose | null) => void) | null): void {
    this.referenceSpaceResetHandler = handler;
  }

  emitUnexpectedEnd(): void {
    this.active = false;
    this.endHandler?.();
  }

  emitReferenceSpaceReset(previousReferenceFromNew: Pose | null): void {
    this.referenceSpaceResetHandler?.(previousReferenceFromNew);
  }
}

class FakeRenderer implements RendererAdapter {
  beginViewFrame(): void {}
  selectHandler: ((ray: SelectRay) => void) | null = null;

  startLoop(): void {}
  stopLoop(): void {}
  render(): void {}
  setFlightPose(): void {}
  setLakeVisualCondition(): void {}
  setFlightCameraMode(): void {}
  setCinematicCameraView(): void {}
  transformTrackingPose(pose: Parameters<RendererAdapter["transformTrackingPose"]>[0]): typeof pose { return pose; }
  resize(): void {}
  setStereoPresentation(): void {}
  setSelectRayHandler(handler: ((ray: SelectRay) => void) | null): void { this.selectHandler = handler; }
  dispose(): void {}
}

function createBackend(adapter: FakeWebXrAdapter): WebXrPresentationBackend {
  return new WebXrPresentationBackend(adapter, new FakeRenderer(), viewport, () => undefined, () => undefined);
}

function viewport(): ViewportSize {
  return Object.freeze({ x: 1280, y: 800, pixelRatio: 1 });
}

function deferred<T>(): { readonly promise: Promise<T>; readonly resolve: (value: T) => void } {
  let resolvePromise!: (value: T) => void;
  const promise = new Promise<T>((resolve) => { resolvePromise = resolve; });
  return Object.freeze({ promise, resolve: (value: T) => { resolvePromise(value); } });
}
