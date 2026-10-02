import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { Matrix4, Quaternion, Vector3 } from "three";
import type { Camera, Color, Scene, Vector2 } from "three";
import { createThreeRenderer } from "../../web/src/render/engines/three/three-renderer.js";
import type { ThreeRendererBundle } from "../../web/src/render/engines/three/three-renderer.js";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame } from "../../web/src/render/contracts/runtime.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { createBrowserPhoneVrSensorPort } from "../../web/src/presentation/phone-vr-browser.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";

interface RecordedDraw {
  readonly camera: Matrix4;
  readonly projection: Matrix4;
}

interface RecordingDriver {
  readonly draws: RecordedDraw[];
}

type DeviceAngles = readonly [alpha: number, beta: number, gamma: number];

const orientations: readonly {
  readonly screen: number;
  readonly neutral: DeviceAngles;
  readonly right: DeviceAngles;
  readonly left: DeviceAngles;
  readonly down: DeviceAngles;
  readonly up: DeviceAngles;
  readonly roll: DeviceAngles;
}[] = [
  { screen: 0, neutral: [0, 90, 0], right: [350, 90, 0], left: [10, 90, 0], down: [0, 80, 0], up: [0, 100, 0], roll: [90, 100, -90] },
  { screen: 90, neutral: [90, 0, -90], right: [80, 0, -90], left: [100, 0, -90], down: [90, 0, -80], up: [270, -180, 80], roll: [90, 10, -90] },
  { screen: -90, neutral: [90, -180, -90], right: [80, -180, -90], left: [100, -180, -90], down: [270, 0, 80], up: [90, -180, -80], roll: [90, -170, -90] },
  { screen: 180, neutral: [180, -90, 0], right: [170, -90, 0], left: [190, -90, 0], down: [180, -80, 0], up: [180, -100, 0], roll: [90, -80, -90] },
  { screen: 270, neutral: [90, -180, -90], right: [80, -180, -90], left: [100, -180, -90], down: [270, 0, 80], up: [90, -180, -80], roll: [90, -170, -90] }
];

const capture = vi.hoisted(() => ({ driver: null as RecordingDriver | null }));

vi.mock("three", async (importOriginal) => {
  const actual = await importOriginal<typeof import("three")>();
  class RecordingWebGlRenderer {
    readonly draws: RecordedDraw[] = [];
    readonly xr = { isPresenting: false, enabled: false };
    readonly shadowMap = { enabled: false };
    autoClear = true;
    private readonly size = new actual.Vector2();
    private readonly clearColor = new actual.Color();
    private clearAlpha = 1;
    private target: unknown = null;

    constructor() { capture.driver = this; }
    setClearColor(color: Color | number, alpha: number): void { this.clearColor.set(color); this.clearAlpha = alpha; }
    getClearColor(target: Color): Color { return target.copy(this.clearColor); }
    getClearAlpha(): number { return this.clearAlpha; }
    setPixelRatio(): void {}
    setSize(width: number, height: number): void { this.size.set(width, height); }
    getSize(target: Vector2): Vector2 { return target.copy(this.size); }
    setViewport(): void {}
    setScissor(): void {}
    setScissorTest(): void {}
    setRenderTarget(target: unknown): void { this.target = target; }
    getRenderTarget(): unknown { return this.target; }
    clear(): void {}
    dispose(): void {}
    render(scene: Scene, camera: Camera): void {
      if (this.target !== null) return;
      scene.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);
      this.draws.push({ camera: camera.matrixWorld.clone(), projection: camera.projectionMatrix.clone() });
    }
  }
  return { ...actual, WebGLRenderer: RecordingWebGlRenderer };
});

describe("Phone VR browser orientation through the Three adapter and StereoEffect", () => {
  let bundle: ThreeRendererBundle;
  let driver: RecordingDriver;
  const viewport = { x: 1280, y: 720, pixelRatio: 1 };
  const view = createSceneFixture("Title");

  beforeAll(() => {
    const canvas = { width: 1280, height: 720 } as HTMLCanvasElement;
    bundle = createThreeRenderer(canvas, canvas, null, "low");
    if (capture.driver === null) throw new Error("Recording driver was not constructed");
    driver = capture.driver;
  });

  beforeEach(() => {
    bundle.renderer.setStereoPresentation(null);
    bundle.renderer.setFlightPose({
      datumPositionNed: { north: 0, east: 0, down: -20 },
      attitudeBodyToNed: IDENTITY_POSE.orientation,
      pilotPositionMeters: 0,
      initialPilotPositionMeters: 0
    });
    driver.draws.length = 0;
  });

  afterAll(() => { bundle.renderer.dispose(); });

  function render(backend: PhoneVrPresentationBackend, timestamp = 100): { frame: BackendFrame; eyes: readonly RecordedDraw[] } {
    const frame = backend.currentFrame(timestamp, view);
    driver.draws.length = 0;
    bundle.renderer.render(frame);
    expect(driver.draws).toHaveLength(2);
    return { frame, eyes: [...driver.draws] };
  }

  async function start(screenAngle: number, angles: DeviceAngles) {
    const browser = createBrowser(screenAngle);
    const unavailable: string[] = [];
    const backend = new PhoneVrPresentationBackend(
      createBrowserPhoneVrSensorPort(browser.window), bundle.renderer, () => viewport, () => undefined,
      (message) => { unavailable.push(message); }, { nowMs: () => 1000 }
    );
    expect(await backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    const startup = backend.start();
    browser.emit(angles);
    await startup;
    return { browser, backend, unavailable };
  }

  it.each(orientations)("preserves yaw, pitch, and roll directions at screen $screen degrees", async (orientation) => {
    const { browser, backend, unavailable } = await start(orientation.screen, orientation.neutral);
    try {
      const initial = render(backend);
      const initialCenter = centerCamera(initial.eyes);
      const forwardPoint = new Vector3(0, 0, -10).applyMatrix4(initialCenter);
      const upperPoint = new Vector3(0, 1, -10).applyMatrix4(initialCenter);
      for (const movement of ["right", "left", "down", "up", "roll"] as const) {
        browser.emit(orientation[movement]);
        const current = render(backend);
        const relativeCamera = initialCenter.clone().invert().multiply(centerCamera(current.eyes));
        const angle = Math.PI / 18;
        const expected = movement === "right" ? new Matrix4().makeRotationY(-angle)
          : movement === "left" ? new Matrix4().makeRotationY(angle)
            : movement === "down" ? new Matrix4().makeRotationX(-angle)
              : movement === "up" ? new Matrix4().makeRotationX(angle)
                : new Matrix4().makeRotationZ(-angle);
        expectMatrix(relativeCamera, expected);
        const forward = new Vector3(0, 0, -1).transformDirection(relativeCamera);
        if (movement === "right") expect(forward.x).toBeGreaterThan(0.1);
        if (movement === "left") expect(forward.x).toBeLessThan(-0.1);
        if (movement === "down") expect(forward.y).toBeLessThan(-0.1);
        if (movement === "up") expect(forward.y).toBeGreaterThan(0.1);
        for (const [index, eye] of current.eyes.entries()) {
          const initialEye = initial.eyes[index];
          if (initialEye === undefined) throw new Error("Missing initial eye");
          const displacement = project(movement === "roll" ? upperPoint : forwardPoint, eye)
            .sub(project(movement === "roll" ? upperPoint : forwardPoint, initialEye));
          if (movement === "right") expect(displacement.x).toBeLessThan(-0.1);
          if (movement === "left") expect(displacement.x).toBeGreaterThan(0.1);
          if (movement === "down") expect(displacement.y).toBeGreaterThan(0.1);
          if (movement === "up") expect(displacement.y).toBeLessThan(-0.1);
          if (movement === "roll") expect(displacement.x).toBeLessThan(-0.01);
        }
        expect(current.frame.panelPose).toEqual(initial.frame.panelPose);
      }
      expect(unavailable).toEqual([]);
    } finally {
      await backend.stop();
      expect(browser.listenerCount()).toBe(0);
    }
  });

  it("preserves the displayed head pose when physical device roll and screen angle change together", async () => {
    const { browser, backend, unavailable } = await start(0, [0, 90, 0]);
    try {
      const initial = render(backend);
      for (const orientation of orientations) {
        browser.rotateScreen(orientation.screen);
        browser.emit(orientation.neutral);
        const rotated = render(backend);
        expectMatrix(centerCamera(rotated.eyes), centerCamera(initial.eyes));
        expectMatrix(poseMatrix(rotated.frame.cameraPose), new Matrix4());
        browser.emit(orientation.right);
        const turned = render(backend);
        expectMatrix(centerCamera(initial.eyes).invert().multiply(centerCamera(turned.eyes)), new Matrix4().makeRotationY(-Math.PI / 18));
      }
      expect(unavailable).toEqual([]);
    } finally { await backend.stop(); }
  });

  it.each(orientations)("keeps Menu and head recenter coherent at screen $screen degrees", async (orientation) => {
    const { browser, backend } = await start(orientation.screen, orientation.neutral);
    try {
      render(backend);
      browser.emit(orientation.right);
      const turned = render(backend);
      backend.recenterMenu();
      const movedMenu = render(backend);
      expectMatrix(centerCamera(movedMenu.eyes), centerCamera(turned.eyes));
      const menuRelativeToHead = poseMatrix(bundle.renderer.transformTrackingPose(movedMenu.frame.cameraPose)).invert().multiply(poseMatrix(movedMenu.frame.panelPose));
      expectMatrix(menuRelativeToHead, new Matrix4().makeTranslation(0, 0, -2.4));
      backend.recenterTracking();
      const reset = render(backend);
      expectMatrix(poseMatrix(reset.frame.cameraPose), new Matrix4());
      expectMatrix(poseMatrix(bundle.renderer.transformTrackingPose(reset.frame.cameraPose)).invert().multiply(poseMatrix(reset.frame.panelPose)), menuRelativeToHead);
      browser.emit(orientation.left);
      const later = render(backend);
      expectMatrix(centerCamera(reset.eyes).invert().multiply(centerCamera(later.eyes)), new Matrix4().makeRotationY(Math.PI / 9));
    } finally { await backend.stop(); }
  });
});

function poseMatrix(value: Pose): Matrix4 {
  return new Matrix4().compose(
    new Vector3(value.position.x, value.position.y, value.position.z),
    new Quaternion(value.orientation.x, value.orientation.y, value.orientation.z, value.orientation.w),
    new Vector3(1, 1, 1)
  );
}

function centerCamera(eyes: readonly RecordedDraw[]): Matrix4 {
  const first = eyes[0];
  const second = eyes[1];
  if (first === undefined || second === undefined) throw new Error("Both eyes are required");
  const position = new Vector3().setFromMatrixPosition(first.camera)
    .add(new Vector3().setFromMatrixPosition(second.camera)).multiplyScalar(0.5);
  return first.camera.clone().setPosition(position);
}

function project(point: Vector3, draw: RecordedDraw): Vector3 {
  return point.clone().applyMatrix4(draw.camera.clone().invert()).applyMatrix4(draw.projection);
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  for (const [index, value] of actual.elements.entries()) expect(value).toBeCloseTo(expected.elements[index] ?? Number.NaN, 10);
}

function createBrowser(initialScreenAngle: number) {
  type ReadingEvent = { readonly alpha: number; readonly beta: number; readonly gamma: number; readonly timeStamp: number };
  const orientationListeners = new Set<(event: ReadingEvent) => void>();
  const screenListeners = new Set<() => void>();
  let screenAngle = initialScreenAngle;
  let timeStamp = 0;
  return {
    window: {
      isSecureContext: true,
      DeviceOrientationEvent: { requestPermission: () => Promise.resolve("granted" as const) },
      screen: { orientation: {
        get angle() { return screenAngle; },
        addEventListener: (_type: string, listener: () => void) => { screenListeners.add(listener); },
        removeEventListener: (_type: string, listener: () => void) => { screenListeners.delete(listener); }
      } },
      addEventListener: (_type: string, listener: (event: ReadingEvent) => void) => { orientationListeners.add(listener); },
      removeEventListener: (_type: string, listener: (event: ReadingEvent) => void) => { orientationListeners.delete(listener); }
    },
    emit(angles: DeviceAngles): void {
      timeStamp++;
      for (const listener of orientationListeners) listener({ alpha: angles[0], beta: angles[1], gamma: angles[2], timeStamp });
    },
    rotateScreen(angle: number): void {
      screenAngle = angle;
      for (const listener of screenListeners) listener();
    },
    listenerCount(): number { return orientationListeners.size + screenListeners.size; }
  };
}
