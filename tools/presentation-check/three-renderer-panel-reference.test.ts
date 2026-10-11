import { configuredViewerFixture, panelFrameCursor, visiblePanelFrame } from "./viewer-fixture.js";
import { readFileSync } from "node:fs";
import { fixtureBackendFrame, fixturePresentation, fixtureSemanticAction } from "./menu-fixture.js";
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { BufferGeometry, DataTexture, DirectionalLight, Fog, Matrix4, Mesh, Quaternion, ShaderMaterial, Vector3, Vector4 } from "three";
import type { Camera, Color, Object3D, Scene, Vector2 } from "three";
import { createThreeRenderer, titleScreenCameraPoseForViewport } from "../../web/src/render/engines/three/three-renderer.js";
import type { ThreeRendererBundle } from "../../web/src/render/engines/three/three-renderer.js";
import { flightRelativePose } from "../../web/src/render/engines/three/flight-pose.js";
import { pilotEyePoseThree, SYNTHETIC_PILOT_EYE_POINT } from "../../web/src/render/camera/pilot-eye-point.js";
import { composePose, IDENTITY_POSE, pose, quaternion, rotateVec3, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, FlightRenderPose, LakeWaterQualityCleanupResult } from "../../web/src/render/contracts/runtime.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { projectHeadPoint } from "../../web/src/render/contracts/viewer-frame.js";
import type { UiAction, UiViewModel, UiPanel } from "../../web/src/render/contracts/ui.js";
import type { AnchorKind } from "../../web/src/render/anchors.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { placeMenuPanel } from "../../web/src/render/anchors.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import { PHONE_VR_OPTICAL_PROFILE } from "../../web/src/presentation/phone-vr-contracts.js";
import type { PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import { hitTestControl, intersectPanel } from "../../web/src/presentation/panel-interaction.js";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createFlightFrameViewDraft, finalizeFlightFrameView } from "../../web/src/app/flight-frame-view.js";
import { currentFlightDisplayFixture } from "../game-check/current-session-fixture.js";
import { headHudCanvasSize, prepareHeadHudPaint, validateHeadHudPaint } from "../../web/src/presentation/head-hud-canvas.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { parseRuntimeEnvironmentSnapshot } from "../../web/src/game/runtime-environment.js";
import { projectRuntimeVenue } from "../../web/src/game/runtime-venue.js";
import * as lakeWater from "../../web/src/render/contracts/lake-water.js";
import * as lakeDetail from "../../web/src/render/engines/three/lake-detail-texture.js";
import type { LakeWaterQuality } from "../../web/src/render/contracts/lake-water.js";
import { createLakeWaveProjection, LAKE_WAVE_PROJECTION_GLSL } from "../../web/src/render/engines/three/lake-wave-projection.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

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
  readonly headHud: Matrix4 | null;
}

interface RecordingDriver {
  readonly draws: RecordedDraw[];
  readonly scene: Scene | null;
  readonly xr: { isPresenting: boolean; enabled: boolean };
  tick(timestamp: number, viewer: XRViewerPose | null): void;
}

const capture = vi.hoisted(() => ({ driver: null as RecordingDriver | null }));

vi.mock("three", async (importOriginal) => {
  const actual = await importOriginal<typeof import("three")>();
  class RecordingWebGlRenderer {
    readonly draws: RecordedDraw[] = [];
    scene: Scene | null = null;
    readonly xr = { isPresenting: false, enabled: false, getReferenceSpace: () => ({}) as XRReferenceSpace };
    readonly shadowMap = { enabled: false };
    autoClear = true;
    private readonly size = new actual.Vector2();
    private readonly clearColor = new actual.Color();
    private clearAlpha = 1;
    private target: unknown = null;
    private animationLoop: ((timestamp: number, frame: XRFrame) => void) | null = null;

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
    setAnimationLoop(callback: ((timestamp: number, frame: XRFrame) => void) | null): void { this.animationLoop = callback; }
    tick(timestamp: number, viewer: XRViewerPose | null): void {
      this.animationLoop?.(timestamp, { getViewerPose: () => viewer } as unknown as XRFrame);
    }
    dispose(): void {}
    render(scene: Scene, camera: Camera): void {
      if (this.target !== null) return;
      this.scene = scene;
      scene.updateMatrixWorld(true);
      camera.updateMatrixWorld(true);
      let headHud: Matrix4 | null = null;
      scene.traverse((object) => { if (object.name === "head-hud" && object.visible) headHud = object.matrixWorld.clone(); });
      scene.traverse((object) => {
        if (object instanceof actual.Mesh && object.material instanceof actual.MeshBasicMaterial &&
            object.material.map instanceof actual.CanvasTexture && object.name !== "head-hud") {
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
            cursorRenderOrder: cursor.renderOrder, worldTransparentOrders, groupOrders, headHud
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
    bundle = createThreeRenderer(canvas, canvas, null, "low", canvas);
    if (capture.driver === null) throw new Error("Recording driver was not constructed");
    driver = capture.driver;
  });

  beforeEach(() => {
    driver.xr.isPresenting = false;
    bundle.renderer.setStereoPresentation(null);
    bundle.renderer.setFlightPose(null);
    bundle.renderer.setPreparedFlightPose(null);
    bundle.renderer.setLakeSkyCondition(null);
    bundle.renderer.setLakeVenueVisible(true);
    bundle.renderer.setFlightCameraMode("pilot");
    bundle.renderer.setCinematicCameraView(null);
    driver.draws.length = 0;
  });

  afterAll(() => { bundle.renderer.dispose(); });

  it("exchanges quality geometry and wave budgets without regenerating detail or reflection resources", async () => {
    bundle.renderer.setFlightPose({ datumPositionNed: { north: 0, east: 0, down: -10 }, attitudeBodyToNed: IDENTITY_POSE.orientation,
      pilotPositionMeters: 0.03, initialPilotPositionMeters: 0, simulationTimeSeconds: 12.345 });
    bundle.renderer.render(frame());
    const original = lakeDrawResources(driver);
    expect(original.foreground.material.vertexShader).toContain(LAKE_WAVE_PROJECTION_GLSL);
    expect(original.foreground.material.vertexShader).toContain("projectLakeWaves(p, waveXZ");
    expect(original.foreground.material.vertexShader).not.toContain("3.1 * slopeEnergy");
    const near: unknown = original.foreground.material.uniforms.uDetailNear?.value;
    const far: unknown = original.foreground.material.uniforms.uDetailFar?.value;
    const sky: unknown = original.foreground.material.uniforms.uSkyTexture?.value;
    const reflection: unknown = original.foreground.material.uniforms.uReflectionTexture?.value;
    expect(reflection).toBeDefined();
    const detailCreation = vi.spyOn(lakeDetail, "createLakeDetailLayer");
    try {
      for (const quality of ["high", "medium", "low"] as const) {
        const previous = original.foreground.mesh.geometry;
        let disposals = 0;
        const onDispose = (): void => { disposals += 1; };
        previous.addEventListener("dispose", onDispose);
        await bundle.renderer.setLakeWaterQuality(quality);
        const profile = lakeWater.lakeWaterQualityProfile(quality);
        const projection = createLakeWaveProjection(lakeWater.DEFAULT_LAKE_VISUAL_CONDITION, quality);
        expect(original.foreground.mesh.geometry.getAttribute("position").count).toBe((profile.meshSegments + 1) ** 2);
        expect(original.foreground.material.uniforms.uWaveCount?.value).toBe(profile.componentCount);
        expect(original.backing.material.uniforms.uWaveCount?.value).toBe(profile.componentCount);
        expect(original.foreground.material.uniforms.uChoppiness?.value).toBe(projection.choppiness);
        expect(original.backing.material.uniforms.uChoppiness?.value).toBe(projection.choppiness);
        expect(original.foreground.material.uniforms.uGridSpacingGradient?.value).toBe(projection.spacing.gradientMagnitude);
        expect(original.backing.material.uniforms.uGridSpacingGradient?.value).toBe(projection.spacing.gradientMagnitude);
        expect(disposals).toBe(1);
        expect(Object.is(original.foreground.mesh.geometry, previous)).toBe(false);
        expect(Object.is(original.foreground.material.uniforms.uDetailNear?.value, near)).toBe(true);
        expect(Object.is(original.foreground.material.uniforms.uDetailFar?.value, far)).toBe(true);
        expect(Object.is(original.foreground.material.uniforms.uSkyTexture?.value, sky)).toBe(true);
        expect(Object.is(original.foreground.material.uniforms.uReflectionTexture?.value, reflection)).toBe(true);
        expect(original.foreground.material.uniforms.uTimeSeconds?.value).toBe(12.345);
        expect(original.backing.material.uniforms.uTimeSeconds?.value).toBe(12.345);
        previous.removeEventListener("dispose", onDispose);
      }
      const unchanged = original.foreground.mesh.geometry;
      await bundle.renderer.setLakeWaterQuality("low");
      expect(Object.is(original.foreground.mesh.geometry, unchanged)).toBe(true);
      expect(detailCreation).not.toHaveBeenCalled();
    } finally { detailCreation.mockRestore(); }
  });

  it("settles staged quality only after the next frame exchanges the current environment resources", async () => {
    const counts: unknown[] = [];
    let request: Promise<LakeWaterQualityCleanupResult> = Promise.resolve({ kind: "complete" });
    let applied = false;
    const detailCreation = vi.spyOn(lakeDetail, "createLakeDetailLayer");
    bundle.renderer.startLoop((timestamp) => {
      bundle.renderer.beginViewFrame();
      if (timestamp === 1) {
        bundle.renderer.setLakeVisualCondition({ ...lakeWater.DEFAULT_LAKE_VISUAL_CONDITION, windEastMetersPerSecond: 2, patternSeed: 42 });
        request = bundle.renderer.setLakeWaterQuality("high");
        void request.then(() => { applied = true; });
      }
      bundle.renderer.render(frame({ timestampMs: timestamp }));
      counts.push(lakeDrawResources(driver).foreground.material.uniforms.uWaveCount?.value);
    });
    try {
      driver.tick(1, null);
      expect(applied).toBe(false);
      expect(counts).toEqual([6]);
      expect(detailCreation).not.toHaveBeenCalled();
      driver.tick(2, null);
      await request;
      expect(applied).toBe(true);
      expect(counts).toEqual([6, 18]);
      expect(detailCreation).toHaveBeenCalledTimes(2);
      const foreground = lakeDrawResources(driver).foreground.material;
      expect(vectorUniform(foreground, "uWindVelocity").toArray()).toEqual([2, -0.54, 0, 0]);
      bundle.renderer.setLakeVisualCondition({ ...lakeWater.DEFAULT_LAKE_VISUAL_CONDITION, windNorthMetersPerSecond: 3, patternSeed: 43 });
      expect(foreground.uniforms.uWaveCount?.value).toBe(18);
    } finally {
      bundle.renderer.stopLoop();
      detailCreation.mockRestore();
      bundle.renderer.setLakeVisualCondition(lakeWater.DEFAULT_LAKE_VISUAL_CONDITION);
      await bundle.renderer.setLakeWaterQuality("low");
    }
  });

  it("retains all applied lake resources when quality preparation fails", async () => {
    bundle.renderer.render(frame());
    const foreground = lakeDrawResources(driver).foreground;
    const geometry = foreground.mesh.geometry;
    const waves: unknown = foreground.material.uniforms.uWaveKAmplitude?.value;
    const detail: unknown = foreground.material.uniforms.uDetailNear?.value;
    const spectrum = vi.spyOn(lakeWater, "createLakeWaveSpectrum");
    const failure = new Error("Injected wave allocation failure");
    spectrum.mockImplementationOnce(() => { throw failure; });
    try {
      await expect(bundle.renderer.setLakeWaterQuality("high")).rejects.toBe(failure);
      expect(Object.is(foreground.mesh.geometry, geometry)).toBe(true);
      expect(Object.is(foreground.material.uniforms.uWaveKAmplitude?.value, waves)).toBe(true);
      expect(Object.is(foreground.material.uniforms.uDetailNear?.value, detail)).toBe(true);
      expect(foreground.material.uniforms.uWaveCount?.value).toBe(6);
      await expect(bundle.renderer.setLakeWaterQuality("unknown" as LakeWaterQuality)).rejects.toThrow(RangeError);
      expect(Object.is(foreground.mesh.geometry, geometry)).toBe(true);
    } finally { spectrum.mockRestore(); }
    await bundle.renderer.setLakeWaterQuality("medium");
    expect(foreground.material.uniforms.uWaveCount?.value).toBe(10);
    await bundle.renderer.setLakeWaterQuality("low");
  });

  it("rejects an unfinished staged quality command on disposal", async () => {
    const canvas = { width: 1280, height: 720 } as HTMLCanvasElement;
    const trial = createThreeRenderer(canvas, canvas, null, "low", canvas);
    const localDriver = capture.driver;
    if (localDriver === null) throw new Error("Missing private recording driver");
    let request: Promise<LakeWaterQualityCleanupResult> | null = null;
    const readRequest = (): Promise<LakeWaterQualityCleanupResult> | null => request;
    trial.renderer.startLoop(() => {
      trial.renderer.beginViewFrame();
      request = trial.renderer.setLakeWaterQuality("high");
    });
    localDriver.tick(1, null);
    const pendingRequest = readRequest();
    if (pendingRequest === null) throw new Error("Missing staged quality request");
    const rejected = expect(pendingRequest).rejects.toThrow("disposed");
    trial.renderer.dispose();
    await rejected;
    await expect(trial.renderer.setLakeWaterQuality("medium")).rejects.toThrow("disposed");
  });

  it("reports post-swap cleanup failure as an applied quality and keeps the common view consistent", async () => {
    bundle.renderer.render(frame());
    const foreground = lakeDrawResources(driver).foreground;
    const previous = foreground.mesh.geometry;
    const failure = new Error("Injected geometry disposal failure");
    const onDispose = (): never => { throw failure; };
    previous.addEventListener("dispose", onDispose);
    const session = gameSessionState(1, 0, null);
    if (session === null) throw new Error("Missing quality selection session");
    const initial: AppModel = {
      ...createInitialAppModel(), presentation: { type: "ready", mode: "screen" }, gameSession: session,
      lakeWaterQuality: { kind: "ready", applied: "low", cleanup: { kind: "complete" } }
    };
    const requested = updateApp(initial, { type: "ui-action", action: { type: "activate", controlId: "presentation-water-quality-high" } });
    try {
      const cleanup = await bundle.renderer.setLakeWaterQuality("high");
      expect(cleanup).toEqual({ kind: "failed", message: failure.message });
      expect(Object.is(foreground.mesh.geometry, previous)).toBe(false);
      expect(foreground.mesh.geometry.getAttribute("position").count).toBe(225 ** 2);
      expect(foreground.material.uniforms.uWaveCount?.value).toBe(18);
      const completed = updateApp(requested.model, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "high", cleanup }).model;
      expect(completed.lakeWaterQuality).toEqual({ kind: "ready", applied: "high", cleanup });
      const controls = createGameViewModel(completed, null).panels[0]?.controls ?? [];
      expect(controls.find((control) => control.id === "presentation-water-quality-high")).toMatchObject({ presentation: { selected: true } });
      expect(controls.find((control) => control.id === "presentation-water-quality-status")).toMatchObject({ value: `Highを適用した。旧geometryの解放に失敗した: ${failure.message}` });
    } finally {
      previous.removeEventListener("dispose", onDispose);
      previous.dispose();
      await bundle.renderer.setLakeWaterQuality("low");
    }
  });

  it.each(["registered-archive", "unknown-archive", "calm-archive", "current-calm"] as const)(
    "applies the actual Rust %s venue projection without hiding water or aircraft", (recordKind) => {
      const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
      try {
        session.executeOperation("open-setup");
        session.executeOperation({ kind: "set-difficulty-option", axis: "weather", code: recordKind === "registered-archive" || recordKind === "unknown-archive" ? 2 : 0 });
        session.executeOperation("prepare"); session.executeOperation("start-flight");
        session.advanceCountdown(); session.advanceCountdown(); session.advanceCountdown(); session.launch(); session.executeOperation("abort");
        let saved = session.exportRecordJson();
        if (recordKind === "unknown-archive") {
          const document = JSON.parse(saved) as { header: { environment_version: number } };
          document.header.environment_version = 99;
          saved = JSON.stringify(document);
        }
        if (recordKind !== "current-calm") session.openArchive(saved);
        const phaseCode = session.readLifecycle().phaseCode;
        const identity = recordKind === "current-calm" ? session.readRecordSummary().context.scenario : session.readPlaybackContext().scenario;
        const environment = parseRuntimeEnvironmentSnapshot(session.readEnvironmentJson(), phaseCode, identity);
        bundle.renderer.setLakeVenueVisible(projectRuntimeVenue(environment, phaseCode).kind === "visible");
        bundle.renderer.render(frame());
        const scene = driver.scene;
        const venue = scene?.getObjectByName("lake-biwa-launch-venue");
        if (scene === null || venue === undefined) throw new Error("Missing registered venue group");
        expect(venue.children.length).toBeGreaterThan(0);
        expect(venue.visible).toBe(recordKind === "registered-archive" || recordKind === "current-calm");
        expect(scene.getObjectByName("aircraft-presentation-root")?.visible).toBe(true);
        expect(scene.getObjectByName("lake-biwa-distant-water-backing")?.visible).toBe(true);
        if (recordKind !== "current-calm") {
          session.executeOperation("leave-replay");
          const title = parseRuntimeEnvironmentSnapshot(session.readEnvironmentJson(), 0);
          bundle.renderer.setLakeVenueVisible(projectRuntimeVenue(title, 0).kind === "visible");
          bundle.renderer.render(frame());
          expect(venue.visible).toBe(true);
        }
      } finally { session.dispose(); }
    }
  );

  it("stages both hidden and visible venue projections at the next view frame", () => {
    const visible: boolean[] = [];
    bundle.renderer.startLoop((timestamp) => {
      bundle.renderer.beginViewFrame();
      if (timestamp === 1) bundle.renderer.setLakeVenueVisible(false);
      if (timestamp === 2) bundle.renderer.setLakeVenueVisible(true);
      bundle.renderer.render(frame({ timestampMs: timestamp }));
      const venue = driver.scene?.getObjectByName("lake-biwa-launch-venue");
      if (venue === undefined) throw new Error("Missing registered venue group");
      visible.push(venue.visible);
    });
    try {
      driver.tick(1, null); driver.tick(2, null); driver.tick(3, null);
      expect(visible).toEqual([true, false, true]);
    } finally { bundle.renderer.stopLoop(); }
  });

  it("starts without solar lighting or directional shadows when sky metadata is absent", () => {
    bundle.renderer.render(frame());
    const solar = driver.scene?.children.find((object) => object instanceof DirectionalLight);
    if (!(solar instanceof DirectionalLight)) throw new Error("Missing directional sun");
    expect(solar.intensity).toBe(0);
    expect(solar.castShadow).toBe(false);
  });

  it("connects one sky to background, solar lighting, near/far water and haze without changing wave parameters", () => {
    const condition = { sunAzimuthDegrees: 90, sunElevationDegrees: 0, cloudFraction: 0, visibilityMeters: 6_000 };
    bundle.renderer.setLakeSkyCondition(condition);
    bundle.renderer.render(frame());
    const scene = driver.scene;
    if (scene === null || !(scene.background instanceof DataTexture) || !(scene.fog instanceof Fog)) throw new Error("Missing sky scene");
    const solar = scene.children.find((object) => object instanceof DirectionalLight);
    if (!(solar instanceof DirectionalLight)) throw new Error("Missing directional sun");
    const materials: ShaderMaterial[] = [];
    scene.traverse((object) => {
      if (object instanceof Mesh && object.material instanceof ShaderMaterial && "uSkyTexture" in object.material.uniforms) materials.push(object.material);
    });
    const foreground = materials[0];
    const backing = materials[1];
    if (foreground === undefined || backing === undefined) throw new Error("Missing near/far lake shader");
    const solarDirection = solar.position.clone().sub(solar.target.position).normalize();
    expect(solarDirection.x).toBeCloseTo(1, 14);
    expect(solarDirection.y).toBeCloseTo(0, 14);
    expect(solarDirection.z).toBeCloseTo(0, 14);
    expect(solar.intensity).toBe(1.45);
    expect(solar.castShadow).toBe(true);
    expect(scene.fog.far).toBe(6_000);
    expect(scene.fog.near).toBe(1_050);
    for (const material of materials) {
      expect(material.uniforms.uSkyTexture?.value).toBe(scene.background);
      const waterSun = vectorUniform(material, "uSunDirection");
      expect(waterSun.x).toBeCloseTo(1, 14);
      expect(waterSun.y).toBeCloseTo(0, 14);
      expect(waterSun.z).toBeCloseTo(0, 14);
      expect(waterSun.w).toBe(1);
      expect(vectorUniform(material, "uHazeRange").toArray()).toEqual([1_050, 6_000, 0, 0]);
    }
    for (const uniform of ["uSkyTexture", "uSunDirection", "uHazeRange"]) {
      expect(foreground.uniforms[uniform]).toBe(backing.uniforms[uniform]);
    }
    const previous = scene.background;
    let disposals = 0;
    previous.addEventListener("dispose", () => { disposals += 1; });
    const waveAmplitude: unknown = foreground.uniforms.uWaveKAmplitude?.value;
    const detail: unknown = foreground.uniforms.uDetailNear?.value;
    bundle.renderer.setLakeSkyCondition({ ...condition });
    expect(scene.background).toBe(previous);
    expect(disposals).toBe(0);
    expect(() => { bundle.renderer.setLakeSkyCondition({ ...condition, visibilityMeters: 0 }); }).toThrow(RangeError);
    expect(scene.background).toBe(previous);
    bundle.renderer.setLakeSkyCondition({ ...condition, sunAzimuthDegrees: 0, sunElevationDegrees: 90, visibilityMeters: 8_000 });
    expect(scene.background).not.toBe(previous);
    expect(disposals).toBe(1);
    expect(foreground.uniforms.uWaveKAmplitude?.value).toBe(waveAmplitude);
    expect(foreground.uniforms.uDetailNear?.value).toBe(detail);
    expect(solar.position.clone().sub(solar.target.position).normalize().y).toBeCloseTo(1, 14);
    bundle.renderer.setLakeSkyCondition({ ...condition, sunElevationDegrees: -20 });
    expect(solar.intensity).toBe(0);
    expect(solar.castShadow).toBe(false);
    expect(vectorUniform(foreground, "uSunDirection").w).toBe(0);
    bundle.renderer.setLakeSkyCondition(null);
    expect(solar.intensity).toBe(0);
    expect(solar.castShadow).toBe(false);
    expect(vectorUniform(foreground, "uSunDirection").w).toBe(0);
    expect(scene.fog.far).toBe(80_000);
    expect(scene.fog.near).toBe(14_000);
    expect(foreground.uniforms.uSkyTexture?.value).toBe(scene.background);
    bundle.renderer.setLakeSkyCondition(condition);
    expect(solar.intensity).toBe(1.45);
    expect(solar.castShadow).toBe(true);
  });

  it("defers a view-phase sky change and explicit unavailable reset until the next frame", () => {
    const condition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, visibilityMeters: 25_000 };
    bundle.renderer.setLakeSkyCondition(condition);
    bundle.renderer.render(frame());
    const original = driver.scene?.background;
    if (!(original instanceof DataTexture)) throw new Error("Missing initial sky texture");
    const observed: { background: unknown; visibility: number }[] = [];
    bundle.renderer.startLoop((timestamp) => {
      bundle.renderer.beginViewFrame();
      if (timestamp === 1) bundle.renderer.setLakeSkyCondition({ ...condition, visibilityMeters: 10_000 });
      if (timestamp === 2) bundle.renderer.setLakeSkyCondition(null);
      bundle.renderer.render(frame({ timestampMs: timestamp }));
      if (!(driver.scene?.fog instanceof Fog)) throw new Error("Missing scene fog");
      observed.push({ background: driver.scene.background, visibility: driver.scene.fog.far });
    });
    try {
      driver.tick(1, null);
      driver.tick(2, null);
      driver.tick(3, null);
      expect(observed.map((entry) => entry.visibility)).toEqual([25_000, 10_000, 80_000]);
      expect(Object.is(observed[0]?.background, original)).toBe(true);
      expect(Object.is(observed[1]?.background, original)).toBe(false);
      expect(Object.is(observed[2]?.background, observed[1]?.background)).toBe(false);
    } finally { bundle.renderer.stopLoop(); }
  });

  it("captures current configured projections matching both actual StereoEffect eyes across resize, optics, and camera cuts", () => {
    let currentViewport = viewport;
    const observed: ViewerFrame[] = [];
    bundle.renderer.startLoop((_timestamp, viewer) => {
      observed.push(viewer);
      bundle.renderer.render(frame({ viewport: currentViewport, cameraPose: turnedHead }));
    });
    try {
      for (const [index, configuration] of [
        { size: viewport, profile: PHONE_VR_OPTICAL_PROFILE, external: false },
        { size: { x: 720, y: 1280, pixelRatio: 2 }, profile: { ...PHONE_VR_OPTICAL_PROFILE, eyeSeparationMeters: 0.071, focusDistanceMeters: 3, verticalFieldOfViewDegrees: 68 }, external: false },
        { size: viewport, profile: PHONE_VR_OPTICAL_PROFILE, external: true }
      ].entries()) {
        currentViewport = configuration.size;
        bundle.renderer.resize(currentViewport);
        bundle.renderer.setStereoPresentation(configuration.profile);
        if (configuration.external) {
          bundle.renderer.setFlightPose({ datumPositionNed: { north: 4, east: 2, down: -10 }, attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, initialPilotPositionMeters: 0 });
          bundle.renderer.setFlightCameraMode("platform");
          bundle.renderer.setCinematicCameraView({ pose: pose(vec3(10, 4, -3), IDENTITY_POSE.orientation), verticalFieldOfViewDegrees: 42 });
        }
        driver.draws.length = 0;
        driver.tick(index, null);
        const geometry = observed.at(-1);
        if (geometry?.source !== "configured") throw new Error("Configured geometry missing");
        expect(driver.draws).toHaveLength(2);
        const center = centerEye(driver.draws);
        const target = new Vector3(0.12, -0.2, -2.4);
        for (const [eyeIndex, draw] of driver.draws.entries()) {
          const eye = geometry.eyes[eyeIndex];
          if (eye === undefined) throw new Error("Eye missing");
          expectMatrix(new Matrix4().fromArray(eye.projection), draw.projection);
          const expected = target.clone().applyMatrix4(center).applyMatrix4(draw.camera.clone().invert()).applyMatrix4(draw.projection);
          const projected = projectHeadPoint(eye, vec3(target.x, target.y, target.z));
          expect(projected?.x).toBeCloseTo(expected.x, 10);
          expect(projected?.y).toBeCloseTo(expected.y, 10);
        }
      }
      bundle.renderer.setStereoPresentation(null);
      driver.tick(4, null);
      expect(observed.at(-1)).toEqual({ source: "unavailable", reason: "not-stereo", trackingFromHead: null });
    } finally {
      bundle.renderer.stopLoop();
    }
  });

  it("holds captured Phone optics through a view-phase camera cut and applies latest pending optics next frame", () => {
    const changedViewport = { x: 720, y: 1280, pixelRatio: 2 };
    const changedProfile = { ...PHONE_VR_OPTICAL_PROFILE, eyeSeparationMeters: 0.071, focusDistanceMeters: 3, verticalFieldOfViewDegrees: 68 };
    const flight: FlightRenderPose = { datumPositionNed: { north: 4, east: 2, down: -10 }, attitudeBodyToNed: IDENTITY_POSE.orientation, pilotPositionMeters: 0, initialPilotPositionMeters: 0 };
    bundle.renderer.resize(viewport);
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    bundle.renderer.setFlightPose(flight);
    bundle.renderer.setFlightCameraMode("platform");
    bundle.renderer.setCinematicCameraView({ pose: pose(vec3(10, 4, -3), IDENTITY_POSE.orientation), verticalFieldOfViewDegrees: 36 });
    const observed: ViewerFrame[] = [];
    bundle.renderer.startLoop((timestamp, viewer) => {
      observed.push(viewer);
      bundle.renderer.beginViewFrame();
      if (timestamp === 100) {
        bundle.renderer.setFlightPose(null);
        bundle.renderer.setFlightCameraMode("pilot");
        bundle.renderer.setCinematicCameraView(null);
        bundle.renderer.setStereoPresentation({ ...changedProfile, verticalFieldOfViewDegrees: 48 });
        bundle.renderer.resize(changedViewport);
      }
      bundle.renderer.render(frame({ viewport: changedViewport, cameraPose: turnedHead }));
    });
    try {
      for (const timestamp of [100, 200]) {
        driver.draws.length = 0;
        driver.tick(timestamp, null);
        const geometry = observed.at(-1);
        if (geometry?.source !== "configured") throw new Error("Missing configured optics");
        expect(driver.draws).toHaveLength(2);
        for (const [index, draw] of driver.draws.entries()) {
          const eye = geometry.eyes[index];
          if (eye === undefined) throw new Error("Missing captured eye");
          expectMatrix(new Matrix4().fromArray(eye.projection), draw.projection);
        }
        const firstDraw = driver.draws[0];
        if (firstDraw === undefined) throw new Error("Missing Phone draw");
        const verticalField = 2 * Math.atan(1 / firstDraw.projection.elements[5]) * 180 / Math.PI;
        expect(verticalField).toBeCloseTo(timestamp === 100 ? 36 : 68, 8);
        expect(firstDraw.projection.elements[5] / firstDraw.projection.elements[0])
          .toBeCloseTo(timestamp === 100 ? viewport.x / viewport.y / 2 : changedViewport.x / changedViewport.y / 2, 8);
        if (timestamp === 100) bundle.renderer.setStereoPresentation(changedProfile);
      }
    } finally { bundle.renderer.stopLoop(); }
  });

  it("keeps the Phone Head layer fixed in both actual StereoEffect eyes through body, PilotEye and head rotations", async () => {
    const valuesSnapshot = {
    ...currentFlightDisplayFixture(5),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 0, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 12, airspeedMetersPerSecond: 8, groundspeedMetersPerSecond: 9,
      windVelocityNedMetersPerSecond: { north: 0, east: 0, down: 0 }, angleOfAttackRadians: { kind: "available" as const, value: 0 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } }
  };
    const snapshot = valuesSnapshot;
    const gameSession = gameSessionState(5, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing Flight fixture");
    const model = { ...createInitialAppModel(), gameSession, presentation: { type: "ready", mode: "phone-vr" } as const };
    let currentFlight: FlightRenderPose = { datumPositionNed: { north: 100, east: 25, down: -8 },
      attitudeBodyToNed: IDENTITY_POSE.orientation, pilotPositionMeters: 0, initialPilotPositionMeters: 0 };
    bundle.renderer.resize(viewport);
    bundle.renderer.setFlightPose(currentFlight);
    const phone = await startPhone(bundle, viewport);
    const evidence = { viewer: null as ViewerFrame | null, view: null as UiViewModel | null };
    const runtime = new PresentationRuntime({ ...bundle.renderer, dispose() {} }, [{ mode: "phone-vr", start: () => Promise.resolve(), stop: () => Promise.resolve(),
      currentFrame: (timestamp, view, viewer, menu) => phone.backend.currentFrame(timestamp, view, viewer, menu) }], (viewer) => {
      evidence.viewer = viewer;
      const draft = createFlightFrameViewDraft(model, snapshot, viewer, "ja");
      if (draft.headHud.kind !== "visible") throw new Error("Missing Phone Head layout");
      const size = headHudCanvasSize(draft.headHud.layer);
      const paint = validateHeadHudPaint(prepareHeadHudPaint(new HudCanvasFixture(), draft.headHud, size.width, size.height), viewer);
      if (paint.kind !== "ready") throw new Error("Missing Phone Head plan");
      evidence.view = finalizeFlightFrameView(draft, paint.view);
      return fixturePresentation(evidence.view);
    }, () => { bundle.renderer.setFlightPose(currentFlight); });
    const projections: Vector3[][] = [];
    try {
      expect(await runtime.start("phone-vr")).toEqual({ ok: true });
      for (const [index, pilotPositionMeters] of [-0.15, 0.12, 0.35].entries()) {
        const body = new Quaternion().setFromAxisAngle(new Vector3(0.3, 0.7, -0.2).normalize(), index * 0.2);
        currentFlight = { ...currentFlight, pilotPositionMeters, attitudeBodyToNed: quaternion(body.w, body.x, body.y, body.z) };
        phone.emit({ alpha: index * 11, beta: 90 - index * 7, gamma: index * 5, timestampMs: 100 + index * 100, gravityEvidence: { kind: "earth-z-up" } });
        driver.draws.length = 0;
        driver.tick(100 + index * 100, null);
        expect(driver.draws).toHaveLength(2);
        const viewer = evidence.viewer;
        const headHud = evidence.view?.headHud;
        if (viewer?.source !== "configured" || headHud?.kind !== "visible") throw new Error("Missing same-frame configured Head");
        expect(viewer.trackingFromHead).toBeNull();
        const points: Vector3[] = [];
        for (const [eyeIndex, draw] of driver.draws.entries()) {
          const eye = viewer.eyes[eyeIndex];
          if (eye === undefined || draw.headHud === null) throw new Error("Missing Phone Head draw");
          expectMatrix(new Matrix4().fromArray(eye.projection), draw.projection);
          for (const horizontal of [-0.5, 0.5]) for (const vertical of [-0.5, 0.5]) {
            const projected = new Vector3(horizontal, vertical, 0).applyMatrix4(draw.headHud).applyMatrix4(draw.camera.clone().invert()).applyMatrix4(draw.projection);
            const headPoint = new Vector3(horizontal * headHud.size.width, vertical * headHud.size.height, 0).applyMatrix4(poseMatrix(headHud.localPose));
            const pure = projectHeadPoint(eye, vec3(headPoint.x, headPoint.y, headPoint.z));
            expect(projected.x).toBeCloseTo(pure?.x ?? Number.NaN, 8);
            expect(projected.y).toBeCloseTo(pure?.y ?? Number.NaN, 8);
            points.push(projected);
          }
        }
        projections.push(points);
      }
      const first = projections[0];
      if (first === undefined) throw new Error("Missing first Phone projection");
      for (const points of projections.slice(1)) points.forEach((point, index) => {
        const original = first[index];
        if (original === undefined) throw new Error("Missing corresponding Phone point");
        expect(point.distanceTo(original)).toBeLessThan(1e-8);
      });
    } finally { await runtime.dispose(); await phone.backend.stop(); }
  });

  it("copies runtime center and current eye projections in the actual animation-loop adapter", () => {
    const frames: ViewerFrame[] = [];
    bundle.renderer.startLoop((_timestamp, viewer) => { frames.push(viewer); });
    try {
      driver.xr.isPresenting = true;
      const transform = (offset: number) => ({ position: { x: offset, y: 1.6, z: 0, w: 1 }, orientation: { x: 0, y: 0, z: 0, w: 1 } });
      const matrix = new Matrix4().makePerspective(-0.08, 0.11, 0.09, -0.07, 0.1, 100);
      const projection = new Float32Array(matrix.elements);
      const viewer = { transform: transform(0), views: [
        { eye: "right", transform: transform(0.034), projectionMatrix: projection },
        { eye: "left", transform: transform(-0.032), projectionMatrix: projection }
      ] } as unknown as XRViewerPose;
      driver.tick(10, viewer);
      const initial = frames[0];
      if (initial?.source !== "runtime-derived") throw new Error("Runtime geometry missing");
      expect(initial.trackingFromHead.position.y).toBe(1.6);
      expect(initial.eyes[0].headFromEye.position.x).toBe(-0.032);
      projection[0] = 9;
      driver.tick(11, viewer);
      const changed = frames[1];
      if (changed?.source !== "runtime-derived") throw new Error("Second runtime frame missing");
      expect(changed.eyes[0].projection[0]).toBe(9);
      expect(initial.eyes[0].projection[0]).not.toBe(9);
      driver.tick(12, null);
      expect(frames[2]).toEqual({ source: "unavailable", reason: "viewer-unavailable", trackingFromHead: null });
    } finally {
      bundle.renderer.stopLoop();
    }
  });

  it.each(["Boot", "Title", "FlightSetup", "Briefing", "Result"] as const)("keeps non-flight %s Menu visible in both eyes and stable across viewport changes", (scene) => {
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    const panel = createSceneFixture(scene).panels[0];
    if (panel === undefined) throw new Error("Missing panel fixture");
    const panelPose = placeMenuPanel(IDENTITY_POSE, 2.4);
    for (const size of [viewport, { x: 1920, y: 1080, pixelRatio: 1 }, { x: 720, y: 1280, pixelRatio: 1 }]) {
      driver.draws.length = 0;
      bundle.renderer.render(frame({ panel: testPanel(panel, panelPose), viewport: size }));
      expect(driver.draws).toHaveLength(2);
      const basis = poseMatrix(titleScreenCameraPoseForViewport(size.x, size.y)).multiply(new Matrix4().makeRotationX(Math.PI / 30));
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
    const headFrame = frame({ cameraPose: turnedHead, panel: testPanel({ ...panel, anchor: "head" }, panelPose) });
    expect(bundle.renderer.transformTrackingPose(turnedHead)).toBe(turnedHead);
    bundle.renderer.render(headFrame);
    const expectedCenter = poseMatrix(titleScreenCameraPoseForViewport(1280, 720)).multiply(new Matrix4().makeRotationX(Math.PI / 30)).multiply(poseMatrix(turnedHead));
    const expectedPanel = poseMatrix(titleScreenCameraPoseForViewport(1280, 720)).multiply(new Matrix4().makeRotationX(Math.PI / 30)).multiply(poseMatrix(panelPose));
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
      startListening: (reading) => { emit = reading; reading({ alpha: 0, beta: 90, gamma: 0, timestampMs: 0, gravityEvidence: { kind: "earth-z-up" } }); },
      stopListening: () => { emit = null; }
    };
    const backend = new PhoneVrPresentationBackend(sensors, bundle.renderer, () => viewport, () => undefined,
      (message) => { throw new Error(message); }, { nowMs: () => 1000 });
    await backend.requestPermissionFromUserGesture();
    await backend.start();
    const view = createSceneFixture("Title");
    const render = (timestamp: number): { frame: BackendFrame; relative: Matrix4 } => {
      const current = fixtureBackendFrame(backend, timestamp, view, configuredViewerFixture());
      driver.draws.length = 0;
      bundle.renderer.render(current);
      const draw = driver.draws[0];
      if (draw === undefined) throw new Error("No stereo draw");
      const relative = centerEye(driver.draws).invert().multiply(draw.panel);
      expectMatrix(relative, poseMatrix(current.cameraPose).invert().multiply(poseMatrix(visiblePanelFrame(current).pose)));
      const hit = intersectPanel({ origin: current.cameraPose.position, direction: rotateVec3(current.cameraPose.orientation, vec3(0, 0, -1)) }, visiblePanelFrame(current).pose);
      expect(hit).not.toBeNull();
      return { frame: current, relative };
    };
    try {
      const opening = render(0);
      const sendReading = emit as ((reading: PhoneVrSensorReading) => void) | null;
      if (sendReading === null) throw new Error("No sensor listener");
      sendReading({ alpha: 12, beta: 84, gamma: 0, timestampMs: 10, gravityEvidence: { kind: "earth-z-up" } });
      const before = render(10);
      backend.recenterTracking();
      expectMatrix(render(11).relative, before.relative);
      backend.recenterMenu();
      expectMatrix(render(12).relative, opening.relative);
    } finally {
      await backend.stop();
    }
  });

  it.each(["world", "cockpit"] as const)("retains the existing non-flight %s anchor transform", (anchor) => {
    const panel = createSceneFixture("Title").panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const current = frame({ panel: testPanel({ ...panel, anchor }) });
    bundle.renderer.render(current);
    expectMatrix(singleDraw(driver).panel, poseMatrix(visiblePanelFrame(current).pose));
  });

  it("preserves Screen composition and the native XR reference", () => {
    bundle.renderer.render(frame({ panel: { kind: "absent" } }));
    expectMatrix(singleDraw(driver).camera, poseMatrix(titleScreenCameraPoseForViewport(1280, 720)));
    driver.draws.length = 0;
    driver.xr.isPresenting = true;
    const current = frame();
    bundle.renderer.render(current);
    expectMatrix(singleDraw(driver).camera, new Matrix4());
    expectMatrix(singleDraw(driver).panel, poseMatrix(visiblePanelFrame(current).pose));
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("keeps the %s menu camera and anchors fixed while showing the sealed launch pose", (mode) => {
    driver.xr.isPresenting = mode === "webxr";
    bundle.renderer.setStereoPresentation(mode === "phone-vr" ? PHONE_VR_OPTICAL_PROFILE : null);
    const current = frame({ cameraPose: turnedHead });
    bundle.renderer.render(current);
    const baseline = driver.draws.map((draw) => ({ camera: draw.camera.clone(), panel: draw.panel.clone() }));
    const heading = -Math.PI / 4;
    const initial: FlightRenderPose = {
      datumPositionNed: { north: 0.3, east: -0.2, down: -10.4 },
      attitudeBodyToNed: quaternion(Math.cos(heading / 2), 0, 0, Math.sin(heading / 2)),
      pilotPositionMeters: 0.17,
      initialPilotPositionMeters: 0.17,
      controls: { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.035, verticalTailRadians: -0.018 } },
      tailGeometry: { kind: "available", value: { kind: "bpg041_playable_version_two", horizontalTailArmMeters: 3.6 } }
    };
    bundle.renderer.setPreparedFlightPose(initial);
    driver.draws.length = 0;
    bundle.renderer.render(current);
    expect(driver.draws).toHaveLength(baseline.length);
    driver.draws.forEach((draw, index) => {
      const previous = baseline[index];
      if (previous === undefined) throw new Error("Missing baseline camera");
      expectMatrix(draw.camera, previous.camera);
      expectMatrix(draw.panel, previous.panel);
    });
    const aircraft = driver.scene?.getObjectByName("aircraft-presentation-root");
    if (aircraft === undefined) throw new Error("Missing projected aircraft");
    const preparedMatrix = aircraft.matrixWorld.clone();
    expectMatrix(preparedMatrix, poseMatrix(flightRelativePose(initial, IDENTITY_POSE)));
    expect(driver.scene?.getObjectByName("horizontal-tail-incidence")?.rotation.x).toBeCloseTo(0.035, 12);
    expect(driver.scene?.getObjectByName("vertical-tail-incidence")?.rotation.y).toBeCloseTo(0.018, 12);
    bundle.renderer.setPreparedFlightPose(null);
    bundle.renderer.setFlightPose(initial);
    driver.draws.length = 0;
    bundle.renderer.render(current);
    expectMatrix(aircraft.matrixWorld, preparedMatrix);
    bundle.renderer.setFlightPose(null);
    driver.draws.length = 0;
    bundle.renderer.render(current);
    driver.draws.forEach((draw, index) => {
      const previous = baseline[index];
      if (previous === undefined) throw new Error("Missing return-to-Title camera");
      expectMatrix(draw.camera, previous.camera);
      expectMatrix(draw.panel, previous.panel);
    });
  });

  it("renders the current physical tail controls and resets absent controls", () => {
    const base = { datumPositionNed: { north: 0, east: 0, down: -10 }, attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
      pilotPositionMeters: 0, initialPilotPositionMeters: 0 };
    bundle.renderer.setFlightPose({ ...base, controls: { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.02, verticalTailRadians: -0.03 } },
      tailGeometry: { kind: "available", value: { kind: "bpg041_playable_version_two", horizontalTailArmMeters: 3.6 } } });
    expect(() => { bundle.renderer.render(frame({})); }).not.toThrow();
    expect(driver.draws).toHaveLength(1);
    expect(driver.scene?.getObjectByName("horizontal-tail-incidence")?.rotation.x).toBeCloseTo(0.02);
    expect(driver.scene?.getObjectByName("vertical-tail-incidence")?.rotation.y).toBeCloseTo(0.03);
    expect(driver.scene?.getObjectByName("horizontal-tail-incidence")?.position.z).toBe(3.6);
    expect(driver.scene?.getObjectByName("vertical-tail-incidence")?.position.z).toBe(1.8);
    bundle.renderer.setFlightPose(base);
    bundle.renderer.render(frame({}));
    expect(driver.scene?.getObjectByName("horizontal-tail-incidence")?.visible).toBe(false);
    expect(driver.scene?.getObjectByName("vertical-tail-incidence")?.visible).toBe(false);
    expect(driver.scene?.getObjectByName("horizontal-tail-incidence")?.rotation.x).toBe(0);
    expect(driver.scene?.getObjectByName("vertical-tail-incidence")?.rotation.y).toBe(0);
  });

  it("preserves Pilot/Cockpit and external Replay transforms without an additional Title basis", () => {
    const flight: FlightRenderPose = {
      datumPositionNed: { north: 100, east: 25, down: -8 },
      attitudeBodyToNed: { w: Math.cos(0.1), x: Math.sin(0.1), y: 0, z: 0 },
      pilotPositionMeters: 0.15, initialPilotPositionMeters: 0
    };
    bundle.renderer.setFlightPose(flight);
    const current = frame({ cameraPose: turnedHead, panel: testPanel(requiredScenePanel("Flight")) });
    bundle.renderer.render(current);
    const eye = pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0.15, 0);
    expectMatrix(singleDraw(driver).camera, poseMatrix(flightRelativePose(flight, composePose(eye, turnedHead))));
    expectMatrix(singleDraw(driver).panel, poseMatrix(flightRelativePose(flight, visiblePanelFrame(current).pose)));
    const external = pose(vec3(100, 25, -40), turnedHead.orientation);
    bundle.renderer.setFlightCameraMode("platform");
    bundle.renderer.setCinematicCameraView({ pose: external, verticalFieldOfViewDegrees: 60 });
    bundle.renderer.setStereoPresentation(PHONE_VR_OPTICAL_PROFILE);
    driver.draws.length = 0;
    const transformed = bundle.renderer.transformTrackingPose(turnedHead);
    expectMatrix(poseMatrix(transformed), poseMatrix(composePose(external, turnedHead)));
    const worldPanel = placeMenuPanel(transformed, 2.4);
    bundle.renderer.render(frame({ cameraPose: turnedHead, panel: testPanel(requiredScenePanel("Title"), worldPanel) }));
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
        cameraPose: turnedHead, panel: { ...testPanel({ ...basePanel, anchor }, panelPose), cursor: { point: { x: 0.2, y: -0.3 }, progress: 0.5 } }
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

  it.each([[1280, 720], [720, 1280]] as const)("fits the actual Pause Menu and hit targets in both frozen StereoEffect eyes at %s x %s", async (width, height) => {
    const size = { x: width, y: height, pixelRatio: 1 };
    const valuesSnapshot = {
    ...currentFlightDisplayFixture(6),
    positionNed: { north: 0, east: 0, down: 0 }, velocityNed: { north: 0, east: 0, down: 0 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 }, pilotPositionMeters: 0, pilotVelocityMetersPerSecond: 0,
    stamp: { kind: "exact" as const, tick: 0, fraction: 0, timeSeconds: 0 },
    telemetry: { kind: "available" as const, value: { altitudeMeters: 12, airspeedMetersPerSecond: 8, groundspeedMetersPerSecond: 9,
      windVelocityNedMetersPerSecond: { north: 0, east: 0, down: 0 }, angleOfAttackRadians: { kind: "available" as const, value: 0 }, sideslipAngleRadians: { kind: "available" as const, value: 0 },
      rollRadians: 0, pitchRadians: 0, headingRadians: 0 } }
  };
    const snapshot = valuesSnapshot;
    const gameSession = gameSessionState(6, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing Pause domain fixture");
    const model = { ...createInitialAppModel(), gameSession, presentation: { type: "ready", mode: "phone-vr" } as const };
    for (const pilotPositionMeters of [-0.15, 0.1, 0.35]) {
      bundle.renderer.resize(size);
      bundle.renderer.setFlightPose(null);
      const phone = await startPhone(bundle, size);
      let paused = false;
      const observed = { viewer: null as ViewerFrame | null, frame: null as BackendFrame | null };
      const runtime = new PresentationRuntime({ ...bundle.renderer, dispose() {} }, [{ mode: "phone-vr", start: () => Promise.resolve(), stop: () => Promise.resolve(),
        currentFrame: (timestamp, view, viewer, menu) => { observed.frame = phone.backend.currentFrame(timestamp, view, viewer, menu); return observed.frame; } }], (viewer) => {
        observed.viewer = viewer;
        if (!paused) return fixturePresentation(createSceneFixture("Title"));
        const draft = createFlightFrameViewDraft(model, snapshot, viewer, "ja");
        return fixturePresentation(finalizeFlightFrameView(draft, draft.headHud));
      });
      try {
        expect(await runtime.start("phone-vr")).toEqual({ ok: true });
        driver.tick(1, null);
        bundle.renderer.setFlightPose({ datumPositionNed: { north: 100, east: 25, down: -8 },
          attitudeBodyToNed: IDENTITY_POSE.orientation, pilotPositionMeters, initialPilotPositionMeters: 0.1 });
        paused = true;
        driver.draws.length = 0;
        driver.tick(100, null);
        const current = observed.frame;
        const viewer = observed.viewer;
        if (current === null || viewer?.source !== "configured") throw new Error("Missing same-frame Menu evidence");
        const visible = visiblePanelFrame(current);
        expect(visible.panel.title).toBe("Pause");
        expect(visible.panel.size).toEqual({ width: 2.4, height: 1.8 });
        expect(driver.draws).toHaveLength(2);
        for (const [eyeIndex, draw] of driver.draws.entries()) {
          const eye = viewer.eyes[eyeIndex];
          if (eye === undefined) throw new Error("Missing eye");
          expectMatrix(new Matrix4().fromArray(eye.projection), draw.projection);
          for (const coordinateX of [-1.2, 1.2]) for (const coordinateY of [-0.9, 0.9]) expectVisible(new Vector3(coordinateX, coordinateY, 0), draw);
          for (const control of visible.panel.controls) {
            for (const normalizedX of [control.rect.x, control.rect.x + control.rect.width]) {
              for (const normalizedY of [control.rect.y, control.rect.y + control.rect.height]) {
                expectVisible(new Vector3((normalizedX - 0.5) * 2.4, (0.5 - normalizedY) * 1.8, 0), draw);
              }
            }
          }
        }
        const center = centerEye(driver.draws);
        const worldPanel = driver.draws[0]?.panel;
        if (worldPanel === undefined) throw new Error("Missing Menu draw");
        const origin = new Vector3().setFromMatrixPosition(center);
        for (const control of visible.panel.controls) {
          if (control.kind === "status") continue;
          const target = new Vector3((control.rect.x + control.rect.width / 2 - 0.5) * 2.4,
            (0.5 - control.rect.y - control.rect.height / 2) * 1.8, 0).applyMatrix4(worldPanel);
          const direction = target.clone().sub(origin).normalize();
          const point = intersectPanel({ origin: vec3(origin.x, origin.y, origin.z), direction: vec3(direction.x, direction.y, direction.z) }, matrixPose(worldPanel));
          if (point === null) throw new Error("Rendered Menu ray missed");
          expect(hitTestControl(visible.panel, point)?.id).toBe(control.id);
        }
        driver.draws.length = 0;
        driver.tick(10_100, null);
        if (observed.frame === null) throw new Error("Missing silent frame");
        expect(visiblePanelFrame(observed.frame).pose).toEqual(visible.pose);
        expect(driver.draws).toHaveLength(2);
        expect(runtime.currentMode).toBe("phone-vr");
      } finally { await runtime.dispose(); await phone.backend.stop(); }
    }
  });

  it.each(["menu", "head", "cockpit", "world"] as const)("matches real Phone VR %s gaze to both rendered eyes for moving PilotEye positions", async (anchor) => {
    for (const pilotPositionMeters of [-0.15, 0.1, 0.35]) {
      const flight: FlightRenderPose = {
        datumPositionNed: { north: 100, east: 25, down: -8 },
        attitudeBodyToNed: { w: Math.cos(0.1), x: Math.sin(0.1), y: 0, z: 0 },
        pilotPositionMeters, initialPilotPositionMeters: 0.1
      };
      bundle.renderer.setFlightPose(flight);
      const phone = await startPhone(bundle, viewport);
      try {
        phone.emit({ alpha: 12, beta: 84, gamma: 0, timestampMs: 10, gravityEvidence: { kind: "earth-z-up" } });
        const rawFrame = fixtureBackendFrame(phone.backend, 10, { ...createSceneFixture("Flight"), panels: [] }, configuredViewerFixture());
        const mountedHead = composePose(pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, pilotPositionMeters, 0.1), rawFrame.cameraPose);
        const view = phonePanel(anchor, mountedHead);
        const current = fixtureBackendFrame(phone.backend, 20, view, configuredViewerFixture());
        driver.draws.length = 0;
        bundle.renderer.render(current);
        const center = centerEye(driver.draws);
        const panel = driver.draws[0]?.panel;
        if (panel === undefined) throw new Error("Missing Phone panel draw");
        expectMatrix(center, poseMatrix(flightRelativePose(flight, mountedHead)));
        expectMatrix(panel, poseMatrix(flightRelativePose(flight, visiblePanelFrame(current).pose)));
        const worldCamera = matrixPose(center);
        const point = intersectPanel({ origin: worldCamera.position,
          direction: rotateVec3(worldCamera.orientation, vec3(0, 0, -1)) }, matrixPose(panel));
        expect(point?.x).toBeCloseTo(panelFrameCursor(current)?.point.x ?? Number.NaN, 8);
        expect(point?.y).toBeCloseTo(panelFrameCursor(current)?.point.y ?? Number.NaN, 8);
        expect(panelFrameCursor(current)?.point.x).toBeCloseTo(0, 8);
        expect(panelFrameCursor(current)?.point.y).toBeCloseTo(0, 8);
        for (const draw of driver.draws) expectVisible(new Vector3(), draw);
        fixtureBackendFrame(phone.backend, 2020, view, configuredViewerFixture());
        expect(phone.actions).toContainEqual({ type: "activate", controlId: "phone-eye-button" });
        await phone.backend.stop();
        driver.draws.length = 0;
        bundle.renderer.render(frame({ panel: { kind: "absent" } }));
        const eye = pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, pilotPositionMeters, 0.1);
        expectMatrix(singleDraw(driver).camera, poseMatrix(flightRelativePose(flight, eye)));
      } finally {
        await phone.backend.stop();
      }
    }
  });

  it.each(["pilot", "platform"] as const)("preserves Phone VR tracking/Menu recenter with the %s mount", async (mode) => {
    const flight: FlightRenderPose = {
      datumPositionNed: { north: 100, east: 25, down: -8 },
      attitudeBodyToNed: { w: Math.cos(0.1), x: Math.sin(0.1), y: 0, z: 0 },
      pilotPositionMeters: 0.35, initialPilotPositionMeters: 0.1
    };
    bundle.renderer.setFlightPose(flight);
    bundle.renderer.setFlightCameraMode(mode);
    const external = pose(vec3(50, 20, -130), quaternion(Math.cos(0.23), 0, Math.sin(0.23), 0));
    if (mode === "platform") bundle.renderer.setCinematicCameraView({ pose: external, verticalFieldOfViewDegrees: 60 });
    const mount = mode === "platform" ? external : pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0.35, 0.1);
    const phone = await startPhone(bundle, viewport);
    try {
      phone.emit({ alpha: 12, beta: 84, gamma: 0, timestampMs: 10, gravityEvidence: { kind: "earth-z-up" } });
      const rawFrame = fixtureBackendFrame(phone.backend, 10, { ...createSceneFixture("Flight"), panels: [] }, configuredViewerFixture());
      const view = phonePanel("menu", composePose(mount, rawFrame.cameraPose));
      const render = (timestampMs: number): Matrix4 => {
        const current = fixtureBackendFrame(phone.backend, timestampMs, view, configuredViewerFixture());
        driver.draws.length = 0;
        bundle.renderer.render(current);
        const panel = driver.draws[0]?.panel;
        if (panel === undefined) throw new Error("Missing Phone recenter panel");
        expect(panelFrameCursor(current)?.point.x).toBeCloseTo(0, 8);
        expect(panelFrameCursor(current)?.point.y).toBeCloseTo(0, 8);
        if (mode === "platform") expectMatrix(centerEye(driver.draws), poseMatrix(composePose(external, current.cameraPose)));
        return centerEye(driver.draws).invert().multiply(panel);
      };
      const before = render(20);
      phone.backend.recenterTracking();
      expectMatrix(render(21), before);
      phone.backend.recenterMenu();
      expectMatrix(render(22), before);
    } finally {
      await phone.backend.stop();
    }
  });
});

async function startPhone(bundle: ThreeRendererBundle, viewport: BackendFrame["viewport"]): Promise<{
  readonly backend: PhoneVrPresentationBackend;
  readonly actions: UiAction[];
  readonly emit: (reading: PhoneVrSensorReading) => void;
}> {
  let listener: ((reading: PhoneVrSensorReading) => void) | null = null;
  const sensors: PhoneVrSensorPort = {
    checkAvailability: () => Promise.resolve({ supported: true, message: "Available" }),
    requestPermissionFromUserGesture: () => Promise.resolve({ ok: true }),
    getScreenOrientationAngle: () => 90,
    startListening: (reading) => { listener = reading; reading({ alpha: 0, beta: 90, gamma: 0, timestampMs: 0, gravityEvidence: { kind: "earth-z-up" } }); },
    stopListening: () => { listener = null; }
  };
  const actions: UiAction[] = [];
  const backend = new PhoneVrPresentationBackend(sensors, bundle.renderer, () => viewport,
    (action) => { actions.push(fixtureSemanticAction(action)); }, (message) => { throw new Error(message); }, { nowMs: () => 10_000 });
  await backend.requestPermissionFromUserGesture();
  await backend.start();
  return { backend, actions, emit: (reading) => {
    if (listener === null) throw new Error("Phone sensor listener is unavailable");
    listener(reading);
  } };
}

function phonePanel(anchor: AnchorKind, mountedHead: Pose): UiViewModel {
  const fixture = createSceneFixture("Flight");
  const panel = fixture.panels[0];
  if (panel === undefined) throw new Error("Missing Phone Flight panel");
  const forward = pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation);
  const localPose = anchor === "menu" ? IDENTITY_POSE : anchor === "head" ? forward : composePose(mountedHead, forward);
  return { ...fixture, panels: [{ ...panel, anchor, localPose, controls: [{
    id: "phone-eye-button", kind: "button", label: "Phone Eye", enabled: true,
    rect: { x: 0.25, y: 0.25, width: 0.5, height: 0.5 }
  }] }] };
}

function lakeDrawResources(driver: RecordingDriver): {
  readonly foreground: { readonly mesh: Mesh; readonly material: ShaderMaterial };
  readonly backing: { readonly mesh: Mesh; readonly material: ShaderMaterial };
} {
  const candidates: { readonly mesh: Mesh; readonly material: ShaderMaterial }[] = [];
  driver.scene?.traverse((object) => {
    if (isLakeDrawMesh(object)) {
      candidates.push({ mesh: object, material: object.material });
    }
  });
  const foreground = candidates.find((entry) => entry.mesh.geometry.getAttribute("position").count > 4);
  const backing = candidates.find((entry) => entry.mesh.geometry.getAttribute("position").count === 4);
  if (foreground === undefined || backing === undefined) throw new Error("Missing lake render resources");
  return { foreground, backing };
}

function isLakeDrawMesh(object: Object3D): object is Mesh<BufferGeometry, ShaderMaterial> {
  return object instanceof Mesh && object.geometry instanceof BufferGeometry
    && object.material instanceof ShaderMaterial && "uWaveCount" in object.material.uniforms;
}

function vectorUniform(material: ShaderMaterial, name: string): Vector4 {
  const value: unknown = material.uniforms[name]?.value;
  if (!(value instanceof Vector4)) throw new Error(`Missing vector uniform: ${name}`);
  return value;
}

function matrixPose(matrix: Matrix4): Pose {
  const position = new Vector3();
  const orientation = new Quaternion();
  matrix.decompose(position, orientation, new Vector3());
  return pose(vec3(position.x, position.y, position.z), quaternion(orientation.w, orientation.x, orientation.y, orientation.z));
}

function frame(overrides: Partial<BackendFrame> = {}): BackendFrame {
  return {
    headHud: { kind: "absent" },
    timestampMs: 0, cameraPose: IDENTITY_POSE, panel: testPanel(requiredScenePanel("Title")),
    viewport: { x: 1280, y: 720, pixelRatio: 1 }, ...overrides
  };
}

function requiredScenePanel(scene: UiViewModel["scene"]): UiPanel {
  const panel = createSceneFixture(scene).panels[0];
  if (panel === undefined) throw new Error("Missing scene panel");
  return panel;
}

function testPanel(panel: UiPanel, panelPose = placeMenuPanel(IDENTITY_POSE, 2.4)): Extract<BackendFrame["panel"], { readonly kind: "visible" }> {
  return { kind: "visible", panel, pose: panelPose, cursor: null };
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
