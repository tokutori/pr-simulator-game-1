import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Euler, Matrix4, Quaternion, Vector2, Vector3 } from "three";
import type { Camera, Color, Scene, WebGLRenderer } from "three";
import { WebXRManager } from "three/src/renderers/webxr/WebXRManager.js";
import { createThreeRenderer, titleScreenCameraPoseForViewport } from "../../web/src/render/engines/three/three-renderer.js";
import type { ThreeRendererBundle } from "../../web/src/render/engines/three/three-renderer.js";
import { flightRelativePose } from "../../web/src/render/engines/three/flight-pose.js";
import { pilotEyePoseThree, SYNTHETIC_PILOT_EYE_POINT } from "../../web/src/render/camera/pilot-eye-point.js";
import { composePose, IDENTITY_POSE, inversePose, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import type { AnchorKind } from "../../web/src/render/anchors.js";
import type { UiAction, UiViewModel } from "../../web/src/render/contracts/ui.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { WebXrPresentationBackend } from "../../web/src/presentation/webxr-backend.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import { createFlightFrameViewDraft, failFlightMenuFrame, finalizeFlightFrameView, flightMenuFailureRecovery } from "../../web/src/app/flight-frame-view.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { projectHeadPoint } from "../../web/src/render/contracts/viewer-frame.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { HEAD_HUD_MINIMUM_INK_HEIGHT_DEGREES, headHudCanvasSize, headHudPaintedTextInk, prepareHeadHudPaint, validateHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import type { HeadHudPaintPreparation } from "../../web/src/presentation/head-hud-canvas.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

interface RecordedDraw {
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
      scene.traverse((object) => {
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
        camera: camera.matrixWorld.clone(), panel, headHud, headDepth
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
  readonly inputSources: readonly XRInputSource[] = [];
  readonly enabledFeatures: readonly string[] = [];
  readonly environmentBlendMode = "opaque";
  readonly renderState: XRRenderStateInit = {};
  private callback: XRFrameRequestCallback | null = null;
  private requestId = 0;
  poseQueries = 0;
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
      predictedDisplayTime: timestamp,
      getViewerPose: () => head === null ? null : {
        transform: rigidTransform(head),
        views: geometry.map((eye, index) => ({
          eye: index === 0 ? "left" : "right", projectionMatrix: new Float32Array(eye.projection.elements),
          transform: rigidTransform(composePose(head, eye.pose))
        }))
      },
      getPose: () => { this.poseQueries++; return target === null ? null : { transform: rigidTransform(target) }; }
    } as unknown as XRFrame;
  }
  run(timestamp: number, head: Pose | null): void {
    const callback = this.callback;
    if (callback === null) throw new Error("XR frame callback was not scheduled");
    callback(timestamp, this.frame(timestamp, head));
  }
  select(timestamp: number, target: Pose | null): void {
    const event = new Event("select");
    Object.assign(event, { frame: this.frame(timestamp, target), inputSource: { targetRaySpace: {} } });
    this.dispatchEvent(event);
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
  const flight: FlightRenderPose = {
    datumPositionNed: { north: 100, east: 25, down: -8 },
    attitudeBodyToNed: quaternion(Math.cos(0.17), Math.sin(0.17), 0, 0),
    pilotPositionMeters: 0.12, initialPilotPositionMeters: 0.12
  };

  beforeEach(async () => {
    vi.stubGlobal("XRWebGLLayer", BrowserXrLayer);
    session = new BrowserXrSession();
    const system = { requestSession: () => Promise.resolve(session), isSessionSupported: () => Promise.resolve(true) } as unknown as XRSystem;
    const canvas = { width: 1280, height: 720 } as HTMLCanvasElement;
    bundle = createThreeRenderer(canvas, canvas, system, "low", canvas);
    if (capture.driver === null) throw new Error("Recording driver not constructed");
    driver = capture.driver;
    actions = [];
    onUiAction = null;
    view = { ...createSceneFixture("Flight"), panels: [] };
    latestFrame = null;
    backend = new WebXrPresentationBackend(bundle.webxr, bundle.renderer, () => viewport,
      (action) => { actions.push(action); onUiAction?.(action); }, () => undefined);
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.beginViewFrame();
      latestFrame = backend.currentFrame(timestamp, view, viewer.trackingFromHead);
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

  it.each([0, 1, 2, 3, 4, "flight-path-only"] as const)("matches canted/asymmetric eyes and keeps Information %s fixed through body, head and PilotEye motion", async (information) => {
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
    const evidence = { viewer: null as ViewerFrame | null, view: null as UiViewModel | null, paint: null as HeadHudPaintPreparation | null };
    const runtime = new PresentationRuntime(bundle.renderer, [{
      mode: "webxr", start: () => Promise.resolve(), stop: () => Promise.resolve(),
      currentFrame: (timestamp, projectedView, rawHead) => backend.currentFrame(timestamp, projectedView, rawHead)
    }], (viewer) => {
      evidence.viewer = viewer;
      const values = new Array<number>(33).fill(0);
      values[4] = 8;
      values[7] = 1;
      values[19] = -1;
      values[20] = 15;
      values[21] = 8;
      values[22] = 9;
      values[31] = 1;
      const snapshot = parseFlightSnapshot(values);
      const gameSession = gameSessionState(5, 0, snapshot, true);
      if (gameSession === null) throw new Error("Missing Flight session");
      const initial = createInitialAppModel();
      const model = { ...initial, gameSession, presentation: { type: "ready", mode: "webxr" } as const,
        difficulty: { ...initial.difficulty, informationCode: information === "flight-path-only" ? 4 : information,
          hudProfile: information === "flight-path-only"
            ? { telemetry: false, attitude: false, wind: false, flightPath: true, angleOfAttack: false, warnings: false }
            : initial.difficulty.hudProfile } };
      const draft = createFlightFrameViewDraft(model, snapshot, viewer);
      if (draft.headHud.kind !== "visible") throw new Error("Missing Head HUD");
      const dimensions = headHudCanvasSize(draft.headHud.layer);
      const preparation = validateHeadHudPaint(prepareHeadHudPaint(new HudCanvasFixture(), draft.headHud, dimensions.width, dimensions.height), viewer);
      if (preparation.kind !== "ready") throw new Error("Head preflight failed");
      evidence.view = finalizeFlightFrameView(draft, preparation.view);
      evidence.paint = preparation;
      return evidence.view;
    }, () => { bundle.renderer.setFlightPose(currentFlight); });
    bundle.renderer.stopLoop();
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    const referenceProjections: Vector3[][] = [];
    const worldProjections: Vector3[] = [];
    for (const [index, pilotPositionMeters] of [-0.1, 0.12, 0.4].entries()) {
      const bodyRotation = new Quaternion().setFromEuler(new Euler(index * 0.13, index * -0.24, index * 0.17));
      currentFlight = { ...flight, pilotPositionMeters, attitudeBodyToNed: quaternion(bodyRotation.w, bodyRotation.x, bodyRotation.y, bodyRotation.z) };
      const headRotation = new Quaternion().setFromEuler(new Euler(index * -0.16, index * 0.22, index * -0.11));
      const rawHead = pose(vec3(index * 0.05, index * 0.02, index * -0.03), quaternion(headRotation.w, headRotation.x, headRotation.y, headRotation.z));
      session.run(100 + index * 20, rawHead);
      const draw = lastDraw(driver);
      if (evidence.viewer === null || evidence.viewer.source !== "runtime-derived" || evidence.view === null || evidence.view.headHud.kind !== "visible" || draw.headHud === null) throw new Error("Missing same-frame Head evidence");
      const hud = evidence.view.headHud;
      if (information === "flight-path-only") {
        expect(hud.elements.map((element) => element.id)).toEqual(["head-flight-path"]);
      }
      const layerProjections: Vector3[] = [];
      for (const [eyeIndex, eye] of evidence.viewer.eyes.entries()) {
        const actualEye = draw.eyes[eyeIndex];
        const actualProjection = draw.projections[eyeIndex];
        if (actualEye === undefined || actualProjection === undefined) throw new Error("Missing rendered eye");
        actualProjection.elements.forEach((value, entry) => { expect(value).toBeCloseTo(eye.projection[entry] ?? Number.NaN, 6); });
        const eyeFromHud = actualEye.clone().invert().multiply(draw.headHud);
        for (const element of hud.elements) {
          const corners = [element.bounds.left, element.bounds.left + element.bounds.width].flatMap((horizontal) =>
            [element.bounds.top, element.bounds.top + element.bounds.height].map((vertical) =>
              new Vector3(horizontal - 0.5, 0.5 - vertical, 0).applyMatrix4(eyeFromHud)));
          const verticalAngles = corners.map((point) => Math.atan2(point.y, -point.z) * 180 / Math.PI);
          expect(verticalAngles.every((angle) => angle > 10) || verticalAngles.every((angle) => angle < -10)).toBe(true);
        }
        const paint = evidence.paint;
        if (paint?.kind !== "ready") throw new Error("Missing paint plan");
        for (const glyph of headHudPaintedTextInk(paint)) {
          if (glyph.value.trim() === "") continue;
          for (const horizontal of [glyph.left, glyph.left + glyph.width]) {
            const upper = new Vector3(horizontal / paint.width - 0.5, 0.5 - glyph.top / paint.height, 0).applyMatrix4(eyeFromHud);
            const lower = new Vector3(horizontal / paint.width - 0.5, 0.5 - (glyph.top + glyph.height) / paint.height, 0).applyMatrix4(eyeFromHud);
            expect(upper.angleTo(lower) * 180 / Math.PI).toBeGreaterThanOrEqual(HEAD_HUD_MINIMUM_INK_HEIGHT_DEGREES);
            for (const point of [upper, lower]) {
              point.applyMatrix4(actualProjection);
              expect(Math.abs(point.x)).toBeLessThan(1);
              expect(Math.abs(point.y)).toBeLessThan(1);
            }
          }
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

  it("keeps all 64 Custom cue combinations and all-off Pause access through actual native eye frames", async () => {
    const values = new Array<number>(33).fill(0);
    values[4] = 8; values[7] = 1; values[19] = -1; values[20] = 15; values[21] = 8; values[22] = 9; values[31] = 1;
    const snapshot = parseFlightSnapshot(values);
    const gameSession = gameSessionState(5, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing Flight session");
    const initial = createInitialAppModel();
    let mask = 0;
    let captured: UiViewModel | null = null;
    const runtime = new PresentationRuntime(bundle.renderer, [{
      mode: "webxr", start: () => Promise.resolve(), stop: () => Promise.resolve(),
      currentFrame: (timestamp, projectedView, rawHead) => backend.currentFrame(timestamp, projectedView, rawHead)
    }], (viewer) => {
      const model = { ...initial, gameSession, presentation: { type: "ready", mode: "webxr" } as const,
        difficulty: { ...initial.difficulty, informationCode: 4, hudProfile: {
          telemetry: (mask & 1) !== 0, attitude: (mask & 2) !== 0, wind: (mask & 4) !== 0,
          flightPath: (mask & 8) !== 0, angleOfAttack: (mask & 16) !== 0, warnings: (mask & 32) !== 0
        } } };
      const draft = createFlightFrameViewDraft(model, snapshot, viewer);
      if (mask === 0 || mask === 32) {
        expect(draft.headHud.kind).toBe("absent");
        captured = finalizeFlightFrameView(draft, draft.headHud);
      } else {
        if (draft.headHud.kind !== "visible") throw new Error(`Missing Custom layout ${String(mask)}`);
        const dimensions = headHudCanvasSize(draft.headHud.layer);
        const preparation = validateHeadHudPaint(prepareHeadHudPaint(new HudCanvasFixture(), draft.headHud, dimensions.width, dimensions.height), viewer);
        if (preparation.kind !== "ready") throw new Error(`Custom preflight failed ${String(mask)}`);
        captured = finalizeFlightFrameView(draft, preparation.view);
      }
      return captured;
    });
    bundle.renderer.stopLoop();
    expect(await runtime.start("webxr")).toEqual({ ok: true });
    for (mask = 0; mask < 64; mask++) {
      session.run(100 + mask * 20, head);
      const draw = lastDraw(driver);
      expect(draw.eyes).toHaveLength(2);
      const frameView = captured as UiViewModel | null;
      if (frameView === null) throw new Error("Missing Custom frame");
      expect(frameView.panels[0]?.controls.some((control) => control.id === "game-flight-pause" && control.enabled)).toBe(true);
      if (mask === 0 || mask === 32) expect(draw.headHud).toBeNull();
      else {
        if (frameView.headHud.kind !== "visible" || draw.headHud === null) throw new Error("Missing Custom draw");
        const ids = frameView.headHud.elements.map((element) => element.id);
        expect(ids.includes("head-attitude")).toBe((mask & 2) !== 0);
        const attitude = frameView.headHud.elements.find((element) => element.kind === "attitude");
        expect(ids.includes("head-flight-path")).toBe((mask & 8) !== 0 && (mask & 2) === 0);
        if (attitude?.kind === "attitude") expect(attitude.flightPathAngleDegrees !== null).toBe((mask & 8) !== 0);
        expect(draw.headDepth).toEqual([false, false, false]);
      }
    }
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
      bundle.renderer.render(backend.currentFrame(timestamp, view, viewer.trackingFromHead));
    });
    session.run(200, head);
    expect(lastDraw(driver).panel).not.toBeNull();
    actions.length = 0;
    session.select(10_000, head);
    expect(actions).toHaveLength(0);
    bundle.renderer.startLoop((timestamp, viewer) => {
      bundle.renderer.beginViewFrame();
      bundle.renderer.render(backend.currentFrame(timestamp, view, viewer.trackingFromHead));
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
      bundle.renderer.render(backend.currentFrame(timestamp, view, viewer.trackingFromHead));
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
      bundle.renderer.render(backend.currentFrame(timestamp, view, viewer.trackingFromHead));
    });
    session.run(100, head);
    expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(updatedFlight, composePose(eyePose(updatedFlight), head))));
  });

  it("clears failed Menu pick and Head before guarded after-frame Screen recovery", async () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    const values = new Array<number>(33).fill(0); values[7] = 1; values[19] = -1;
    const snapshot = parseFlightSnapshot(values);
    const gameSession = gameSessionState(5, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing Flight fixture");
    let model = { ...createInitialAppModel(), gameSession, presentation: { type: "ready", mode: "webxr" } as const } as ReturnType<typeof createInitialAppModel>;
    const draft = createFlightFrameViewDraft(model, snapshot, { source: "unavailable", reason: "invalid-view-geometry", trackingFromHead: head });
    const failure = failFlightMenuFrame(draft, view, "context-unavailable");
    onUiAction = (action) => { model = updateApp(model, { type: "ui-action", action }).model; };
    actions.length = 0;
    queueMicrotask(() => {
      const message = flightMenuFailureRecovery(model, failure);
      if (message !== null) model = updateApp(model, message).model;
    });
    view = failure.viewModel;
    session.run(200, head);
    expect(lastDraw(driver).panel).toBeNull();
    expect(lastDraw(driver).headHud).toBeNull();
    expect(actions).toEqual([{ type: "focus", controlId: null }]);
    session.select(10_000, head);
    expect(actions).toEqual([{ type: "focus", controlId: null }]);
    expect(model.presentation).toEqual({ type: "ready", mode: "webxr" });
    await Promise.resolve();
    expect(model.presentation.type).toBe("transitioning");
  });

  it("retains PilotEye, moving body offset, runtime head and IPD exactly once in both native eyes", async () => {
    for (const pilotPositionMeters of [-0.1, 0.12, 0.4]) {
      const currentFlight = { ...flight, pilotPositionMeters };
      bundle.renderer.setFlightPose(currentFlight);
      session.run(100, head);
      const mountedHead = composePose(eyePose(currentFlight), head);
      expectMatrix(poseMatrix(bundle.webxr.transformTrackingPose(head)), poseMatrix(mountedHead));
      expectEyes(lastDraw(driver), poseMatrix(flightRelativePose(currentFlight, mountedHead)));
    }
    await backend.stop();
    bundle.renderer.render(screenFrame(viewport));
    expectMatrix(lastDraw(driver).camera, poseMatrix(flightRelativePose({ ...flight, pilotPositionMeters: 0.4 }, eyePose({ ...flight, pilotPositionMeters: 0.4 }))));
    expect(lastDraw(driver).eyes).toHaveLength(0);
  });

  it.each(["menu", "head", "cockpit", "world"] as const)("aligns native %s panel, gaze and controller rays in the same mounted frame", (anchor) => {
    const mountedHead = composePose(eyePose(flight), head);
    view = centeredPanel(anchor, mountedHead);
    session.run(100, head);
    const expectedPanel = composePose(mountedHead, pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation));
    const panel = lastDraw(driver).panel;
    if (panel === null) throw new Error("Missing panel draw");
    expectMatrix(panel, poseMatrix(flightRelativePose(flight, expectedPanel)));
    expect(currentFrame().gazeCursor?.point.x).toBeCloseTo(0, 8);
    expect(currentFrame().gazeCursor?.point.y).toBeCloseTo(0, 8);
    session.run(2200, head);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
    actions.length = 0;
    session.select(10_000, head);
    expect(session.poseQueries).toBe(1);
    expect(actions).toContainEqual({ type: "activate", controlId: "mount-button" });
  });

  it.each(["menu", "cockpit"] as const)("conjugates rotation plus translation resets for mounted %s panels and selectors", (anchor) => {
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
    const beforeEye = before.eyes[0];
    const afterEye = after.eyes[0];
    if (beforeEye === undefined || afterEye === undefined) throw new Error("Missing native eyes around reset");
    expectMatrix(afterEye.clone().invert().multiply(after.panel), beforeEye.clone().invert().multiply(before.panel));
    expect(currentFrame().gazeCursor?.point.x).toBeCloseTo(0, 8);
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
  });

  it("keeps native non-flight tracking unmounted and restores the Screen Title basis on session end", async () => {
    bundle.renderer.setFlightPose(null);
    view = centeredPanel("head", head);
    session.run(100, head);
    expectEyes(lastDraw(driver), poseMatrix(head));
    expectMatrix(poseMatrix(bundle.webxr.transformTrackingPose(head)), poseMatrix(head));
    await backend.stop();
    bundle.renderer.render(screenFrame(viewport));
    expectMatrix(lastDraw(driver).camera, poseMatrix(titleScreenCameraPoseForViewport(viewport.x, viewport.y)));
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
      () => { viewTimestamps.push(onFrameTimestamps.at(-1) ?? Number.NaN); return view; },
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
      expectMatrix(lastDraw(driver).camera, poseMatrix(titleScreenCameraPoseForViewport(viewport.x, viewport.y)));
      expect(lastDraw(driver).eyes).toHaveLength(0);
      expect(lastDraw(driver).panel).toBeNull();
    } finally {
      await runtime.dispose();
    }
  });

  it("does not synthesize a valid gaze or controller pose when browser XR reports null", () => {
    view = centeredPanel("menu", composePose(eyePose(flight), head));
    session.run(100, head);
    session.run(2200, null);
    expect(currentFrame().gazeCursor).toBeNull();
    expect(actions.some((action) => action.type === "activate")).toBe(false);
    actions.length = 0;
    session.select(10_000, null);
    expect(session.poseQueries).toBe(1);
    expect(actions).toHaveLength(0);
  });

  function currentFrame(): BackendFrame {
    if (latestFrame === null) throw new Error("No backend frame");
    return latestFrame;
  }
});

function eyePose(flight: FlightRenderPose): Pose {
  return pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, flight.pilotPositionMeters, flight.initialPilotPositionMeters);
}

function centeredPanel(anchor: AnchorKind, mountedHead: Pose): UiViewModel {
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
  return { headHud: { kind: "absent" }, timestampMs: 0, cameraPose: IDENTITY_POSE, panelPose: IDENTITY_POSE, panel: null,
    panelVisible: false, gazeCursor: null, viewport };
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
