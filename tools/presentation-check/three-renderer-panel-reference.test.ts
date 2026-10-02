import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { Matrix4, Quaternion, Vector3 } from "three";
import type { Camera, Color, Scene, Vector2 } from "three";
import { createThreeRenderer, titleScreenCameraPoseForViewport } from "../../web/src/render/engines/three/three-renderer.js";
import type { ThreeRendererBundle } from "../../web/src/render/engines/three/three-renderer.js";
import { flightRelativePose } from "../../web/src/render/engines/three/flight-pose.js";
import { pilotEyePoseThree, SYNTHETIC_PILOT_EYE_POINT } from "../../web/src/render/camera/pilot-eye-point.js";
import { composePose, IDENTITY_POSE, pose, quaternion, rotateVec3, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { placeMenuPanel } from "../../web/src/render/anchors.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import { PHONE_VR_OPTICAL_PROFILE } from "../../web/src/presentation/phone-vr-contracts.js";
import type { PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import { intersectPanel } from "../../web/src/presentation/panel-interaction.js";

interface RecordedDraw {
  readonly camera: Matrix4;
  readonly panel: Matrix4;
  readonly projection: Matrix4;
  readonly panelVisible: boolean;
  readonly transparent: boolean;
  readonly depthTest: boolean;
  readonly depthWrite: boolean;
  readonly renderOrder: number;
  readonly cursorDepthTest: boolean;
  readonly cursorDepthWrite: boolean;
  readonly cursorRenderOrder: number;
  readonly worldTransparentOrders: readonly number[];
  readonly groupOrders: readonly number[];
}

interface RecordingDriver {
  readonly draws: RecordedDraw[];
  readonly xr: { isPresenting: boolean; enabled: boolean };
}

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
      scene.traverse((object) => {
        if (object instanceof actual.Mesh && object.material instanceof actual.MeshBasicMaterial &&
            object.material.map instanceof actual.CanvasTexture) {
          const cursor = object.children.find((child) => child instanceof actual.Mesh && child.geometry instanceof actual.CircleGeometry);
          if (!(cursor instanceof actual.Mesh) || !(cursor.material instanceof actual.MeshBasicMaterial)) {
            throw new Error("Panel cursor is missing");
          }
          const worldTransparentOrders: number[] = [];
          const groupOrders: number[] = [];
          scene.traverse((entry) => {
            if (entry instanceof actual.Group) groupOrders.push(entry.renderOrder);
            if (entry instanceof actual.Mesh && entry !== object && entry !== cursor) {
              const materials = Array.isArray(entry.material) ? entry.material : [entry.material];
              if (materials.some((material: unknown) => material instanceof actual.Material && material.transparent)) {
                worldTransparentOrders.push(entry.renderOrder);
              }
            }
          });
          this.draws.push({
            camera: camera.matrixWorld.clone(), panel: object.matrixWorld.clone(),
            projection: camera.projectionMatrix.clone(), panelVisible: object.visible,
            transparent: object.material.transparent, depthTest: object.material.depthTest,
            depthWrite: object.material.depthWrite, renderOrder: object.renderOrder,
            cursorDepthTest: cursor.material.depthTest, cursorDepthWrite: cursor.material.depthWrite,
            cursorRenderOrder: cursor.renderOrder, worldTransparentOrders, groupOrders
          });
        }
      });
    }
  }
  return { ...actual, WebGLRenderer: RecordingWebGlRenderer };
});

describe("Three adapter panel reference with real StereoEffect", () => {
  let bundle: ThreeRendererBundle;
  let driver: RecordingDriver;
  const viewport = { x: 1280, y: 720, pixelRatio: 1 };
  const turnedHead = pose(vec3(0, 0, 0), quaternion(Math.cos(0.12), 0, Math.sin(0.12), 0));

  beforeAll(() => {
    const canvas = { width: 1280, height: 720 } as HTMLCanvasElement;
    bundle = createThreeRenderer(canvas, canvas, null, "low");
    if (capture.driver === null) throw new Error("Recording driver was not constructed");
    driver = capture.driver;
  });

  beforeEach(() => {
    driver.xr.isPresenting = false;
    bundle.renderer.setStereoPresentation(null);
    bundle.renderer.setFlightPose(null);
    bundle.renderer.setFlightCameraMode("pilot");
    bundle.renderer.setCinematicCameraView(null);
    driver.draws.length = 0;
  });

  afterAll(() => { bundle.renderer.dispose(); });

  it.each(["Boot", "Title", "FlightSetup", "Briefing", "Result"] as const)("keeps non-flight %s Menu visible in both eyes and stable across viewport changes", (scene) => {
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    const panel = createSceneFixture(scene).panels[0];
    if (panel === undefined) throw new Error("Missing panel fixture");
    const panelPose = placeMenuPanel(IDENTITY_POSE, 2.4);
    for (const size of [viewport, { x: 1920, y: 1080, pixelRatio: 1 }, { x: 720, y: 1280, pixelRatio: 1 }]) {
      driver.draws.length = 0;
      bundle.renderer.render(frame({ panel, panelPose, viewport: size }));
      expect(driver.draws).toHaveLength(2);
      const basis = poseMatrix(titleScreenCameraPoseForViewport(size.x, size.y));
      for (const [index, draw] of driver.draws.entries()) {
        const eye = new Matrix4().makeTranslation((index === 0 ? -1 : 1) * PHONE_VR_OPTICAL_PROFILE.eyeSeparationMeters / 2, 0, 0);
        expectMatrix(draw.camera, basis.clone().multiply(eye));
        expectMatrix(draw.panel, basis.clone().multiply(poseMatrix(panelPose)));
        expectMatrix(draw.camera.clone().invert().multiply(draw.panel), eye.clone().invert().multiply(poseMatrix(panelPose)));
        expect(draw.panelVisible).toBe(true);
        expectVisible(new Vector3(), draw);
        if (size.x > size.y) {
          for (const horizontal of [-1.2, 1.2]) for (const vertical of [-0.9, 0.9]) expectVisible(new Vector3(horizontal, vertical, 0), draw);
        }
      }
    }
  });

  it("keeps Head and Credits panels in the common Title basis without transforming tracking twice", () => {
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    const panel = createSceneFixture("Title", "Credits").panels[0];
    if (panel === undefined) throw new Error("Missing Credits panel");
    const panelPose = placeMenuPanel(turnedHead, 2.4);
    const headFrame = frame({ cameraPose: turnedHead, panelPose, panel: { ...panel, anchor: "head" } });
    expect(bundle.renderer.transformTrackingPose(turnedHead)).toBe(turnedHead);
    bundle.renderer.render(headFrame);
    const expectedCenter = poseMatrix(titleScreenCameraPoseForViewport(1280, 720)).multiply(poseMatrix(turnedHead));
    const expectedPanel = poseMatrix(titleScreenCameraPoseForViewport(1280, 720)).multiply(poseMatrix(panelPose));
    expect(driver.draws).toHaveLength(2);
    expectMatrix(centerEye(driver.draws), expectedCenter);
    for (const draw of driver.draws) {
      expectMatrix(draw.panel, expectedPanel);
      expectVisible(new Vector3(), draw);
    }
  });

  it("preserves real Phone VR gaze, menu recenter, and tracking recenter in their original reference", async () => {
    let emit: ((reading: PhoneVrSensorReading) => void) | null = null;
    const sensors: PhoneVrSensorPort = {
      checkAvailability: () => Promise.resolve({ supported: true, message: "Available" }),
      requestPermissionFromUserGesture: () => Promise.resolve({ ok: true }),
      getScreenOrientationAngle: () => 90,
      startListening: (reading) => { emit = reading; reading({ alpha: 0, beta: 90, gamma: 0, timestampMs: 0 }); },
      stopListening: () => { emit = null; }
    };
    const backend = new PhoneVrPresentationBackend(sensors, bundle.renderer, () => viewport, () => undefined,
      (message) => { throw new Error(message); }, { nowMs: () => 1000 });
    await backend.requestPermissionFromUserGesture();
    await backend.start();
    const view = createSceneFixture("Title");
    const render = (timestamp: number): { frame: BackendFrame; relative: Matrix4 } => {
      const current = backend.currentFrame(timestamp, view);
      driver.draws.length = 0;
      bundle.renderer.render(current);
      const draw = driver.draws[0];
      if (draw === undefined) throw new Error("No stereo draw");
      const relative = centerEye(driver.draws).invert().multiply(draw.panel);
      expectMatrix(relative, poseMatrix(current.cameraPose).invert().multiply(poseMatrix(current.panelPose)));
      const hit = intersectPanel({ origin: current.cameraPose.position, direction: rotateVec3(current.cameraPose.orientation, vec3(0, 0, -1)) }, current.panelPose);
      expect(hit).not.toBeNull();
      return { frame: current, relative };
    };
    try {
      render(0);
      const sendReading = emit as ((reading: PhoneVrSensorReading) => void) | null;
      if (sendReading === null) throw new Error("No sensor listener");
      sendReading({ alpha: 12, beta: 84, gamma: 0, timestampMs: 10 });
      const before = render(10);
      backend.recenterTracking();
      expectMatrix(render(11).relative, before.relative);
      backend.recenterMenu();
      expectMatrix(render(12).relative, poseMatrix(placeMenuPanel(IDENTITY_POSE, 2.4)));
    } finally {
      await backend.stop();
    }
  });

  it.each(["world", "cockpit"] as const)("retains the existing non-flight %s anchor transform", (anchor) => {
    const panel = createSceneFixture("Title").panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const current = frame({ panel: { ...panel, anchor } });
    bundle.renderer.render(current);
    expectMatrix(singleDraw(driver).panel, poseMatrix(current.panelPose));
  });

  it("preserves Screen composition and the native XR reference", () => {
    bundle.renderer.render(frame({ panelVisible: false }));
    expectMatrix(singleDraw(driver).camera, poseMatrix(titleScreenCameraPoseForViewport(1280, 720)));
    driver.draws.length = 0;
    driver.xr.isPresenting = true;
    const current = frame();
    bundle.renderer.render(current);
    expectMatrix(singleDraw(driver).camera, new Matrix4());
    expectMatrix(singleDraw(driver).panel, poseMatrix(current.panelPose));
  });

  it("preserves Pilot/Cockpit and external Replay transforms without an additional Title basis", () => {
    const flight: FlightRenderPose = {
      datumPositionNed: { north: 100, east: 25, down: -8 },
      attitudeBodyToNed: { w: Math.cos(0.1), x: Math.sin(0.1), y: 0, z: 0 },
      pilotPositionMeters: 0.15, initialPilotPositionMeters: 0
    };
    bundle.renderer.setFlightPose(flight);
    const current = frame({ cameraPose: turnedHead, panel: createSceneFixture("Flight").panels[0] ?? null });
    bundle.renderer.render(current);
    const eye = pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0.15, 0);
    expectMatrix(singleDraw(driver).camera, poseMatrix(flightRelativePose(flight, composePose(eye, turnedHead))));
    expectMatrix(singleDraw(driver).panel, poseMatrix(flightRelativePose(flight, current.panelPose)));
    const external = pose(vec3(100, 25, -40), turnedHead.orientation);
    bundle.renderer.setFlightCameraMode("platform");
    bundle.renderer.setCinematicCameraView({ pose: external, verticalFieldOfViewDegrees: 60 });
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    driver.draws.length = 0;
    const transformed = bundle.renderer.transformTrackingPose(turnedHead);
    expectMatrix(poseMatrix(transformed), poseMatrix(composePose(external, turnedHead)));
    const worldPanel = placeMenuPanel(transformed, 2.4);
    bundle.renderer.render(frame({ cameraPose: turnedHead, panelPose: worldPanel }));
    expectMatrix(centerEye(driver.draws), poseMatrix(transformed));
    for (const draw of driver.draws) expectMatrix(draw.panel, poseMatrix(worldPanel));
  });

  it("composites terminal Menu and Head panels after the world while restoring physical panel depth", () => {
    const flight: FlightRenderPose = {
      datumPositionNed: { north: 256.7, east: 0, down: -0.05 },
      attitudeBodyToNed: { w: Math.cos(0.04), x: Math.sin(0.04), y: 0, z: 0 },
      pilotPositionMeters: 0.15, initialPilotPositionMeters: 0
    };
    const basePanel = createSceneFixture("Result").panels[0];
    if (basePanel === undefined) throw new Error("Missing Result panel fixture");
    const panelPose = placeMenuPanel(IDENTITY_POSE, 2.4);
    const eye = pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0.15, 0);
    bundle.renderer.setFlightPose(flight);
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    for (const anchor of ["menu", "world", "head", "cockpit", "menu"] as const) {
      driver.draws.length = 0;
      bundle.renderer.render(frame({
        cameraPose: turnedHead, panelPose, panel: { ...basePanel, anchor },
        gazeCursor: { point: { x: 0.2, y: -0.3 }, progress: 0.5 }
      }));
      const overlay = anchor === "menu" || anchor === "head";
      expect(driver.draws).toHaveLength(2);
      expectMatrix(centerEye(driver.draws), poseMatrix(flightRelativePose(flight, composePose(eye, turnedHead))));
      for (const draw of driver.draws) {
        expectMatrix(draw.panel, poseMatrix(flightRelativePose(flight, panelPose)));
        expect(draw.panelVisible).toBe(true);
        expect(draw.transparent).toBe(overlay);
        expect(draw.depthTest).toBe(!overlay);
        expect(draw.depthWrite).toBe(!overlay);
        expect(draw.cursorDepthTest).toBe(!overlay);
        expect(draw.cursorDepthWrite).toBe(false);
        if (overlay) {
          expect(draw.renderOrder).toBeGreaterThan(1000);
          expect(draw.cursorRenderOrder).toBeGreaterThan(draw.renderOrder);
          expect(draw.worldTransparentOrders.length).toBeGreaterThan(0);
          expect(draw.worldTransparentOrders.every((order) => order < draw.renderOrder)).toBe(true);
          expect(draw.groupOrders.every((order) => order === 0)).toBe(true);
        } else {
          expect(draw.renderOrder).toBe(0);
          expect(draw.cursorRenderOrder).toBe(0);
        }
      }
    }
  });
});

function frame(overrides: Partial<BackendFrame> = {}): BackendFrame {
  return {
    timestampMs: 0, cameraPose: IDENTITY_POSE, panelPose: placeMenuPanel(IDENTITY_POSE, 2.4),
    panel: createSceneFixture("Title").panels[0] ?? null, panelVisible: true, gazeCursor: null,
    viewport: { x: 1280, y: 720, pixelRatio: 1 }, ...overrides
  };
}

function poseMatrix(value: Pose): Matrix4 {
  return new Matrix4().compose(new Vector3(value.position.x, value.position.y, value.position.z),
    new Quaternion(value.orientation.x, value.orientation.y, value.orientation.z, value.orientation.w), new Vector3(1, 1, 1));
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  actual.elements.forEach((value, index) => { expect(value).toBeCloseTo(expected.elements[index] ?? Number.NaN, 10); });
}

function expectVisible(point: Vector3, draw: RecordedDraw): void {
  const projected = point.applyMatrix4(draw.panel).applyMatrix4(draw.camera.clone().invert()).applyMatrix4(draw.projection);
  expect(Math.abs(projected.x)).toBeLessThan(1);
  expect(Math.abs(projected.y)).toBeLessThan(1);
  expect(Math.abs(projected.z)).toBeLessThan(1);
}

function singleDraw(driver: RecordingDriver): RecordedDraw {
  expect(driver.draws).toHaveLength(1);
  const draw = driver.draws[0];
  if (draw === undefined) throw new Error("No draw");
  return draw;
}

function centerEye(draws: readonly RecordedDraw[]): Matrix4 {
  expect(draws).toHaveLength(2);
  const left = draws[0]?.camera;
  const right = draws[1]?.camera;
  if (left === undefined || right === undefined) throw new Error("Missing stereo eye");
  const position = new Vector3().setFromMatrixPosition(left).add(new Vector3().setFromMatrixPosition(right)).multiplyScalar(0.5);
  return left.clone().setPosition(position);
}
