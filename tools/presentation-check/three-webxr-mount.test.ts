import "./lake-model-fixture.js";
import { panelFrameCursor, projectHeadPoint } from "./viewer-fixture.js";
import { fixtureBackendFrame, fixturePresentation, fixtureSemanticAction } from "./menu-fixture.js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Euler, Matrix4, Quaternion, Vector2, Vector3 } from "three";
import type { Camera, Color, Scene, WebGLRenderer } from "three";
import { WebXRManager } from "three/src/renderers/webxr/WebXRManager.js";
import { createThreeRenderer, titleScreenCameraPoseForViewport } from "../../web/src/render/engines/three/three-renderer.js";
import type { ThreeRendererBundle } from "../../web/src/render/engines/three/three-renderer.js";
import { flightRelativePose } from "../../web/src/render/camera/spatial.js";
import { pilotEyePoseRender, registeredPilotEyePoint } from "../../web/src/render/camera/spatial.js";
import { composePose, IDENTITY_POSE, inversePose, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import type { UiAction, UiViewModel } from "../../web/src/render/contracts/ui.js";
import { createSceneFixture } from "./ui-adapter-fixture.js";
import { WebXrPresentationBackend } from "../../web/src/presentation/webxr-backend.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import { flightFrameView, menuFrameFailureRecovery } from "../../web/src/app/flight-frame-view.js";
import { currentFlightPoseFixture } from "../game-check/current-session-fixture.js";
import { currentGameSessionProjectionFixture, currentSharedGameUiFixture } from "../game-check/shared-game-ui-fixture.js";

import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { prepareHeadHudFixture } from "./hud-canvas-fixture.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

interface RecordedDraw {
  readonly pointers: readonly Matrix4[];
  readonly eyes: readonly Matrix4[];
  readonly camera: Matrix4;
  readonly panel: Matrix4 | null;
  readonly headHud: Matrix4 | null;
  readonly projections: readonly Matrix4[];
  readonly headDepth: readonly boolean[];
}

interface RecordingDriver {
  readonly xr: WebXRManager;
  readonly draws: RecordedDraw[];
  readonly normalFrames: BrowserWindowFrames;
  getPixelRatio(): number;
  getSize(target: Vector2): Vector2;
}

interface BrowserAnimation {
  start(): void;
  stop(): void;
  setAnimationLoop(callback: ((timestamp: number) => void) | null): void;
  setContext(context: BrowserWindowFrames): void;
}

const capture = vi.hoisted(() => ({ driver: null as RecordingDriver | null }));

vi.mock("../../web/src/render/engines/three/lake-venue-mesh.js", async () => {
  const { Group } = await import("three");
  return { createLakeVenue: () => ({ group: new Group(), dispose: () => undefined }) };
});

vi.mock("three", async (importOriginal) => {
  const actual = await importOriginal<typeof import("three")>();
  const { WebGLAnimation } = await vi.importActual<{ WebGLAnimation: () => BrowserAnimation }>("three/src/renderers/webgl/WebGLAnimation.js");
  class RecordingWebGlRenderer {
    readonly xr: WebXRManager;
    readonly draws: RecordedDraw[] = [];
    readonly normalFrames = new BrowserWindowFrames();
    readonly shadowMap = { enabled: false };
    outputColorSpace = actual.SRGBColorSpace;
    autoClear = true;
    private readonly size = new actual.Vector2(1280, 720);
    private readonly clearColor = new actual.Color();
    private clearAlpha = 1;
    private pixelRatio = 1;
    private target: unknown = null;
    private readonly animation: BrowserAnimation;

    constructor() {
      const gl = { getContextAttributes: () => ({ xrCompatible: true, antialias: false, depth: false, stencil: false }) };
      this.xr = new WebXRManager(this as unknown as WebGLRenderer, gl as unknown as WebGLRenderingContext);
      this.animation = WebGLAnimation();
      this.animation.setContext(this.normalFrames);
      this.xr.addEventListener("sessionstart", () => { this.animation.stop(); });
      this.xr.addEventListener("sessionend", () => { this.animation.start(); });
      capture.driver = this;
    }
    setClearColor(color: Color | number, alpha: number): void { this.clearColor.set(color); this.clearAlpha = alpha; }
    getClearColor(target: Color): Color { return target.copy(this.clearColor); }
    getClearAlpha(): number { return this.clearAlpha; }
    setPixelRatio(value: number): void { this.pixelRatio = value; }
    getPixelRatio(): number { return this.pixelRatio; }
    setSize(width: number, height: number): void { this.size.set(width, height); }
    getSize(target: Vector2): Vector2 { return target.copy(this.size); }
    setViewport(): void {}
    setScissor(): void {}
    setScissorTest(): void {}
    setRenderTarget(target: unknown): void { this.target = target; }
    getRenderTarget(): unknown { return this.target; }
    setRenderTargetFramebuffer(): void {}
    setAnimationLoop(callback: XRFrameRequestCallback | null): void {
      this.xr.setAnimationLoop(callback);
      this.animation.setAnimationLoop(callback === null ? null : (timestamp) => { callback(timestamp, undefined as unknown as XRFrame); });
      if (callback === null) this.animation.stop();
      else this.animation.start();
    }
    clear(): void {}
    dispose(): void { this.animation.stop(); }
    render(scene: Scene, camera: Camera): void {
      if (!this.xr.enabled && this.target !== null) return;
      scene.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);
      if (!(camera instanceof actual.PerspectiveCamera)) throw new Error("Expected perspective camera");
      const presenting = this.xr.enabled && this.xr.isPresenting;
      if (presenting) this.xr.updateCamera(camera);
      let panel: Matrix4 | null = null;
      let headHud: Matrix4 | null = null;
      let headDepth: readonly boolean[] = [];
      const pointers: Matrix4[] = [];
      scene.traverse((object) => {
        if (object instanceof actual.Mesh && object.geometry instanceof actual.CircleGeometry && object.visible && object.parent?.visible) pointers.push(object.matrixWorld.clone());
        if (object instanceof actual.Mesh && object.material instanceof actual.MeshBasicMaterial &&
            object.material.map instanceof actual.CanvasTexture && object.visible) {
          if (object.name === "head-hud") {
            headHud = object.matrixWorld.clone();
            headDepth = [object.material.depthTest, object.material.depthWrite, object.layers.isEnabled(1)];
            expect(object.renderOrder).toBe(1000);
          } else panel = object.matrixWorld.clone();
        }
      });
      this.draws.push({
        eyes: presenting ? this.xr.getCamera().cameras.map((eye) => eye.matrixWorld.clone()) : [],
        projections: presenting ? this.xr.getCamera().cameras.map((eye) => eye.projectionMatrix.clone()) : [],
        camera: camera.matrixWorld.clone(), panel, headHud, headDepth, pointers
      });
    }
  }
  return { ...actual, WebGLRenderer: RecordingWebGlRenderer };
});

class BrowserXrLayer {
  readonly framebuffer = {};
  readonly framebufferWidth = 1280;
  readonly framebufferHeight = 720;
  readonly ignoreDepthValues = true;
  fixedFoveation = 0;
  getViewport(view: XRView): XRViewport {
    return { x: view.eye === "left" ? 0 : 640, y: 0, width: 640, height: 720 };
  }
}

class BrowserWindowFrames {
  private callback: FrameRequestCallback | null = null;
  private requestId = 0;

  requestAnimationFrame(callback: FrameRequestCallback): number { this.callback = callback; return ++this.requestId; }
  cancelAnimationFrame(): void { this.callback = null; }
  run(timestamp: number): void {
    const callback = this.callback;
    if (callback === null) throw new Error("Window frame callback was not scheduled");
    this.callback = null;
    callback(timestamp);
  }
}

class BrowserXrSession extends EventTarget {
  readonly referenceSpace = new EventTarget();
  readonly inputSources: XRInputSource[] = [];
  visibilityState: XRVisibilityState = "visible";
  readonly targetPoses = new Map<XRSpace, Pose | null>();
  readonly enabledFeatures: readonly string[] = [];
  readonly environmentBlendMode = "opaque";
  readonly renderState: XRRenderStateInit = {};
  private callback: XRFrameRequestCallback | null = null;
  private requestId = 0;
  poseQueries = 0;
  predictionLeadMs = 0;
  eyeGeometry: readonly { readonly pose: Pose; readonly projection: Matrix4 }[] | null = null;

  requestAnimationFrame(callback: XRFrameRequestCallback): number { this.callback = callback; return ++this.requestId; }
  cancelAnimationFrame(): void { this.callback = null; }
  requestReferenceSpace(): Promise<XRReferenceSpace> { return Promise.resolve(this.referenceSpace as XRReferenceSpace); }
  updateRenderState(state: XRRenderStateInit): void { Object.assign(this.renderState, state); }
  end(): Promise<void> { this.dispatchEvent(new Event("end")); return Promise.resolve(); }
  frame(timestamp: number, head: Pose | null, target: Pose | null = head): XRFrame {
    const geometry = this.eyeGeometry ?? [-0.033, 0.033].map((offset) => ({
      pose: pose(vec3(offset, 0, 0), IDENTITY_POSE.orientation),
      projection: new Matrix4().makePerspective(-0.05, 0.05, 0.05, -0.05, 0.05, 100_000)
    }));
    return {
      predictedDisplayTime: timestamp + this.predictionLeadMs,
      getViewerPose: () => head === null ? null : {
        transform: rigidTransform(head),
        views: geometry.map((eye, index) => ({
          eye: index === 0 ? "left" : "right", projectionMatrix: new Float32Array(eye.projection.elements),
          transform: rigidTransform(composePose(head, eye.pose))
        }))
      },
      getPose: (space: XRSpace) => {
        this.poseQueries++;
        const selected = this.targetPoses.has(space) ? this.targetPoses.get(space) ?? null : target;
        return selected === null ? null : { transform: rigidTransform(selected) };
      }
    } as unknown as XRFrame;
  }
  run(timestamp: number, head: Pose | null): void {
    const callback = this.callback;
    if (callback === null) throw new Error("XR frame callback was not scheduled");
    callback(timestamp, this.frame(timestamp, head));
  }
  select(timestamp: number, target: Pose | null, source?: XRInputSource): void {
    const event = new Event("select");
    Object.assign(event, { frame: this.frame(timestamp, target), inputSource: source ?? { targetRaySpace: {}, targetRayMode: "gaze" } });
    const clock = vi.spyOn(performance, "now").mockReturnValue(timestamp);
    try { this.dispatchEvent(event); } finally { clock.mockRestore(); }
  }
  reset(previousReferenceFromNew: Pose): void {
    const event = new Event("reset");
    Object.assign(event, { transform: rigidTransform(previousReferenceFromNew) });
    this.referenceSpace.dispatchEvent(event);
  }
}

describe("Three adapter mount through real WebXRManager and browser XR frames", () => {
  let bundle: ThreeRendererBundle;
  let driver: RecordingDriver;
  let session: BrowserXrSession;
  let backend: WebXrPresentationBackend;
  let view: UiViewModel;
  let latestFrame: BackendFrame | null;
  let actions: UiAction[];
  let onUiAction: ((action: UiAction) => void) | null;
  const viewport = { x: 1280, y: 720, pixelRatio: 1 };
  const head = pose(vec3(0.06, 0.08, -0.05), quaternion(Math.cos(0.13), 0, Math.sin(0.13), 0));
  const flight: FlightRenderPose = { ...currentFlightPoseFixture(),
    datumPositionNed: { north: 100, east: 25, down: -8 },
    attitudeBodyToNed: { ...quaternion(Math.cos(0.17), Math.sin(0.17), 0, 0) },
    pilotPositionMeters: 0.12, initialPilotPositionMeters: 0.12
  };

  beforeEach(async () => {
    vi.stubGlobal("XRWebGLLayer", BrowserXrLayer);
    session = new BrowserXrSession();
    const system = { requestSession: () => Promise.resolve(session), isSessionSupported: () => Promise.resolve(true) } as unknown as XRSystem;
    const canvas = { width: 1280, height: 720 } as HTMLCanvasElement;
    bundle = createThreeRenderer(canvas, canvas, system, 60, "low", canvas);
    if (capture.driver === null) throw new Error("Recording driver not constructed");
    driver = capture.driver;
    actions = [];
    onUiAction = null;
    view = { ...createSceneFixture("Flight"), panels: [] };
    latestFrame = null;
    backend = new WebXrPresentationBackend(bundle.webxr, bundle.renderer, () => viewport,
      (action) => { const semantic = fixtureSemanticAction(action); actions.push(semantic); onUiAction?.(semantic); }, () => undefined);
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.beginViewFrame();
      latestFrame = fixtureBackendFrame(backend, timestamp, view, viewer);
      bundle.renderer.render(latestFrame);
    });
    await backend.requestSessionFromUserGesture();
    await backend.start();
    bundle.renderer.setFlightPose(flight);
  });

  afterEach(async () => {
    await backend.stop();
    bundle.renderer.stopLoop();
    bundle.renderer.dispose();
    vi.unstubAllGlobals();
  });

  it("keeps the shared Head surface fixed in canted/asymmetric eyes through body, head and PilotEye motion", async () => {
    const eyeRotation = (angle: number) => new Quaternion().setFromEuler(new Euler(angle / 3, angle, angle / 2));
    const leftRotation = eyeRotation(0.04);
    const rightRotation = eyeRotation(-0.06);
    const leftProjection = new Matrix4().makePerspective(-0.045, 0.052, 0.045, -0.048, 0.05, 100_000);
    const rightProjection = new Matrix4().makePerspective(-0.048, 0.044, 0.048, -0.046, 0.05, 100_000);
    leftProjection.elements[4] = 0.025;
    session.eyeGeometry = [
      { pose: pose(vec3(-0.035, 0.002, 0.001), quaternion(leftRotation.w, leftRotation.x, leftRotation.y, leftRotation.z)), projection: leftProjection },
      { pose: pose(vec3(0.032, -0.001, 0.002), quaternion(rightRotation.w, rightRotation.x, rightRotation.y, rightRotation.z)), projection: rightProjection }
    ];
    let currentFlight = flight;
    const evidence = { viewer: null as ViewerFrame | null, view: null as UiViewModel | null };
    const runtime = new PresentationRuntime(bundle.renderer, [{
      mode: "webxr", start: () => Promise.resolve(), stop: () => Promise.resolve(),
      currentFrame: (timestamp, projectedView, rawHead, menu) => backend.currentFrame(timestamp, projectedView, rawHead, menu)
    }], (viewer) => {
      evidence.viewer = viewer;
      const gameSession = currentGameSessionProjectionFixture("flight_running");
      const initial = createInitialAppModel();
      const model = { ...initial, gameSession, presentation: { type: "ready", mode: "webxr" } as const,
        sharedGameUi: currentSharedGameUiFixture("flight_running", { informationCode: 0, platform: { presentation_mode: "hmd" } }) };
      evidence.view = flightFrameView(model, prepareHeadHudFixture(viewer, { informationCode: 0 }));
      return fixturePresentation(evidence.view);
    }, () => { bundle.renderer.setFlightPose(currentFlight); });
    bundle.renderer.stopLoop();
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    const referenceProjections: Vector3[][] = [];
    const worldProjections: Vector3[] = [];
    for (const [index, pilotPositionMeters] of [-0.1, 0.12, 0.4].entries()) {
      const bodyRotation = new Quaternion().setFromEuler(new Euler(index * 0.13, index * -0.24, index * 0.17));
      currentFlight = { ...flight, pilotPositionMeters, attitudeBodyToNed: { ...quaternion(bodyRotation.w, bodyRotation.x, bodyRotation.y, bodyRotation.z) } };
      const headRotation = new Quaternion().setFromEuler(new Euler(index * -0.16, index * 0.22, index * -0.11));
      const rawHead = pose(vec3(index * 0.05, index * 0.02, index * -0.03), quaternion(headRotation.w, headRotation.x, headRotation.y, headRotation.z));
      session.run(100 + index * 20, rawHead);
      const draw = lastDraw(driver);
      if (evidence.viewer === null || evidence.viewer.source !== "runtime-derived" || evidence.view === null || evidence.view.headHud.kind !== "visible" || draw.headHud === null) throw new Error("Missing same-frame Head evidence");
      const hud = evidence.view.headHud;
      const layerProjections: Vector3[] = [];
      for (const [eyeIndex, eye] of evidence.viewer.eyes.entries()) {
        const actualEye = draw.eyes[eyeIndex];
        const actualProjection = draw.projections[eyeIndex];
        if (actualEye === undefined || actualProjection === undefined) throw new Error("Missing rendered eye");
        actualProjection.elements.forEach((value, entry) => { expect(value).toBeCloseTo(eye.projection[entry] ?? Number.NaN, 6); });
        const eyeFromHud = actualEye.clone().invert().multiply(draw.headHud);
        for (const item of hud.items) {
          if (item.kind !== "plot") continue;
          const bounds = item.clip;
          const corners = [bounds.x, bounds.x + bounds.width].flatMap(horizontal =>
            [bounds.y, bounds.y + bounds.height].map(vertical =>
              new Vector3(horizontal / hud.size.width - 0.5, 0.5 - vertical / hud.size.height, 0).applyMatrix4(eyeFromHud)));
          const angles = corners.map(point => Math.atan2(point.y, -point.z) * 180 / Math.PI);
          expect(angles.every(angle => angle > 10) || angles.every(angle => angle < -10)).toBe(true);
        }
        for (const horizontal of [-0.5, 0.5]) for (const vertical of [-0.5, 0.5]) {
          const point = new Vector3(horizontal, vertical, 0).applyMatrix4(draw.headHud).applyMatrix4(actualEye.clone().invert()).applyMatrix4(actualProjection);
          const local = new Vector3(horizontal * hud.size.width, vertical * hud.size.height, 0).applyMatrix4(poseMatrix(hud.localPose));
          const expected = projectHeadPoint(eye, vec3(local.x, local.y, local.z));
          if (expected === null) throw new Error("Missing projection");
          expect(point.x).toBeCloseTo(expected.x, 5);
          expect(point.y).toBeCloseTo(expected.y, 5);
          expect(Math.abs(point.x)).toBeLessThan(1);
          expect(Math.abs(point.y)).toBeLessThan(1);
          layerProjections.push(point);
        }
      }
      referenceProjections.push(layerProjections);
      const leftEye = draw.eyes[0];
      const leftProjection = draw.projections[0];
      if (leftEye === undefined || leftProjection === undefined) throw new Error("Missing left eye");
      worldProjections.push(new Vector3(30, 10, -80).applyMatrix4(leftEye.clone().invert()).applyMatrix4(leftProjection));
      expect(draw.headDepth).toEqual([false, false, false]);
      expectMatrix(draw.headHud, poseMatrix(composePose(flightRelativePose(currentFlight, eyePose(currentFlight)), rawHead)).multiply(poseMatrix(hud.localPose)).scale(new Vector3(hud.size.width, hud.size.height, 1)));
    }
    const initialPoints = referenceProjections[0];
    if (initialPoints === undefined) throw new Error("Missing initial Head projection");
    for (const points of referenceProjections.slice(1)) points.forEach((point, index) => {
      const initialPoint = initialPoints[index];
      if (initialPoint === undefined) throw new Error("Missing corresponding Head point");
      expect(point.distanceTo(initialPoint)).toBeLessThan(1e-5);
    });
    const firstWorld = worldProjections[0];
    const lastWorld = worldProjections[2];
    if (firstWorld === undefined || lastWorld === undefined) throw new Error("Missing world projections");
    expect(firstWorld.distanceTo(lastWorld)).toBeGreaterThan(0.2);
  });

  it("stages a synchronous gaze Scene cut until the following rendered frame", () => {
    const mountedHead = composePose(eyePose(flight), head);
    view = centeredPanel("menu", mountedHead);
    onUiAction = (action) => {
      if (action.type !== "activate") return;
      bundle.renderer.setFlightPose(null);
      bundle.renderer.setFlightCameraMode("pilot");
    };
    session.run(100, head);
    expect(actions).toEqual([{ type: "focus", controlId: "mount-button" }]);
    expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(flight, mountedHead)));
    session.run(2200, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
    expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(flight, mountedHead)));
    session.run(2300, head);
    expectEyes(lastDraw(driver), poseMatrix(head));
  });

  it("uses only the last rendered mount for native select before an idle Scene cut is drawn", async () => {
    view = centeredPanel("head", composePose(eyePose(flight), head));
    session.select(100, head);
    expect(actions).toHaveLength(0);
    session.run(200, head);
    actions.length = 0;
    bundle.renderer.setFlightPose(null);
    session.select(10_000, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
    actions.length = 0;
    view = centeredPanel("head", head);
    session.run(10_100, head);
    expectEyes(lastDraw(driver), poseMatrix(head));
    session.select(20_000, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
    actions.length = 0;
    await backend.stop();
    actions.length = 0;
    session.select(30_000, head);
    expect(actions).toHaveLength(0);
  });

  it("keeps native select disabled after a stopped callback renders and until a restarted loop draws", () => {
    view = centeredPanel("head", composePose(eyePose(flight), head));
    session.run(100, head);
    bundle.renderer.stopLoop();
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.beginViewFrame();
      bundle.renderer.stopLoop();
      bundle.renderer.render(fixtureBackendFrame(backend, timestamp, view, viewer));
    });
    session.run(200, head);
    expect(lastDraw(driver).panel).not.toBeNull();
    actions.length = 0;
    session.select(10_000, head);
    expect(actions).toHaveLength(0);
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.beginViewFrame();
      bundle.renderer.render(fixtureBackendFrame(backend, timestamp, view, viewer));
    });
    session.select(11_000, head);
    expect(actions).toHaveLength(0);
    session.run(11_100, head);
    actions.length = 0;
    session.select(20_000, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
  });

  it("keeps pending input lifetime and latest-write-wins across frame exceptions and stop/start", () => {
    bundle.renderer.stopLoop();
    bundle.renderer.startLoop(() => {
      bundle.renderer.beginViewFrame();
      bundle.renderer.setFlightPose(null);
      throw new Error("fixture view failure");
    });
    expect(() => { session.run(100, head); }).toThrow("fixture view failure");
    const latestFlight = { ...flight, pilotPositionMeters: 0.4 };
    bundle.renderer.setFlightPose(latestFlight);
    bundle.renderer.stopLoop();
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.beginViewFrame();
      bundle.renderer.render(fixtureBackendFrame(backend, timestamp, view, viewer));
    });
    session.run(200, head);
    expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(latestFlight, composePose(eyePose(latestFlight), head))));
  });

  it("applies newer nonnull physics pose in the current frame despite staged optical inputs", () => {
    bundle.renderer.stopLoop();
    const updatedFlight = { ...flight, pilotPositionMeters: 0.4 };
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.setFlightCameraMode("pilot");
      bundle.renderer.setFlightPose(updatedFlight);
      bundle.renderer.beginViewFrame();
      bundle.renderer.render(fixtureBackendFrame(backend, timestamp, view, viewer));
    });
    session.run(100, head);
    expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(updatedFlight, composePose(eyePose(updatedFlight), head))));
  });

  it("clears failed Menu pick and Head before guarded after-frame Screen recovery", async () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    bundle.renderer.stopLoop();
    let model = { ...createInitialAppModel(), presentation: { type: "ready", mode: "webxr" } as const } as ReturnType<typeof createInitialAppModel>;
    onUiAction = (action) => { model = updateApp(model, { type: "ui-action", action }).model; };
    actions.length = 0;
    const runtime = new PresentationRuntime({ ...bundle.renderer, dispose() {} }, [{
      mode: "webxr", start: () => Promise.resolve(), stop: () => Promise.resolve(),
      currentFrame: (timestamp, value, viewer, menu) => backend.currentFrame(timestamp, value, viewer, menu)
    }], () => ({ viewModel: view, menu: { kind: "unavailable", reason: "context-unavailable" } }), () => undefined,
    (mode, reason) => {
      const frameModel = model;
      queueMicrotask(() => {
        const message = menuFrameFailureRecovery(model, frameModel, mode, reason);
        if (message !== null) model = updateApp(model, message).model;
      });
    });
    try {
      expect(await runtime.start("webxr")).toEqual({ ok: true });
      session.run(200, head);
      expect(lastDraw(driver).panel).toBeNull();
      expect(lastDraw(driver).headHud).toBeNull();
      expect(actions).toEqual([{ type: "focus", controlId: null }]);
      session.select(10_000, head);
      expect(actions).toEqual([{ type: "focus", controlId: null }]);
      expect(model.presentation).toEqual({ type: "ready", mode: "webxr" });
      await Promise.resolve();
      expect(model.presentation.type).toBe("transitioning");
    } finally { await runtime.dispose(); }
  });

  it("retains PilotEye, moving body offset, runtime head and IPD exactly once in both native eyes", async () => {
    for (const pilotPositionMeters of [-0.1, 0.12, 0.4]) {
      const currentFlight = { ...flight, pilotPositionMeters };
      bundle.renderer.setFlightPose(currentFlight);
      session.run(100, head);
      const mountedHead = composePose(eyePose(currentFlight), head);
      expectMatrix(poseMatrix(bundle.webxr.projectTrackingFrame(head).head), poseMatrix(mountedHead));
      expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(currentFlight, mountedHead)));
    }
    await backend.stop();
    bundle.renderer.render(screenFrame(viewport));
    expectMatrix(lastDraw(driver).camera, poseMatrix(flightRelativePose({ ...flight, pilotPositionMeters: 0.4 }, eyePose({ ...flight, pilotPositionMeters: 0.4 }))));
    expect(lastDraw(driver).eyes).toHaveLength(0);
  });

  it.each(["menu", "head", "cockpit", "world"] as const)("aligns native %s panel, gaze and controller rays in the same mounted frame", (anchor) => {
    const mountedHead = composePose(eyePose(flight), head);
    view = centeredPanel(anchor, anchor === "world" ? flightRelativePose(flight, mountedHead) : mountedHead);
    session.run(100, head);
    const expectedPanel = composePose(mountedHead, pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation));
    const panel = lastDraw(driver).panel;
    if (panel === null) throw new Error("Missing panel draw");
    expectMatrix(panel, poseMatrix(flightRelativePose(flight, expectedPanel)));
    expect(panelFrameCursor(currentFrame())?.point.x).toBeCloseTo(0, 8);
    expect(panelFrameCursor(currentFrame())?.point.y).toBeCloseTo(0, 8);
    session.run(2200, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
    actions.length = 0;
    session.select(10_000, head);
    expect(session.poseQueries).toBe(1);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
  });

  it.each(["menu", "cockpit"] as const)("preserves both eyes and mounted %s panels across runtime origin resets", (anchor) => {
    view = centeredPanel(anchor, composePose(eyePose(flight), head));
    session.run(100, head);
    const before = lastDraw(driver);
    if (before.panel === null) throw new Error("Missing panel before reset");
    const reset = pose(vec3(0.25, 0.1, -0.2), quaternion(Math.cos(0.19), 0, Math.sin(0.19), 0));
    session.reset(reset);
    const newHead = composePose(inversePose(reset), head);
    session.run(200, newHead);
    const after = lastDraw(driver);
    if (after.panel === null) throw new Error("Missing panel after reset");
    expectMatrix(after.camera, before.camera);
    expectMatrix(after.panel, before.panel);
    const beforeEye = before.eyes[0];
    const afterEye = after.eyes[0];
    if (beforeEye === undefined || afterEye === undefined) throw new Error("Missing native eyes around reset");
    expectMatrix(afterEye.clone().invert().multiply(after.panel), beforeEye.clone().invert().multiply(before.panel));
    expect(panelFrameCursor(currentFrame())?.point.x).toBeCloseTo(0, 8);
    session.select(10_000, newHead);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
  });

  it("preserves external Replay mounting without applying aircraft or PilotEye to native tracking", () => {
    const external = pose(vec3(50, 20, -130), quaternion(Math.cos(0.23), 0, Math.sin(0.23), 0));
    bundle.renderer.setFlightCameraMode("platform");
    bundle.renderer.setCinematicCameraView({ pose: external, verticalFieldOfViewDegrees: 60 });
    const mountedHead = composePose(external, head);
    view = centeredPanel("menu", mountedHead);
    session.run(100, head);
    expectEyes(lastDraw(driver), poseMatrix(mountedHead));
    const panel = lastDraw(driver).panel;
    if (panel === null) throw new Error("Missing Replay panel");
    expectMatrix(panel, poseMatrix(composePose(mountedHead, pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation))));
    session.select(10_000, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
    const before = lastDraw(driver);
    const reset = pose(vec3(-0.3, 0.1, 0.2), quaternion(Math.cos(0.18), 0, Math.sin(0.18), 0));
    session.reset(reset);
    actions.length = 0;
    const nextHead = composePose(inversePose(reset), head);
    session.select(11_000, nextHead);
    expect(actions).toHaveLength(0);
    session.run(11_100, nextHead);
    const after = lastDraw(driver);
    expectEyes(after, poseMatrix(mountedHead));
    if (after.panel === null || before.panel === null) throw new Error("Missing reset panels");
    expectMatrix(after.panel, before.panel);
    session.select(20_000, nextHead);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
  });

  it("keeps geographical World fixed while Cockpit follows moving and rolling aircraft", () => {
    const mountedHead = composePose(eyePose(flight), head);
    const local = composePose(mountedHead, pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation));
    const geographical = flightRelativePose(flight, local);
    const moved = { ...flight, datumPositionNed: { north: 170, east: -20, down: -13 },
      attitudeBodyToNed: { w: Math.cos(0.35), x: Math.sin(0.35), y: 0, z: 0 } };
    for (const [index, anchor] of (["world", "cockpit"] as const).entries()) {
      bundle.renderer.setFlightPose(flight);
      view = centeredPanel(anchor, anchor === "world" ? flightRelativePose(flight, mountedHead) : mountedHead);
      session.run(100 + index * 200, head);
      bundle.renderer.setFlightPose(moved);
      session.run(200 + index * 200, head);
      const panel = lastDraw(driver).panel;
      if (panel === null) throw new Error("Missing physical panel");
      expectMatrix(panel, poseMatrix(anchor === "world" ? geographical : flightRelativePose(moved, local)));
    }
  });

  it("keeps Menu fixed and permits off-centre gaze selection through float32-scale eye noise", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    const initial = lastDraw(driver).panel;
    if (initial === null) throw new Error("Missing initial Menu");
    const turned = composePose(head, pose(vec3(0.03, -0.02, 0), quaternion(Math.cos(0.025), 0, Math.sin(0.025), 0)));
    for (const [index, noise] of [-1e-5, -1e-7, 0, 1e-7, 1e-5].entries()) {
      session.eyeGeometry = [-0.033, 0.033].map(offset => {
        const projection = new Matrix4().makePerspective(-0.05, 0.05, 0.05, -0.05, 0.05, 100_000);
        projection.elements[0] = Math.fround(projection.elements[0] + noise);
        return { projection, pose: pose(vec3(Math.fround(offset + noise), noise, 0), quaternion(Math.cos(noise / 2), 0, Math.sin(noise / 2), 0)) };
      });
      session.run(200 + index * 100, turned);
      const panel = lastDraw(driver).panel;
      if (panel === null) throw new Error("Missing noisy Menu");
      expectMatrix(panel, initial);
      expect(Math.abs(panelFrameCursor(currentFrame())?.point.x ?? 0)).toBeGreaterThan(0.01);
    }
    session.run(2200, turned);
    expect(actions.filter(action => action.type === "activate" && action.controlId === "mount-button")).toHaveLength(1);
  });

  it("refits the retained menu when Pilot and external Replay use different reference bases", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    const external = pose(vec3(50, 20, -130), quaternion(Math.cos(0.23), 0, Math.sin(0.23), 0));
    bundle.renderer.setFlightCameraMode("platform");
    bundle.renderer.setCinematicCameraView({ pose: external, verticalFieldOfViewDegrees: 60 });
    session.run(200, head);
    expect(panelFrameCursor(currentFrame())?.point.x).toBeCloseTo(0, 8);
    expect(panelFrameCursor(currentFrame())?.point.y).toBeCloseTo(0, 8);
    const panel = lastDraw(driver).panel;
    if (panel === null) throw new Error("Missing external Menu");
    expectMatrix(panel, poseMatrix(composePose(composePose(external, head), pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation))));
    bundle.renderer.setFlightCameraMode("pilot");
    session.run(300, head);
    session.select(10_000, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
  });

  it("classifies invalid runtime-origin observations and discards stale selects before drawing", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    const notifications: (Pose | null)[] = [];
    bundle.webxr.setReferenceSpaceResetHandler(value => { notifications.push(value); });
    for (const invalid of [{ ...IDENTITY_POSE, position: { x: Number.NaN, y: 0, z: 0 } },
      { ...IDENTITY_POSE, orientation: { w: 0, x: 0, y: 0, z: 0 } },
      { ...IDENTITY_POSE, orientation: { w: 2, x: 0, y: 0, z: 0 } }]) {
      session.run(100, head);
      actions.length = 0;
      expect(() => { session.reset(invalid); }).not.toThrow();
      expect(notifications.at(-1)).toBeNull();
      session.select(10_000, head);
      expect(actions).toHaveLength(0);
    }
    expect(notifications).toHaveLength(3);
  });

  it("preserves XR compositor sizing and restores the retained Screen drawing ratio on exit", async () => {
    await backend.stop();
    bundle.renderer.resize(viewport);
    bundle.renderer.setScreenRenderScale(0.5);
    expect(driver.getPixelRatio()).toBe(0.5);
    await backend.requestSessionFromUserGesture();
    await backend.start();
    expect(driver.xr.isPresenting).toBe(true);
    expect(driver.getPixelRatio()).toBe(1);
    expect(driver.getSize(new Vector2()).toArray()).toEqual([1280, 720]);
    bundle.renderer.setScreenRenderScale(0.75);
    session.run(100, head);
    expect(driver.getPixelRatio()).toBe(1);
    expect(driver.getSize(new Vector2()).toArray()).toEqual([1280, 720]);
    await backend.stop();
    bundle.renderer.render(screenFrame(viewport));
    expect(driver.getPixelRatio()).toBe(0.75);
    expect(driver.getSize(new Vector2()).toArray()).toEqual([viewport.x, viewport.y]);
  });

  it("keeps native non-flight tracking unmounted and restores the Screen Title basis on session end", async () => {
    bundle.renderer.setFlightPose(null);
    view = centeredPanel("head", head);
    session.run(100, head);
    expectEyes(lastDraw(driver), poseMatrix(head));
    expectMatrix(poseMatrix(bundle.webxr.projectTrackingFrame(head).head), poseMatrix(head));
    await backend.stop();
    bundle.renderer.render(screenFrame(viewport));
    expectMatrix(lastDraw(driver).camera, poseMatrix(titleScreenCameraPoseForViewport(viewport.x, viewport.y, 60)));
    expect(lastDraw(driver).eyes).toHaveLength(0);
  });

  it("continues normal Window rAF frames through Runtime after WebXR ends with a retained reference space", async () => {
    bundle.renderer.stopLoop();
    await backend.stop();
    bundle.renderer.setFlightPose(null);
    session = new BrowserXrSession();
    const onFrameTimestamps: number[] = [];
    const viewTimestamps: number[] = [];
    const screen = new ScreenPresentationBackend(() => viewport);
    const runtime = new PresentationRuntime(bundle.renderer, [screen, backend],
      () => { viewTimestamps.push(onFrameTimestamps.at(-1) ?? Number.NaN); return fixturePresentation(view); },
      (timestamp) => { onFrameTimestamps.push(timestamp); });
    await backend.requestSessionFromUserGesture();
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    session.run(100, head);
    expect(lastDraw(driver).eyes).toHaveLength(2);
    const referenceSpace = driver.xr.getReferenceSpace();
    expect(referenceSpace).not.toBeNull();
    expect(await runtime.switchTo("screen")).toEqual({ ok: true });
    expect(driver.xr.isPresenting).toBe(false);
    expect(driver.xr.getSession()).toBeNull();
    expect(driver.xr.getReferenceSpace()).toBe(referenceSpace);
    view = createSceneFixture("Title");
    const drawCount = driver.draws.length;
    try {
      expect(() => { driver.normalFrames.run(200); }).not.toThrow();
      expect(() => { driver.normalFrames.run(300); }).not.toThrow();
      expect(onFrameTimestamps).toEqual([100, 200, 300]);
      expect(viewTimestamps).toEqual([100, 200, 300]);
      expect(driver.draws).toHaveLength(drawCount + 2);
      expect(runtime.currentMode).toBe("screen");
      expectMatrix(lastDraw(driver).camera, poseMatrix(titleScreenCameraPoseForViewport(viewport.x, viewport.y, 60)));
      expect(lastDraw(driver).eyes).toHaveLength(0);
      expect(lastDraw(driver).panel).toBeNull();
    } finally {
      await runtime.dispose();
    }
  });

  it("draws both tracked controllers at the same metric hit used by trigger under a nonuniform panel scale", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    const initial = view.panels[0];
    if (initial === undefined) throw new Error("Missing panel");
    view = { ...view, panels: [{ ...initial, size: { width: 1.4, height: 0.8 } }] };
    const left = controllerSource("left");
    const right = controllerSource("right");
    session.inputSources.push(left, right);
    session.targetPoses.set(left.targetRaySpace, composePose(head, pose(vec3(-0.1, 0, 0), IDENTITY_POSE.orientation)));
    session.targetPoses.set(right.targetRaySpace, composePose(head, pose(vec3(0.1, 0, 0), IDENTITY_POSE.orientation)));
    session.run(100, head);
    const panelFrame = currentFrame().panel;
    const draw = lastDraw(driver);
    if (panelFrame.kind !== "visible" || panelFrame.cursor === null || draw.panel === null) throw new Error("Missing pointer frame");
    const points = [panelFrame.cursor, ...(panelFrame.secondaryCursors ?? [])];
    expect(points).toHaveLength(2);
    expect(draw.pointers).toHaveLength(2);
    for (const [index, pointer] of draw.pointers.entries()) {
      const point = points[index];
      if (point === undefined) throw new Error("Missing pointer");
      const local = new Vector3().setFromMatrixPosition(pointer).applyMatrix4(draw.panel.clone().invert());
      expect(local.x).toBeCloseTo(point.point.x * 2.4 / 1.4, 6);
      expect(local.y).toBeCloseTo(point.point.y * 1.8 / 0.8, 6);
    }
    session.run(2100, head);
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    actions.length = 0;
    session.select(2200, null, right);
    expect(actions.filter((action) => action.type === "activate")).toEqual([{ type: "activate", controlId: "mount-button" }]);
    session.select(2200, null, left);
    expect(actions.filter((action) => action.type === "activate")).toHaveLength(1);
    session.inputSources.reverse();
    session.run(2300, head);
    const switched = panelFrameCursor(currentFrame());
    expect(switched?.point.x).toBeGreaterThan(0);
    session.inputSources.length = 0;
    actions.length = 0;
    session.select(2400, null, right);
    expect(actions).toHaveLength(0);
    session.run(2500, head);
    expect(currentFrame().panel.kind === "visible" && currentFrame().panel).toMatchObject({ secondaryCursors: [], cursor: { progress: 0 } });
    session.run(3500, head);
    expect(actions.filter((action) => action.type === "activate")).toHaveLength(1);
  });

  it("clears controller cursors on focus or pose loss and requires a fresh gaze dwell after restoration", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    const controller = controllerSource("right");
    session.inputSources.push(controller);
    session.targetPoses.set(controller.targetRaySpace, head);
    session.run(100, head);
    expect(panelFrameCursor(currentFrame())?.progress).toBe(1);
    session.visibilityState = "visible-blurred";
    session.select(200, head, controller);
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    session.run(300, head);
    expect(panelFrameCursor(currentFrame())).toBeNull();
    expect(lastDraw(driver).pointers).toHaveLength(0);
    session.targetPoses.set(controller.targetRaySpace, null);
    session.visibilityState = "visible";
    session.run(400, head);
    expect(panelFrameCursor(currentFrame())?.progress).toBe(0);
    session.select(500, null, controller);
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    session.run(1400, head);
    expect(actions.filter((action) => action.type === "activate")).toHaveLength(1);
  });

  it("restarts gaze dwell when input focus is lost and restored between XR animation frames", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    session.run(900, head);
    expect(panelFrameCursor(currentFrame())?.progress).toBeCloseTo(0.8);
    session.visibilityState = "visible-blurred";
    session.dispatchEvent(new Event("visibilitychange"));
    session.visibilityState = "visible";
    session.dispatchEvent(new Event("visibilitychange"));
    session.select(2000, head);
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    session.run(2100, head);
    expect(panelFrameCursor(currentFrame())?.progress).toBe(0);
    session.run(3100, head);
    expect(actions.filter((action) => action.type === "activate")).toHaveLength(1);
  });

  it("uses input receipt time instead of predicted display time for explicit gaze selection and dwell", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.predictionLeadMs = 50;
    session.run(100, head);
    session.select(110, head);
    session.run(120, head);
    session.run(1200, head);
    session.run(2400, head);
    expect(actions.filter((action) => action.type === "activate")).toHaveLength(1);
  });

  it("does not synthesize a valid gaze or controller pose when browser XR reports null", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    session.run(2200, null);
    expect(currentFrame().panel).toEqual({ kind: "unavailable", reason: "viewer-unavailable" });
    expect(lastDraw(driver).panel).toBeNull();
    expect(panelFrameCursor(currentFrame())).toBeNull();
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    actions.length = 0;
    session.select(10_000, null);
    expect(session.poseQueries).toBe(0);
    expect(actions).toHaveLength(0);
  });

  function currentFrame(): BackendFrame {
    if (latestFrame === null) throw new Error("No backend frame");
    return latestFrame;
  }
});

function eyePose(flight: FlightRenderPose): Pose {
  return pilotEyePoseRender(registeredPilotEyePoint(), flight.pilotPositionMeters, flight.initialPilotPositionMeters);
}

function controllerSource(handedness: XRHandedness): XRInputSource {
  return { handedness, targetRayMode: "tracked-pointer", targetRaySpace: {}, gripSpace: null, profiles: [] } as unknown as XRInputSource;
}

function centeredPanel(anchor: UiViewModel["panels"][number]["anchor"], mountedHead: Pose): UiViewModel {
  const fixture = createSceneFixture("Flight");
  const panel = fixture.panels[0];
  if (panel === undefined) throw new Error("Missing Flight panel");
  const forward = pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation);
  const localPose = anchor === "menu" ? IDENTITY_POSE : anchor === "head" ? forward : composePose(mountedHead, forward);
  return {
    ...fixture,
    panels: [{ ...panel, anchor, localPose, controls: [{
      id: "mount-button", kind: "button", label: "Mount", enabled: true,
      rect: { x: 0.25, y: 0.25, width: 0.5, height: 0.5 }
    }] }]
  };
}

function screenFrame(viewport: BackendFrame["viewport"]): BackendFrame {
  return { headHud: { kind: "absent" }, timestampMs: 0, cameraPose: IDENTITY_POSE, panel: { kind: "absent" }, viewport };
}

function rigidTransform(value: Pose): XRRigidTransform {
  return {
    position: { ...value.position, w: 1 },
    orientation: { x: value.orientation.x, y: value.orientation.y, z: value.orientation.z, w: value.orientation.w },
    matrix: new Float32Array(poseMatrix(value).elements)
  } as unknown as XRRigidTransform;
}

function poseMatrix(value: Pose): Matrix4 {
  return new Matrix4().compose(new Vector3(value.position.x, value.position.y, value.position.z),
    new Quaternion(value.orientation.x, value.orientation.y, value.orientation.z, value.orientation.w), new Vector3(1, 1, 1));
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  actual.elements.forEach((value, index) => { expect(value).toBeCloseTo(expected.elements[index] ?? Number.NaN, 5); });
}

function expectEyes(draw: RecordedDraw, center: Matrix4): void {
  expect(draw.eyes).toHaveLength(2);
  draw.eyes.forEach((eye, index) => {
    expectMatrix(eye, center.clone().multiply(new Matrix4().makeTranslation(index === 0 ? -0.033 : 0.033, 0, 0)));
  });
  const left = draw.eyes[0];
  const right = draw.eyes[1];
  if (left === undefined || right === undefined) throw new Error("Missing eye matrices");
  expect(new Vector3().setFromMatrixPosition(left).distanceTo(new Vector3().setFromMatrixPosition(right))).toBeCloseTo(0.066, 6);
}

function lastDraw(driver: RecordingDriver): RecordedDraw {
  const draw = driver.draws.at(-1);
  if (draw === undefined) throw new Error("No recorded draw");
  return draw;
}
