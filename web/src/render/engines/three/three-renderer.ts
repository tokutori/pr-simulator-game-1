import {
  AmbientLight,
  BufferAttribute,
  CanvasTexture,
  CircleGeometry,
  Color,
  DoubleSide,
  Group,
  LinearFilter,
  DirectionalLight,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  SRGBColorSpace,
  ShaderMaterial,
  Vector4,
  WebGLRenderer
} from "three";
import { StereoEffect } from "three/addons/effects/StereoEffect.js";
import type { Object3D } from "three";
import type { BackendFrame, FlightCameraMode, FlightRenderPose, RendererAdapter, SelectRay, StereoPresentationProfile, ViewportSize } from "../../contracts/runtime.js";
import type { CinematicCameraView } from "../../contracts/camera.js";
import { IDENTITY_POSE, quaternion, rotateVec3, vec3 } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import { composePose, inversePose } from "../../contracts/math.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "../../../presentation/webxr-contracts.js";
import { selectRayFromXrEvent } from "./xr-select-ray.js";
import { flightRelativePose } from "./flight-pose.js";
import { pilotEyePoseThree, poseFrdToThree, SYNTHETIC_PILOT_EYE_POINT } from "../../camera/pilot-eye-point.js";
import { replayCameraPoseFrd } from "../../camera/replay-camera.js";
import { createLakeWaveSpectrum, DEFAULT_LAKE_VISUAL_CONDITION, lakeWaterQualityProfile, selectLakeWaveComponentsForQuality } from "../../contracts/lake-water.js";
import type { LakeVisualCondition, LakeWaterQuality } from "../../contracts/lake-water.js";
import { createLakeDetailLayer } from "./lake-detail-texture.js";
import type { LakeDetailLayer } from "./lake-detail-texture.js";
import { createLakeSkyTexture } from "./lake-sky-texture.js";
import { createBirdmanAirframe } from "./birdman-airframe.js";

type ThreeWebXrState =
  | { readonly type: "idle" }
  | { readonly type: "requesting" }
  | { readonly type: "requested"; readonly session: XRSession }
  | { readonly type: "attaching"; readonly session: XRSession }
  | { readonly type: "active"; readonly session: XRSession }
  | { readonly type: "stopping"; readonly session: XRSession }
  | { readonly type: "failed"; readonly message: string };

export interface ThreeRendererBundle {
  readonly renderer: RendererAdapter;
  readonly webxr: WebXrSessionPort;
}

// The reference photograph has no metric scale. This affects appearance only;
// the spectrum contract and the still-water contact plane retain their values.
const LAKE_VISUAL_HEIGHT_SCALE = 8.0;
interface LakeVisualResources {
  readonly windSpeed: number;
  readonly near: LakeDetailLayer;
  readonly far: LakeDetailLayer;
  readonly waveKAmplitude: readonly Vector4[];
  readonly waveOmegaPhase: readonly Vector4[];
  readonly waveCount: number;
  readonly visualWaveHeight: number;
}

function createLakeVisualResources(condition: LakeVisualCondition, quality: LakeWaterQuality): LakeVisualResources {
  const windSpeed = Math.hypot(condition.windNorthMetersPerSecond, condition.windEastMetersPerSecond);
  if (!Number.isFinite(windSpeed) || windSpeed < 0.05 ||
      !Number.isFinite(condition.detailAmplitudeScale) || condition.detailAmplitudeScale <= 0 || condition.detailAmplitudeScale > 3 ||
      !Number.isSafeInteger(condition.patternSeed) || condition.patternSeed < 0) {
    throw new RangeError("Invalid render-only lake visual condition");
  }
  const directionX = condition.windEastMetersPerSecond / windSpeed;
  const directionZ = -condition.windNorthMetersPerSecond / windSpeed;
  const spectrum = createLakeWaveSpectrum(condition.windNorthMetersPerSecond, condition.windEastMetersPerSecond, condition.fetchMeters, 18);
  const renderedWaves = selectLakeWaveComponentsForQuality(spectrum, quality);
  const renderedWaveBands = renderedWaves.map((wave) =>
    Math.floor(spectrum.components.indexOf(wave) / 3)
  );
  const directionCountByBand = new Array<number>(6).fill(0);
  for (const band of renderedWaveBands) {
    if (band >= 0 && band < directionCountByBand.length) {
      directionCountByBand[band] = (directionCountByBand[band] ?? 0) + 1;
    }
  }
  const amplitudeScaleByWave = renderedWaveBands.map((band) => {
    const directionCount = directionCountByBand[band] ?? 0;
    return directionCount > 0 ? Math.sqrt(3 / directionCount) : 1;
  });
  const near = createLakeDetailLayer(64, 1400, 1717 + condition.patternSeed * 997, directionX, directionZ);
  let far: LakeDetailLayer;
  try {
    far = createLakeDetailLayer(193, 2400, 2917 + condition.patternSeed * 991, directionX, directionZ);
  } catch (error) {
    near.texture.dispose();
    throw error;
  }
  const waveKAmplitude = Array.from({ length: 24 }, (_, index) => {
    const wave = renderedWaves[index];
    return wave === undefined ? new Vector4() : new Vector4(
      wave.directionEast,
      -wave.directionNorth,
      wave.waveNumberRadiansPerMeter,
      wave.amplitudeMeters * (amplitudeScaleByWave[index] ?? 1) * lakeVisualAmplitudeScale(wave.waveNumberRadiansPerMeter)
    );
  });
  const waveOmegaPhase = Array.from({ length: 24 }, (_, index) => {
    const wave = renderedWaves[index];
    return wave === undefined ? new Vector4() : new Vector4(
      wave.angularFrequencyRadiansPerSecond,
      wave.phaseRadians,
      0,
      0
    );
  });
  return {
    windSpeed, near, far, waveKAmplitude, waveOmegaPhase,
    waveCount: renderedWaves.length,
    visualWaveHeight: spectrum.significantWaveHeightMeters * LAKE_VISUAL_HEIGHT_SCALE
  };
}

function sameLakeVisualCondition(a: LakeVisualCondition, b: LakeVisualCondition): boolean {
  return a.windNorthMetersPerSecond === b.windNorthMetersPerSecond &&
    a.windEastMetersPerSecond === b.windEastMetersPerSecond &&
    a.fetchMeters === b.fetchMeters &&
    a.detailAmplitudeScale === b.detailAmplitudeScale &&
    a.patternSeed === b.patternSeed;
}

export function createThreeRenderer(
  canvas: HTMLCanvasElement,
  panelCanvas: HTMLCanvasElement,
  xrSystem: XRSystem | null,
  lakeQuality: LakeWaterQuality = "high"
): ThreeRendererBundle {
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
  const stereoEffect = new StereoEffect(renderer);
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setClearColor(0x9fb0ad, 1);

  const scene = new Scene();
  const skyTexture = createLakeSkyTexture();
  scene.background = skyTexture;
  scene.add(new AmbientLight(0xdceaf0, 1.15));
  const sun = new DirectionalLight(0xffefd8, 1.45);
  sun.position.set(-20, 35, -18);
  scene.add(sun);

  const waterQuality = lakeWaterQualityProfile(lakeQuality);
  const waterGeometry = createLakeGeometry(waterQuality.meshSegments);
  let activeLakeCondition = DEFAULT_LAKE_VISUAL_CONDITION;
  let lakeResources = createLakeVisualResources(activeLakeCondition, lakeQuality);
  const waterMaterial = new ShaderMaterial({
    uniforms: {
      uTimeSeconds: { value: 0 },
      uWindSpeed: { value: lakeResources.windSpeed },
      uWindVelocity: { value: new Vector4(activeLakeCondition.windEastMetersPerSecond, -activeLakeCondition.windNorthMetersPerSecond, 0, 0) },
      uDetailScale: { value: activeLakeCondition.detailAmplitudeScale },
      uDetailNear: { value: lakeResources.near.texture },
      uDetailFar: { value: lakeResources.far.texture },
      uDetailExtents: { value: new Vector4(lakeResources.near.extentMeters, lakeResources.far.extentMeters, 0, 0) },
      uSkyTexture: { value: skyTexture },
      uWaveKAmplitude: { value: lakeResources.waveKAmplitude },
      uWaveOmegaPhase: { value: lakeResources.waveOmegaPhase },
      uWaveCount: { value: lakeResources.waveCount },
      uVisualWaveHeight: { value: lakeResources.visualWaveHeight },
      uWaterDark: { value: new Color(0x172831) },
      uWaterMid: { value: new Color(0x293b43) },
      uWaterLight: { value: new Color(0x50636a) },
      uSunDirection: { value: new Vector4(0.42, 0.82, 0.38, 0) }
    },
    vertexShader: lakeWaterVertexShader,
    fragmentShader: lakeWaterFragmentShader,
    depthWrite: true,
    depthTest: true
  });
  const lakeUniforms = {
    time: lakeUniform(waterMaterial, "uTimeSeconds", 0),
    windSpeed: lakeUniform(waterMaterial, "uWindSpeed", lakeResources.windSpeed),
    windVelocity: lakeUniform(waterMaterial, "uWindVelocity", new Vector4()),
    detailScale: lakeUniform(waterMaterial, "uDetailScale", 1),
    detailNear: lakeUniform(waterMaterial, "uDetailNear", lakeResources.near.texture),
    detailFar: lakeUniform(waterMaterial, "uDetailFar", lakeResources.far.texture),
    waveKAmplitude: lakeUniform(waterMaterial, "uWaveKAmplitude", lakeResources.waveKAmplitude),
    waveOmegaPhase: lakeUniform(waterMaterial, "uWaveOmegaPhase", lakeResources.waveOmegaPhase),
    waveCount: lakeUniform(waterMaterial, "uWaveCount", lakeResources.waveCount),
    visualWaveHeight: lakeUniform(waterMaterial, "uVisualWaveHeight", lakeResources.visualWaveHeight)
  };
  const water = new Mesh(waterGeometry, waterMaterial);
  water.frustumCulled = false;
  scene.add(water);

  const panelTexture = new CanvasTexture(panelCanvas);
  panelTexture.colorSpace = SRGBColorSpace;
  panelTexture.minFilter = LinearFilter;
  panelTexture.generateMipmaps = false;
  const panelMaterial = new MeshBasicMaterial({ map: panelTexture, side: DoubleSide });
  const panelGeometry = new PlaneGeometry(2.4, 1.8);
  const panelMesh = new Mesh(panelGeometry, panelMaterial);
  panelMesh.visible = false;
  scene.add(panelMesh);
  const gazeCursorGeometry = new CircleGeometry(0.035, 32);
  const gazeCursorMaterial = new MeshBasicMaterial({ color: 0xf0d382, side: DoubleSide, transparent: true, opacity: 0.8, depthWrite: false });
  const gazeCursor = new Mesh(gazeCursorGeometry, gazeCursorMaterial);
  gazeCursor.position.z = 0.015;
  gazeCursor.visible = false;
  panelMesh.add(gazeCursor);

  const aircraftRoot = new Group();
  scene.add(aircraftRoot);
  const airframe = createBirdmanAirframe();
  aircraftRoot.add(airframe.root);
  const camera = new PerspectiveCamera(60, 1, 0.05, 2000);
  aircraftRoot.add(camera);
  const externalCameraRig = new Group();
  scene.add(externalCameraRig);
  const fixedCamera = new PerspectiveCamera(60, 1, 0.05, 2000);
  externalCameraRig.add(fixedCamera);
  let disposed = false;
  let loopRunning = false;
  let width = 0;
  let height = 0;
  let pixelRatio = 0;
  let currentPanel = null as BackendFrame["panel"];
  let stereoPresentation: StereoPresentationProfile | null = null;
  let xrState: ThreeWebXrState = { type: "idle" };
  let requestGeneration = 0;
  let sessionEndHandler: (() => void) | null = null;
  let referenceSpaceResetHandler: ((previousReferenceFromNew: Pose | null) => void) | null = null;
  let activeReferenceSpace: XRReferenceSpace | null = null;
  let selectRayHandler: ((ray: SelectRay) => void) | null = null;
  let flightPose: FlightRenderPose | null = null;
  let flightCameraMode: FlightCameraMode = "pilot";
  let fixedCameraView: CinematicCameraView | null = null;
  const currentExternalCameraPose = (): Pose | null => {
    if (flightPose === null || flightCameraMode === "pilot") return null;
    if (flightCameraMode !== "chase" && fixedCameraView !== null) return fixedCameraView.pose;
    return flightRelativePose(flightPose, poseFrdToThree(replayCameraPoseFrd("chase")));
  };

  const onSelect = (event: XRInputSourceEvent): void => {
    if (xrState.type !== "active" && xrState.type !== "attaching") return;
    const referenceSpace = renderer.xr.getReferenceSpace();
    if (referenceSpace === null || selectRayHandler === null) return;
    const ray = selectRayFromXrEvent(event.frame, event.inputSource, referenceSpace);
    if (ray !== null) selectRayHandler(transformRay(ray, currentExternalCameraPose()));
  };

  const onSessionEnd = (): void => {
    const session = sessionFromState(xrState);
    if (session === null) return;
    session.removeEventListener("select", onSelect);
    const expected = xrState.type === "stopping";
    xrState = { type: "idle" };
    if (expected) return;
    queueMicrotask(() => {
      clearSession(session);
      sessionEndHandler?.();
    });
  };

  const onReferenceSpaceReset = (event: XRReferenceSpaceEvent): void => {
    const transform = event.transform as XRRigidTransform | null | undefined;
    referenceSpaceResetHandler?.(transform === null || transform === undefined ? null : {
      position: vec3(transform.position.x, transform.position.y, transform.position.z),
      orientation: quaternion(transform.orientation.w, transform.orientation.x, transform.orientation.y, transform.orientation.z)
    });
  };

  const rendererAdapter: RendererAdapter = {
    startLoop(callback) {
      ensureActive(disposed);
      if (loopRunning) throw new Error("Three.js frame loop is already active");
      loopRunning = true;
      renderer.setAnimationLoop((timestamp, xrFrame) => {
        const referenceSpace = renderer.xr.getReferenceSpace();
        const viewer = referenceSpace === null ? null : xrFrame.getViewerPose(referenceSpace);
        const transform = viewer?.transform;
        const viewerPose = transform === undefined ? null : {
          position: vec3(transform.position.x, transform.position.y, transform.position.z),
          orientation: quaternion(transform.orientation.w, transform.orientation.x, transform.orientation.y, transform.orientation.z)
        };
        callback(timestamp, viewerPose);
      });
    },
    stopLoop() {
      if (disposed || !loopRunning) return;
      renderer.setAnimationLoop(null);
      loopRunning = false;
    },
    render(frame: BackendFrame) {
      ensureActive(disposed);
      if (!renderer.xr.isPresenting) resizeIfNeeded(frame.viewport);
      const externalCameraPose = currentExternalCameraPose();
      const useExternalCamera = externalCameraPose !== null;
      const fixedView = useExternalCamera && flightCameraMode !== "chase" ? fixedCameraView : null;
      const pilotEyePose = flightPose === null
        ? IDENTITY_POSE
        : pilotEyePoseThree(
          SYNTHETIC_PILOT_EYE_POINT,
          flightPose.pilotPositionMeters,
          flightPose.initialPilotPositionMeters
        );
      setPose(camera, composePose(pilotEyePose, frame.cameraPose));
      const externalPose = externalCameraPose ?? IDENTITY_POSE;
      setPose(externalCameraRig, externalPose);
      setPose(fixedCamera, renderer.xr.isPresenting ? IDENTITY_POSE : frame.cameraPose);
      if (fixedView !== null && fixedCamera.fov !== fixedView.verticalFieldOfViewDegrees) {
        fixedCamera.fov = fixedView.verticalFieldOfViewDegrees;
        fixedCamera.updateProjectionMatrix();
      } else if (fixedView === null && fixedCamera.fov !== 60) {
        fixedCamera.fov = 60;
        fixedCamera.updateProjectionMatrix();
      }
      setPose(aircraftRoot, flightPose === null ? IDENTITY_POSE : flightRelativePose(flightPose, IDENTITY_POSE));
      airframe.setVisualState(
        flightPose?.airspeedMetersPerSecond ?? null,
        flightPose?.actuatorDeflectionRadians?.pitch ?? 0,
        flightPose?.actuatorDeflectionRadians?.yaw ?? 0
      );
      const simulationTimeSeconds = flightPose?.simulationTimeSeconds ?? 0;
      lakeUniforms.time.value = simulationTimeSeconds;
      // The non-flight scenic camera is at the origin. Keep its water below
      // eye level so amplified wave crests do not cut across the horizon.
      water.position.set(flightPose?.datumPositionNed.east ?? 0, flightPose === null ? -1.5 : 0, -(flightPose?.datumPositionNed.north ?? 0));
      if (useExternalCamera) {
        externalCameraRig.add(panelMesh);
        setPose(panelMesh, composePose(inversePose(externalPose), frame.panelPose));
      } else {
        scene.add(panelMesh);
        setPose(panelMesh, flightPose === null ? frame.panelPose : flightRelativePose(flightPose, frame.panelPose));
      }
      panelMesh.visible = frame.panelVisible;
      if (frame.panel !== currentPanel) {
        panelTexture.needsUpdate = true;
        currentPanel = frame.panel;
      }
      const panelWidth = frame.panel?.size.width ?? 2.4;
      const panelHeight = frame.panel?.size.height ?? 1.8;
      panelMesh.scale.set(panelWidth / 2.4, panelHeight / 1.8, 1);
      gazeCursor.visible = frame.gazeCursor !== null && frame.panelVisible;
      if (frame.gazeCursor !== null) {
        gazeCursor.position.set(frame.gazeCursor.point.x, frame.gazeCursor.point.y, 0.015);
        gazeCursor.scale.setScalar(Math.max(0.05, frame.gazeCursor.progress));
      }
      if (stereoPresentation !== null && !renderer.xr.isPresenting) {
        stereoEffect.render(scene, useExternalCamera ? fixedCamera : camera);
        renderer.setViewport(0, 0, width, height);
        renderer.setScissor(0, 0, width, height);
        renderer.setScissorTest(false);
      } else {
        renderer.render(scene, useExternalCamera ? fixedCamera : camera);
      }
    },
    resize(viewport: ViewportSize) {
      ensureActive(disposed);
      if (renderer.xr.isPresenting) return;
      resizeIfNeeded(viewport);
    },
    setStereoPresentation(profile: StereoPresentationProfile | null) {
      ensureActive(disposed);
      if (renderer.xr.isPresenting && profile !== null) throw new Error("Phone VR stereo cannot overlap an immersive WebXR session");
      if (profile !== null && (!Number.isFinite(profile.eyeSeparationMeters) || profile.eyeSeparationMeters <= 0 ||
          !Number.isFinite(profile.verticalFieldOfViewDegrees) || profile.verticalFieldOfViewDegrees <= 0 ||
          profile.verticalFieldOfViewDegrees >= 180 || !Number.isFinite(profile.focusDistanceMeters) || profile.focusDistanceMeters <= 0)) {
        throw new RangeError("Phone VR optical profile values must be positive and finite");
      }
      stereoPresentation = profile;
      camera.fov = profile?.verticalFieldOfViewDegrees ?? 60;
      camera.focus = profile?.focusDistanceMeters ?? 10;
      if (profile !== null) stereoEffect.setEyeSeparation(profile.eyeSeparationMeters);
      camera.updateProjectionMatrix();
    },
    dispose() {
      if (disposed) return;
      if (loopRunning) renderer.setAnimationLoop(null);
      loopRunning = false;
      panelTexture.dispose();
      gazeCursorGeometry.dispose();
      gazeCursorMaterial.dispose();
      panelGeometry.dispose();
      panelMaterial.dispose();
      waterGeometry.dispose();
      waterMaterial.dispose();
      skyTexture.dispose();
      lakeResources.near.texture.dispose();
      lakeResources.far.texture.dispose();
      airframe.dispose();
      renderer.dispose();
      disposed = true;
    },
    setSelectRayHandler(handler) {
      selectRayHandler = handler;
    },
    setFlightPose(pose: FlightRenderPose | null) {
      flightPose = pose;
    },
    setLakeVisualCondition(condition: LakeVisualCondition) {
      ensureActive(disposed);
      if (sameLakeVisualCondition(activeLakeCondition, condition)) return;
      const next = createLakeVisualResources(condition, lakeQuality);
      lakeUniforms.windSpeed.value = next.windSpeed;
      lakeUniforms.windVelocity.value.set(condition.windEastMetersPerSecond, -condition.windNorthMetersPerSecond, 0, 0);
      lakeUniforms.detailScale.value = condition.detailAmplitudeScale;
      lakeUniforms.detailNear.value = next.near.texture;
      lakeUniforms.detailFar.value = next.far.texture;
      lakeUniforms.waveKAmplitude.value = next.waveKAmplitude;
      lakeUniforms.waveOmegaPhase.value = next.waveOmegaPhase;
      lakeUniforms.waveCount.value = next.waveCount;
      lakeUniforms.visualWaveHeight.value = next.visualWaveHeight;
      lakeResources.near.texture.dispose();
      lakeResources.far.texture.dispose();
      lakeResources = next;
      activeLakeCondition = condition;
    },
    setFlightCameraMode(mode: FlightCameraMode) {
      flightCameraMode = mode;
    },
    setCinematicCameraView(view: CinematicCameraView | null) {
      fixedCameraView = view;
    },
    transformTrackingPose(pose: Pose): Pose {
      const externalPose = currentExternalCameraPose();
      return externalPose === null ? pose : composePose(externalPose, pose);
    }
  };

  const webxr: WebXrSessionPort = {
    async checkAvailability(): Promise<WebXrAvailability> {
      if (xrSystem === null) return { supported: false, message: "WebXR is unavailable in this browser" };
      try {
        const supported = await xrSystem.isSessionSupported("immersive-vr");
        return supported
          ? { supported: true, message: "WebXR immersive-vr is supported" }
          : { supported: false, message: "This browser or device does not support immersive-vr" };
      } catch (error) {
        return { supported: false, message: `WebXR capability check failed: ${errorMessage(error)}` };
      }
    },
    requestSessionFromUserGesture(): Promise<WebXrSessionRequest> {
      if (xrSystem === null) return Promise.resolve({ ok: false, message: "WebXR is unavailable in this browser" });
      if (xrState.type !== "idle" && xrState.type !== "failed") {
        return Promise.resolve({ ok: false, message: "A WebXR session is already active or starting" });
      }
      const generation = ++requestGeneration;
      xrState = { type: "requesting" };
      let request: Promise<XRSession>;
      try {
        request = xrSystem.requestSession("immersive-vr");
      } catch (error) {
        const message = `WebXR session request failed: ${errorMessage(error)}`;
        if (generation === requestGeneration) xrState = { type: "failed", message };
        return Promise.resolve({ ok: false, message });
      }
      return request.then(
        (session) => {
          if (generation !== requestGeneration || xrState.type !== "requesting") {
            void session.end().catch(() => undefined);
            return { ok: false, message: "WebXR session request was canceled" } as const;
          }
          xrState = { type: "requested", session };
          return { ok: true } as const;
        },
        (error: unknown) => {
          const message = `WebXR session request failed: ${errorMessage(error)}`;
          if (generation === requestGeneration && xrState.type === "requesting") xrState = { type: "failed", message };
          return { ok: false, message } as const;
        }
      );
    },
    async startSession(): Promise<void> {
      ensureActive(disposed);
      if (xrState.type !== "requested") throw new Error("WebXR session must be requested by an explicit user action");
      const session = xrState.session;
      xrState = { type: "attaching", session };
      session.addEventListener("end", onSessionEnd);
      session.addEventListener("select", onSelect);
      renderer.xr.enabled = true;
      renderer.xr.setReferenceSpaceType("local");
      try {
        await renderer.xr.setSession(session);
        if (!isAttaching()) throw new Error("WebXR session ended during startup");
        const referenceSpace = renderer.xr.getReferenceSpace();
        if (referenceSpace === null) throw new Error("WebXR reference space is unavailable");
        activeReferenceSpace = referenceSpace;
        activeReferenceSpace.addEventListener("reset", onReferenceSpaceReset);
        xrState = { type: "active", session };
      } catch (error) {
        if (xrState.type === "attaching") {
          xrState = { type: "stopping", session };
          try {
            await session.end();
          } catch (cleanupError) {
            xrState = { type: "failed", message: errorMessage(cleanupError) };
            throw new Error(`WebXR attach failed: ${errorMessage(error)}; session cleanup failed: ${errorMessage(cleanupError)}`, { cause: cleanupError });
          } finally {
            clearSession(session);
          }
        }
        throw error;
      }
    },
    async endSession(): Promise<void> {
      const state = xrState;
      if (state.type === "requesting") {
        requestGeneration++;
        xrState = { type: "idle" };
        return;
      }
      const session = sessionFromState(state);
      if (session === null) {
        xrState = { type: "idle" };
        return;
      }
      xrState = { type: "stopping", session };
      if (state.type === "active" || state.type === "attaching") session.removeEventListener("end", onSessionEnd);
      try {
        await session.end();
      } finally {
        clearSession(session);
      }
    },
    setSessionEndHandler(handler: (() => void) | null): void {
      sessionEndHandler = handler;
    },
    setReferenceSpaceResetHandler(handler: ((previousReferenceFromNew: Pose | null) => void) | null): void {
      referenceSpaceResetHandler = handler;
    }
  };

  return Object.freeze({
    renderer: rendererAdapter,
    webxr
  });

  function clearSession(session = sessionFromState(xrState)): void {
    if (session !== null) {
      session.removeEventListener("select", onSelect);
      session.removeEventListener("end", onSessionEnd);
    }
    activeReferenceSpace?.removeEventListener("reset", onReferenceSpaceReset);
    activeReferenceSpace = null;
    xrState = { type: "idle" };
    renderer.xr.enabled = false;
  }

  function isAttaching(): boolean {
    return xrState.type === "attaching";
  }

  function resizeIfNeeded(viewport: ViewportSize): void {
    if (viewport.x === width && viewport.y === height && viewport.pixelRatio === pixelRatio) return;
    width = viewport.x;
    height = viewport.y;
    pixelRatio = viewport.pixelRatio;
    renderer.setPixelRatio(pixelRatio);
    if (stereoPresentation !== null) stereoEffect.setSize(width, height);
    else renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    fixedCamera.aspect = width / height;
    fixedCamera.updateProjectionMatrix();
  }
}

function setPose(object: Object3D, pose: Pose): void {
  object.position.set(pose.position.x, pose.position.y, pose.position.z);
  object.quaternion.set(pose.orientation.x, pose.orientation.y, pose.orientation.z, pose.orientation.w);
}

function transformRay(ray: SelectRay, worldFromTracking: Pose | null): SelectRay {
  if (worldFromTracking === null) return ray;
  const offset = rotateVec3(worldFromTracking.orientation, ray.origin);
  return Object.freeze({
    origin: vec3(offset.x + worldFromTracking.position.x, offset.y + worldFromTracking.position.y, offset.z + worldFromTracking.position.z),
    direction: rotateVec3(worldFromTracking.orientation, ray.direction),
    timestampMs: ray.timestampMs
  });
}

function ensureActive(disposed: boolean): void {
  if (disposed) throw new Error("Three.js renderer has been disposed");
}

function lakeUniform<T>(material: ShaderMaterial, name: string, expectedValue: T): { value: T } {
  const uniform = material.uniforms[name];
  if (uniform === undefined) throw new Error(`Lake shader uniform is missing: ${name}`);
  if (typeof uniform.value !== typeof expectedValue) throw new TypeError(`Lake shader uniform has an unexpected value type: ${name}`);
  return uniform as { value: T };
}

function lakeVisualAmplitudeScale(waveNumberRadiansPerMeter: number): number {
  const shortWaveWeight = Math.max(0, Math.min(1, (waveNumberRadiansPerMeter - 1.5) / 4.5));
  const smoothWeight = shortWaveWeight * shortWaveWeight * (3 - 2 * shortWaveWeight);
  return 0.65 + 1.35 * smoothWeight;
}

function createLakeGeometry(segments: number): PlaneGeometry {
  const extent = 6000;
  const geometry = new PlaneGeometry(extent, extent, segments, segments);
  const positions = geometry.getAttribute("position");
  const spacing = new Float32Array(positions.count);
  const halfWidth = extent / 2;
  // Preserve the former near-eye spacing while extending the lake beyond the
  // camera far plane in every horizontal viewing direction.
  const exponent = 7.2;
  const denominator = Math.expm1(exponent);
  const map = (coordinate: number): number => Math.sign(coordinate) * halfWidth * Math.expm1(exponent * Math.abs(coordinate) / halfWidth) / denominator;
  const localStep = (coordinate: number): number => extent / segments * exponent * Math.exp(exponent * Math.abs(coordinate) / halfWidth) / denominator;
  for (let index = 0; index < positions.count; index++) {
    const x = positions.getX(index);
    const y = positions.getY(index);
    spacing[index] = Math.max(localStep(x), localStep(y));
    positions.setXY(index, map(x), map(y));
  }
  positions.needsUpdate = true;
  geometry.setAttribute("aGridSpacing", new BufferAttribute(spacing, 1));
  geometry.computeBoundingSphere();
  return geometry;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function sessionFromState(state: ThreeWebXrState): XRSession | null {
  switch (state.type) {
    case "requested":
    case "attaching":
    case "active":
    case "stopping":
      return state.session;
    case "idle":
    case "requesting":
    case "failed":
      return null;
  }
}

const lakeWavePacketShader = /* glsl */ `
float lakeWaveHash(vec2 cell) {
  return fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
}

vec3 lakeValueNoise(vec2 point) {
  vec2 cell = floor(point);
  vec2 fraction = fract(point);
  vec2 blend = fraction * fraction * (3.0 - 2.0 * fraction);
  vec2 blendGradient = 6.0 * fraction * (1.0 - fraction);
  float a = lakeWaveHash(cell);
  float b = lakeWaveHash(cell + vec2(1.0, 0.0));
  float c = lakeWaveHash(cell + vec2(0.0, 1.0));
  float d = lakeWaveHash(cell + vec2(1.0, 1.0));
  float value = mix(mix(a, b, blend.x), mix(c, d, blend.x), blend.y);
  vec2 gradient = vec2(
    blendGradient.x * mix(b - a, d - c, blend.y),
    blendGradient.y * mix(c - a, d - b, blend.x)
  );
  return vec3(value * 2.0 - 1.0, gradient * 2.0);
}

mat3 lakeWaveModulation(vec2 point, vec2 direction, float waveNumber, float seed) {
  vec2 across = vec2(-direction.y, direction.x);
  float frequency = max(0.11, waveNumber * 0.055);
  vec2 coordinate = vec2(dot(point, across), dot(point, direction)) * frequency + vec2(seed * 1.7, seed * 2.3);
  vec2 secondCoordinate = vec2(coordinate.x * 1.7 + coordinate.y * 0.37,
    coordinate.y * 1.9 - coordinate.x * 0.24) + vec2(13.7, -8.2);
  vec3 broad = lakeValueNoise(coordinate);
  vec3 fine = lakeValueNoise(secondCoordinate);
  vec2 broadGradient = frequency * (across * broad.y + direction * broad.z);
  vec2 fineGradient = frequency * (fine.y * (across * 1.7 + direction * 0.37)
    + fine.z * (across * -0.24 + direction * 1.9));
  // Shared, two-scale noise bends and interrupts crests without an empty mask.
  float packet = 1.0 + 0.38 * broad.x + 0.12 * fine.x;
  vec2 packetGradient = 0.38 * broadGradient + 0.12 * fineGradient;
  float offset = 4.6 * broad.x + 1.4 * fine.x;
  vec2 offsetGradient = 4.6 * broadGradient + 1.4 * fineGradient;
  return mat3(vec3(packet, packetGradient), vec3(offset, offsetGradient), vec3(0.0));
}
`;

const lakeWaterVertexShader = /* glsl */ `
uniform float uTimeSeconds;
uniform vec4 uWindVelocity;
uniform float uDetailScale;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform vec4 uDetailExtents;
uniform vec4 uWaveKAmplitude[24];
uniform vec4 uWaveOmegaPhase[24];
uniform int uWaveCount;
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying float vCrest;
varying float vCompression;
varying float vGridSpacing;
varying vec2 vBaseXZ;
varying vec2 vRepresentedSlope;
${lakeWavePacketShader}

void main() {
  // PlaneGeometry's XY winding points +Z; reversing its second axis while
  // mapping to world XZ keeps the resulting top face wound toward +Y.
  vec3 p = vec3(position.x, 0.0, -position.y);
  vec2 waveXZ = (modelMatrix * vec4(p, 1.0)).xz;
  vBaseXZ = waveXZ;
  vec2 detailDrift = uWindVelocity.xy * uTimeSeconds;
  vec4 nearDetail = texture2D(uDetailNear, (waveXZ - detailDrift * 0.42) / uDetailExtents.x);
  vec4 farDetail = texture2D(uDetailFar, (waveXZ - detailDrift * 0.73) / uDetailExtents.y);
  float nearHeight = nearDetail.a - 128.0 / 255.0;
  float farHeight = farDetail.a - 128.0 / 255.0;
  float nearVisibility = 1.0 - smoothstep(0.3, 0.9, aGridSpacing);
  float farVisibility = 1.0 - smoothstep(0.8, 2.8, aGridSpacing);
  p.y += (nearHeight * 0.8 * nearVisibility + farHeight * 0.4 * farVisibility) * uDetailScale;
  float slopeEnergy = 0.0;
  for (int i = 0; i < 24; i++) {
    if (i < uWaveCount) slopeEnergy += uWaveKAmplitude[i].z * uWaveKAmplitude[i].w;
  }
  // Limit horizontal displacement as the combined wave steepness rises.
  // Gerstner displacement gathers vertices near each sharp crest.
  float q = min(4.5, 0.56 / max(3.1 * slopeEnergy, 0.001));
  float dxx = 1.0;
  float dxz = 0.0;
  float dzx = 0.0;
  float dzz = 1.0;
  float dhdx = 0.0;
  float dhdz = 0.0;
  vec2 nearSlope = (nearDetail.rg * 255.0 - 128.0) / 127.0;
  vec2 farSlope = (farDetail.rg * 255.0 - 128.0) / 127.0;
  vec2 resolvedDetailSlope = (nearSlope * (0.8 * nearVisibility)
    + farSlope * (0.4 * farVisibility)) * uDetailScale;
  dhdx += resolvedDetailSlope.x;
  dhdz += resolvedDetailSlope.y;
  float crest = 0.0;
  for (int i = 0; i < 24; i++) {
    if (i < uWaveCount) {
      vec4 ka = uWaveKAmplitude[i];
      vec4 op = uWaveOmegaPhase[i];
      vec2 direction = ka.xy;
      float k = ka.z;
      float geometricVisibility = 1.0 - smoothstep(1.3, 2.5, k * aGridSpacing);
      mat3 modulation = lakeWaveModulation(waveXZ, direction, k, op.y);
      vec3 packet = modulation[0];
      vec3 warp = modulation[1];
      float baseAmplitude = ka.w * geometricVisibility;
      float a = baseAmplitude * packet.x;
      vec2 amplitudeGradient = baseAmplitude * packet.yz;
      float phase = k * dot(direction, waveXZ) + warp.x - op.x * uTimeSeconds + op.y;
      vec2 phaseGradient = k * direction + warp.yz;
      float s = sin(phase);
      float c = cos(phase);
      // A cubic offset sine gives a narrow crest and a broad trough. 0.625 is
      // twice the cycle mean of ((1 + sin(phase)) / 2)^3, so mean height is zero.
      float crestBasis = 0.5 + 0.5 * s;
      p.xz += q * a * direction * c;
      p.y += a * (2.0 * crestBasis * crestBasis * crestBasis - 0.625);
      vec2 horizontalGradient = q * (c * amplitudeGradient - a * s * phaseGradient);
      dxx += direction.x * horizontalGradient.x;
      dxz += direction.x * horizontalGradient.y;
      dzx += direction.y * horizontalGradient.x;
      dzz += direction.y * horizontalGradient.y;
      vec2 heightGradient = a * 3.0 * crestBasis * crestBasis * c * phaseGradient
        + (2.0 * crestBasis * crestBasis * crestBasis - 0.625) * amplitudeGradient;
      dhdx += heightGradient.x;
      dhdz += heightGradient.y;
      crest += s * k * a;
    }
  }
  float horizontalDeterminant = dxx * dzz - dxz * dzx;
  // The geometric normal is measured after Gerstner horizontal displacement.
  // Convert the represented detail slope into that same coordinate system.
  vRepresentedSlope = vec2(
    dzz * resolvedDetailSlope.x - dzx * resolvedDetailSlope.y,
    -dxz * resolvedDetailSlope.x + dxx * resolvedDetailSlope.y
  ) / max(horizontalDeterminant, 0.01);
  vec3 tangentX = vec3(dxx, dhdx, dzx);
  vec3 tangentZ = vec3(dxz, dhdz, dzz);
  vec3 localNormal = normalize(cross(tangentZ, tangentX));
  vec4 worldPosition = modelMatrix * vec4(p, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * localNormal);
  vCrest = crest / max(slopeEnergy, 0.001);
  vCompression = clamp(1.0 - horizontalDeterminant, 0.0, 1.0);
  vGridSpacing = aGridSpacing;
  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
`;

const lakeWaterFragmentShader = /* glsl */ `
uniform float uWindSpeed;
uniform vec4 uWindVelocity;
uniform float uDetailScale;
uniform float uVisualWaveHeight;
uniform float uTimeSeconds;
uniform vec4 uWaveKAmplitude[24];
uniform vec4 uWaveOmegaPhase[24];
uniform int uWaveCount;
uniform vec3 uWaterDark;
uniform vec3 uWaterMid;
uniform vec3 uWaterLight;
uniform vec4 uSunDirection;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform sampler2D uSkyTexture;
uniform vec4 uDetailExtents;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying float vCrest;
varying float vCompression;
varying float vGridSpacing;
varying vec2 vBaseXZ;
varying vec2 vRepresentedSlope;
${lakeWavePacketShader}

vec3 lakeSkyRadiance(vec3 ray) {
  vec3 skyRay = vec3(ray.x, max(ray.y, 0.0), ray.z);
  skyRay = dot(skyRay, skyRay) < 1e-8 ? vec3(1.0, 0.0, 0.0) : normalize(skyRay);
  vec2 skyUv = vec2(atan(skyRay.z, skyRay.x) / 6.2831853 + 0.5,
    asin(skyRay.y) / 3.1415927 + 0.5);
  return texture2D(uSkyTexture, skyUv).rgb;
}

float lakeNoise(vec2 point) {
  vec2 cell = floor(point);
  vec2 blend = fract(point);
  blend = blend * blend * (3.0 - 2.0 * blend);
  float a = fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
  float b = fract(sin(dot(cell + vec2(1.0, 0.0), vec2(127.1, 311.7))) * 43758.5453);
  float c = fract(sin(dot(cell + vec2(0.0, 1.0), vec2(127.1, 311.7))) * 43758.5453);
  float d = fract(sin(dot(cell + vec2(1.0, 1.0), vec2(127.1, 311.7))) * 43758.5453);
  return mix(mix(a, b, blend.x), mix(c, d, blend.x), blend.y);
}

vec2 lakeWaveSlope(vec2 worldXZ) {
  // Add only wave detail that the vertex grid cannot resolve. Derivative based
  // filtering fades subpixel wavelengths smoothly as they recede from the eye.
  float waveSlopeX = 0.0;
  float waveSlopeZ = 0.0;
  for (int i = 0; i < 24; i++) {
    if (i < uWaveCount) {
      vec4 ka = uWaveKAmplitude[i];
      vec4 op = uWaveOmegaPhase[i];
      float k = ka.z;
      vec2 direction = ka.xy;
      mat3 modulation = lakeWaveModulation(worldXZ, direction, k, op.y);
      vec3 warp = modulation[1];
      float phase = k * dot(direction, worldXZ) + warp.x - op.x * uTimeSeconds + op.y;
      vec2 phaseGradient = k * direction + warp.yz;
      float geometricVisibility = 1.0 - smoothstep(1.3, 2.5, k * vGridSpacing);
      float pixelVisibility = 1.0 - smoothstep(2.0, 5.0, fwidth(phase));
      // Transfer waves that outgrow the mesh to the pixel normal. A weak
      // transfer creates an empty band wherever geometric detail ends.
      float normalWeight = (1.0 - geometricVisibility) * pixelVisibility * 0.65;
      float crestBasis = 0.5 + 0.5 * sin(phase);
      vec3 packet = modulation[0];
      vec2 gradient = ka.w * (packet.x * 3.0 * crestBasis * crestBasis * cos(phase) * phaseGradient
        + (2.0 * crestBasis * crestBasis * crestBasis - 0.625) * packet.yz) * normalWeight;
      waveSlopeX += gradient.x;
      waveSlopeZ += gradient.y;
    }
  }
  return vec2(waveSlopeX, waveSlopeZ);
}

vec3 lakeMicroDetail(vec2 worldXZ) {
  const float fineAmplitude = 0.16;
  const float rippleAmplitude = 0.055;
  vec2 drift = uWindVelocity.xy * uTimeSeconds;
  vec4 nearSample = texture2D(uDetailNear, (worldXZ - drift * 0.42) / uDetailExtents.x);
  // These compact gravity-wave bands use phase-speed ratios proportional to
  // sqrt(wavelength): far/near/fine/ripple = 0.73/0.42/0.26/0.18.
  vec4 farSample = texture2D(uDetailFar, (worldXZ - drift * 0.73) / uDetailExtents.y);
  // Each rotated octave advects along its rotated propagation direction.
  // Reusing the unrotated drift makes oblique crests slide sideways.
  vec2 fineDrift = vec2(
    0.6 * drift.x + 0.8 * drift.y,
    -0.8 * drift.x + 0.6 * drift.y
  ) * 0.26;
  vec2 finePoint = (worldXZ - fineDrift) * 2.6;
  vec2 fineRotated = vec2(
    0.6 * finePoint.x - 0.8 * finePoint.y,
    0.8 * finePoint.x + 0.6 * finePoint.y
  );
  vec4 fineSample = texture2D(uDetailNear, fineRotated / uDetailExtents.x + vec2(0.217, 0.631));
  vec2 rippleDrift = vec2(
    0.8 * drift.x + 0.6 * drift.y,
    -0.6 * drift.x + 0.8 * drift.y
  ) * 0.18;
  vec2 ripplePoint = (worldXZ - rippleDrift) * 5.2;
  vec2 rippleRotated = vec2(
    0.8 * ripplePoint.x - 0.6 * ripplePoint.y,
    0.6 * ripplePoint.x + 0.8 * ripplePoint.y
  );
  vec4 rippleSample = texture2D(uDetailNear, rippleRotated / uDetailExtents.x + vec2(0.683, 0.173));
  vec2 nearSlope = (nearSample.rg * 255.0 - 128.0) / 127.0;
  vec2 farSlope = (farSample.rg * 255.0 - 128.0) / 127.0;
  vec2 fineRawSlope = (fineSample.rg * 255.0 - 128.0) / 127.0;
  vec2 rippleRawSlope = (rippleSample.rg * 255.0 - 128.0) / 127.0;
  vec2 fineSlope = vec2(
    0.6 * fineRawSlope.x + 0.8 * fineRawSlope.y,
    -0.8 * fineRawSlope.x + 0.6 * fineRawSlope.y
  ) * 2.6 * fineAmplitude;
  vec2 rippleSlope = vec2(
    0.8 * rippleRawSlope.x + 0.6 * rippleRawSlope.y,
    -0.6 * rippleRawSlope.x + 0.8 * rippleRawSlope.y
  ) * 5.2 * rippleAmplitude;
  vec2 microSlope = nearSlope * 0.8 + farSlope * 0.4 + fineSlope + rippleSlope;
  // Blue stores slope squared. Mipmaps preserve the variance of unresolved
  // wavelets even after their mean slope approaches zero.
  float nearVariance = max(nearSample.b / 2.5 - dot(nearSlope, nearSlope), 0.0);
  float farVariance = max(farSample.b / 2.5 - dot(farSlope, farSlope), 0.0);
  float fineVariance = max(fineSample.b / 2.5 - dot(fineRawSlope, fineRawSlope), 0.0);
  float rippleVariance = max(rippleSample.b / 2.5 - dot(rippleRawSlope, rippleRawSlope), 0.0);
  float slopeMoment = dot(microSlope, microSlope)
    + 0.8 * 0.8 * nearVariance + 0.4 * 0.4 * farVariance
    + 2.6 * 2.6 * fineAmplitude * fineAmplitude * fineVariance
    + 5.2 * 5.2 * rippleAmplitude * rippleAmplitude * rippleVariance;
  // Preserve visible fine-scale contrast as the pilot eye rises above the
  // water; projected waves otherwise lose nearly all of their normal detail.
  float eyeHeight = max(cameraPosition.y - vWorldPosition.y, 0.0);
  float detailGain = 1.0 + 2.0 * smoothstep(1.5, 7.0, eyeHeight);
  float strength = smoothstep(0.05, 0.6, uWindSpeed) * detailGain;
  float scaledStrength = strength * uDetailScale;
  return vec3(microSlope * scaledStrength, slopeMoment * scaledStrength * scaledStrength);
}

vec3 sampleLakeColor(vec2 worldXZ, vec2 waveSlope, vec3 microDetail) {
  vec3 geometricNormal = normalize(vWorldNormal);
  float patches = lakeNoise(worldXZ * 0.17 + vec2(2.3, -7.1));
  vec2 microSlope = microDetail.xy;
  float unresolvedVariance = max(microDetail.z - dot(microSlope, microSlope), 0.0);
  // Subtract the exact vertex slope already represented by the geometric
  // normal; vertex LOD0 and filtered fragment samples need not be identical.
  vec2 geometricSlope = -geometricNormal.xz / max(geometricNormal.y, 0.01);
  vec2 totalSlope = geometricSlope + waveSlope + microSlope - vRepresentedSlope;
  vec3 normal = normalize(vec3(-totalSlope.x, 1.0, -totalSlope.y));
  vec3 viewDirection = normalize(cameraPosition - vec3(worldXZ.x, vWorldPosition.y, worldXZ.y));
  vec3 reflectedDirection = reflect(-viewDirection, normal);
  vec3 reflection = lakeSkyRadiance(reflectedDirection);
  // Normal-map facets facing away from the eye are not visible. Use the
  // resolved surface for the Fresnel weight while retaining normal detail
  // in the reflected direction until a visibility-aware BRDF is available.
  float ndv = max(dot(geometricNormal, viewDirection), 0.0);
  float fresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
  float sunAlignment = max(dot(reflectedDirection, normalize(uSunDirection.xyz)), 0.0);
  float roughness = clamp(0.23 + uWindSpeed * 0.018 + sqrt(unresolvedVariance) * 0.6, 0.23, 0.62);
  float glitterLobe = pow(sunAlignment, mix(220.0, 45.0, roughness));
  float glitterNoise = 0.35 + 0.65 * clamp(sqrt(microDetail.z) * 3.0, 0.0, 1.0);
  float glitter = glitterLobe * glitterNoise * smoothstep(0.35, 2.0, uWindSpeed);
  float diffuse = 0.58 + 0.42 * max(dot(normal, normalize(vec3(-uSunDirection.x, uSunDirection.y, -uSunDirection.z))), 0.0);
  float sharpCrest = smoothstep(0.42, 0.76, vCompression);
  float crestLight = smoothstep(0.05, 1.05, vCrest) * sharpCrest * mix(0.35, 1.0, patches);
  vec3 base = mix(uWaterDark, uWaterMid, diffuse * 0.72);
  base = mix(base, uWaterLight, crestLight * 0.16 + patches * 0.07);
  // Existing wave groups may remain steep after the local wind weakens.
  float whitecap = smoothstep(0.68, 0.84, vCompression) * smoothstep(0.7, 0.98, vCrest) * 0.12;
  // Dark water absorption must not also darken the reflected sky.
  base *= vec3(0.29, 0.34, 0.37);
  // Art-directed reflection strength for the dark reference water. This is
  // not a rough-surface BRDF and leaves a visible sky/water boundary.
  vec3 color = mix(base, reflection, fresnel * 0.45);
  float depthTint = clamp(uVisualWaveHeight / 0.35, 0.0, 1.0);
  color = mix(color, color * vec3(0.92, 0.97, 1.04), depthTint * 0.12);
  float distanceToEye = length(cameraPosition.xz - worldXZ);
  color += vec3(1.0, 0.78, 0.52) * glitter * 0.15;
  color = mix(color, vec3(0.38, 0.43, 0.43), whitecap * 0.28);
  // Airlight belongs after the water tint. Tinting the haze made the horizon
  // darker than the foreground and produced a black horizontal band.
  float atmosphericFade = smoothstep(300.0, 1600.0, distanceToEye);
  vec3 hazeColor = vec3(0.12, 0.18, 0.20);
  color = mix(color, hazeColor, atmosphericFade * 0.16);
  return color;
}

void main() {
  vec2 footprintX = dFdx(vBaseXZ) * 0.25;
  vec2 footprintY = dFdy(vBaseXZ) * 0.25;
  vec2 center = vBaseXZ;
  vec2 waveSlope = lakeWaveSlope(center);
  // Supersample subpixel normal detail, then shade the filtered normal once.
  vec3 microDetail = (
    lakeMicroDetail(center - footprintX - footprintY) +
    lakeMicroDetail(center + footprintX - footprintY) +
    lakeMicroDetail(center - footprintX + footprintY) +
    lakeMicroDetail(center + footprintX + footprintY)
  ) * 0.25;
  vec3 color = sampleLakeColor(vWorldPosition.xz, waveSlope, microDetail);
  gl_FragColor = vec4(color, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;
