import {
  AmbientLight,
  BufferAttribute,
  CanvasTexture,
  CircleGeometry,
  Color,
  DoubleSide,
  Float32BufferAttribute,
  Group,
  Fog,
  LinearFilter,
  DirectionalLight,
  Matrix4,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PlaneGeometry,
  Scene,
  SRGBColorSpace,
  StereoCamera,
  ShaderMaterial,
  UniformsLib,
  UniformsUtils,
  Vector3,
  Vector4,
  WebGLRenderer,
  WebGLRenderTarget
} from "three";
import { StereoEffect } from "three/addons/effects/StereoEffect.js";
import type { Object3D } from "three";
import type { BackendFrame, FlightCameraMode, FlightRenderPose, RendererAdapter, SelectRay, StereoPresentationProfile, ViewportSize } from "../../contracts/runtime.js";
import type { CinematicCameraView } from "../../contracts/camera.js";
import { uiPanelComposition } from "../../contracts/ui.js";
import { IDENTITY_POSE, multiplyQuaternion, pose, quaternion, rotateVec3, vec3 } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import { composePose, inversePose } from "../../contracts/math.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "../../../presentation/webxr-contracts.js";
import { selectRayFromXrEvent } from "./xr-select-ray.js";
import { captureConfiguredViewerFrame, captureXrViewerFrame } from "./viewer-frame.js";
import { createHeadHudSurface } from "./head-hud-surface.js";
import { unavailableViewerFrame } from "../../contracts/viewer-frame.js";
import { flightRelativePose } from "./flight-pose.js";
import { pilotEyePoseThree, poseFrdToThree, SYNTHETIC_PILOT_EYE_POINT } from "../../camera/pilot-eye-point.js";
import { replayCameraPoseFrd } from "../../camera/replay-camera.js";
import { createLakeWaveSpectrum, DEFAULT_LAKE_VISUAL_CONDITION, lakeWaterQualityProfile, selectLakeWaveComponentsForQuality } from "../../contracts/lake-water.js";
import type { LakeVisualCondition, LakeWaterQuality } from "../../contracts/lake-water.js";
import { createLakeDetailLayer } from "./lake-detail-texture.js";
import type { LakeDetailLayer } from "./lake-detail-texture.js";
import { createLakeSkyTexture } from "./lake-sky-texture.js";
import { createBirdmanAirframe } from "./birdman-airframe.js";
import { createLakeVenue } from "./lake-venue-mesh.js";

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
// Place Takeshima just outside the centered title panel at every viewport
// aspect ratio. Its bearing is computed from the OSM island outline.
const TITLE_SCREEN_ISLAND_BEARING_RADIANS = 272.16851686166876 * Math.PI / 180;
const TITLE_SCREEN_VERTICAL_FOV_RADIANS = 60 * Math.PI / 180;
const TITLE_SCREEN_PANEL_MAX_WIDTH_PIXELS = 42 * 16;
const TITLE_SCREEN_PANEL_ROOT_PADDING_PIXELS = 32;
const TITLE_SCREEN_PANEL_ISLAND_GAP_PIXELS = 24;
const TITLE_SCREEN_PITCH_HALF_RADIANS = 3 * Math.PI / 180;
const PANEL_OVERLAY_RENDER_ORDER = 2000;

export function titleScreenCameraPoseForViewport(width: number, height: number): Pose {
  if (!Number.isFinite(width) || width <= 0 || !Number.isFinite(height) || height <= 0) {
    throw new RangeError("Viewport dimensions must be positive");
  }
  const aspectRatio = width / height;
  const horizontalHalfFov = Math.atan(Math.tan(TITLE_SCREEN_VERTICAL_FOV_RADIANS / 2) * aspectRatio);
  const panelWidth = Math.min(TITLE_SCREEN_PANEL_MAX_WIDTH_PIXELS, width - TITLE_SCREEN_PANEL_ROOT_PADDING_PIXELS);
  const panelHalfWidthNdc = panelWidth / width;
  const targetIslandX = -Math.min(0.98, panelHalfWidthNdc + TITLE_SCREEN_PANEL_ISLAND_GAP_PIXELS / width);
  const islandRelativeBearing = Math.atan(targetIslandX * Math.tan(horizontalHalfFov));
  const yaw = islandRelativeBearing - TITLE_SCREEN_ISLAND_BEARING_RADIANS;
  return pose(
    vec3(0, 12, 0),
    multiplyQuaternion(
      quaternion(Math.cos(yaw / 2), 0, Math.sin(yaw / 2), 0),
      quaternion(Math.cos(TITLE_SCREEN_PITCH_HALF_RADIANS), -Math.sin(TITLE_SCREEN_PITCH_HALF_RADIANS), 0, 0)
    )
  );
}
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
  const near = createLakeDetailLayer(64, 3150, 1717 + condition.patternSeed * 997, directionX, directionZ);
  let far: LakeDetailLayer;
  try {
    far = createLakeDetailLayer(288, 5400, 2917 + condition.patternSeed * 991, directionX, directionZ,
      { count: 900, featureScaleMeters: 4.5, heightScale: 0.5 });
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
  lakeQuality: LakeWaterQuality = "high",
  headHudCanvas?: HTMLCanvasElement
): ThreeRendererBundle {
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
  renderer.shadowMap.enabled = true;
  const stereoEffect = new StereoEffect(renderer);
  const geometryStereo = new StereoCamera();
  geometryStereo.aspect = 0.5;
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setClearColor(0x9fb0ad, 1);

  const scene = new Scene();
  const skyTexture = createLakeSkyTexture();
  scene.background = skyTexture;
  const lakeFog = new Fog(0x9aafb1, 14_000, 80_000);
  scene.fog = lakeFog;
  const ambient = new AmbientLight(0xdceaf0, 1.15);
  ambient.layers.enable(1);
  scene.add(ambient);
  const sun = new DirectionalLight(0xffefd8, 1.45);
  sun.position.set(-20, 35, -18);
  sun.layers.enable(1);
  sun.castShadow = true;
  sun.shadow.mapSize.set(2048, 2048);
  sun.shadow.camera.left = -28;
  sun.shadow.camera.right = 28;
  sun.shadow.camera.top = 28;
  sun.shadow.camera.bottom = -28;
  sun.shadow.camera.near = 0.5;
  sun.shadow.camera.far = 160;
  sun.shadow.normalBias = 0.025;
  sun.shadow.bias = -0.0001;
  sun.shadow.radius = 5;
  scene.add(sun);
  scene.add(sun.target);
  const sunOffset = new Vector3(-20, 35, -18);

  const waterQuality = lakeWaterQualityProfile(lakeQuality);
  const waterGeometry = createLakeGeometry(waterQuality.meshSegments);
  let activeLakeCondition = DEFAULT_LAKE_VISUAL_CONDITION;
  let lakeResources = createLakeVisualResources(activeLakeCondition, lakeQuality);
  const waterMaterial = new ShaderMaterial({
    lights: true,
    uniforms: {
      ...UniformsUtils.clone(UniformsLib.lights),
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
      uHazeColor: { value: lakeFog.color },
      uHazeRange: { value: new Vector4(lakeFog.near, lakeFog.far, 0, 0) },
      uSunDirection: { value: new Vector4(-0.42, 0.82, -0.38, 0) },
      uReflectionTexture: { value: null },
      uReflectionMatrix: { value: new Matrix4() },
      uReflectionEnabled: { value: 0 },
      uWaterBackingClip: { value: 0 },
      uWaterBackingCenter: { value: new Vector4(0, 0, 0, 0) },
      // Leave a 100 m overlap behind the foreground mesh for its small
      // horizontal wave displacement at the last vertices.
      uWaterBackingInnerExtent: { value: 2900 }
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
  const farWaterGeometry = new PlaneGeometry(160_000, 160_000);
  farWaterGeometry.setAttribute("aGridSpacing", new Float32BufferAttribute([1_200, 1_200, 1_200, 1_200], 1));
  const farWaterUniforms = {
    ...UniformsUtils.clone(UniformsLib.lights),
    uTimeSeconds: lakeUniform(waterMaterial, "uTimeSeconds", 0),
    uWindSpeed: lakeUniform(waterMaterial, "uWindSpeed", lakeResources.windSpeed),
    uWindVelocity: lakeUniform(waterMaterial, "uWindVelocity", new Vector4()),
    uDetailScale: lakeUniform(waterMaterial, "uDetailScale", 1),
    uDetailNear: lakeUniform(waterMaterial, "uDetailNear", lakeResources.near.texture),
    uDetailFar: lakeUniform(waterMaterial, "uDetailFar", lakeResources.far.texture),
    uDetailExtents: lakeUniform(waterMaterial, "uDetailExtents", new Vector4()),
    uSkyTexture: lakeUniform(waterMaterial, "uSkyTexture", skyTexture),
    uWaveKAmplitude: lakeUniform(waterMaterial, "uWaveKAmplitude", lakeResources.waveKAmplitude),
    uWaveOmegaPhase: lakeUniform(waterMaterial, "uWaveOmegaPhase", lakeResources.waveOmegaPhase),
    uWaveCount: lakeUniform(waterMaterial, "uWaveCount", lakeResources.waveCount),
    uVisualWaveHeight: lakeUniform(waterMaterial, "uVisualWaveHeight", lakeResources.visualWaveHeight),
    uWaterDark: lakeUniform(waterMaterial, "uWaterDark", new Color(0x172831)),
    uWaterMid: lakeUniform(waterMaterial, "uWaterMid", new Color(0x293b43)),
    uWaterLight: lakeUniform(waterMaterial, "uWaterLight", new Color(0x50636a)),
    uHazeColor: lakeUniform(waterMaterial, "uHazeColor", lakeFog.color),
    uHazeRange: lakeUniform(waterMaterial, "uHazeRange", new Vector4()),
    uSunDirection: lakeUniform(waterMaterial, "uSunDirection", new Vector4()),
    uReflectionMatrix: lakeUniform(waterMaterial, "uReflectionMatrix", new Matrix4()),
    uReflectionTexture: { value: null },
    uReflectionEnabled: { value: 0 },
    uWaterBackingCenter: { value: new Vector4() },
    uWaterBackingInnerExtent: { value: 2900 },
    uWaterBackingClip: { value: 1 }
  };
  const farWaterMaterial = new ShaderMaterial({
    lights: true,
    uniforms: farWaterUniforms,
    vertexShader: lakeFilteredWaterVertexShader,
    fragmentShader: lakeWaterFragmentShader,
    side: DoubleSide,
    depthWrite: false,
    depthTest: true
  });
  const farWaterBackingCenter = lakeUniform(farWaterMaterial, "uWaterBackingCenter", new Vector4()).value;
  const farWater = new Mesh(farWaterGeometry, farWaterMaterial);
  farWater.name = "lake-biwa-distant-water-backing";
  // Draw the non-depth-writing backing first so the animated mesh always
  // shades the intentional overlap region afterward.
  farWater.renderOrder = -1;
  // Use the same datum as the animated foreground and shoreline transition.
  farWater.position.y = 0;
  farWater.frustumCulled = false;
  scene.add(farWater);
  const water = new Mesh(waterGeometry, waterMaterial);
  water.frustumCulled = false;
  water.receiveShadow = true;
  scene.add(water);
  const venue = createLakeVenue();
  scene.add(venue.group);
  const reflectionTarget = new WebGLRenderTarget(512, 256, {
    minFilter: LinearFilter,
    magFilter: LinearFilter,
    depthBuffer: true
  });
  reflectionTarget.texture.colorSpace = SRGBColorSpace;
  const reflectionTextureUniform = waterMaterial.uniforms.uReflectionTexture;
  if (reflectionTextureUniform === undefined) throw new Error("Lake reflection texture uniform is missing");
  reflectionTextureUniform.value = reflectionTarget.texture;
  const reflectionCamera = new PerspectiveCamera();
  reflectionCamera.layers.set(1);
  const reflectedPosition = new Vector3();
  const reflectedForward = new Vector3();
  const reflectedUp = new Vector3();
  const reflectedTarget = new Vector3();
  const reflectionMatrix = lakeUniform(waterMaterial, "uReflectionMatrix", new Matrix4()).value;
  const reflectionEnabled = lakeUniform(waterMaterial, "uReflectionEnabled", 0);
  const savedClearColor = new Color();

  const panelTexture = new CanvasTexture(panelCanvas);
  panelTexture.colorSpace = SRGBColorSpace;
  panelTexture.minFilter = LinearFilter;
  panelTexture.generateMipmaps = false;
  const panelMaterial = new MeshBasicMaterial({ map: panelTexture, side: DoubleSide, forceSinglePass: true });
  const panelGeometry = new PlaneGeometry(2.4, 1.8);
  const panelMesh = new Mesh(panelGeometry, panelMaterial);
  panelMesh.visible = false;
  scene.add(panelMesh);
  const headHudSurface = headHudCanvas === undefined ? null : createHeadHudSurface(headHudCanvas);
  if (headHudSurface !== null) scene.add(headHudSurface.mesh);
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
  aircraftRoot.layers.enable(1);
  airframe.root.traverse((object) => {
    object.layers.enable(1);
    if (object instanceof Mesh) {
      // Transparent film admits light; its ribs, spar and opaque skin cast the silhouette.
      const material = (object as Mesh).material;
      object.castShadow = (Array.isArray(material) ? material : [material]).every((part) => !part.transparent);
    }
  });
  const trackingOrigin = new Group();
  aircraftRoot.add(trackingOrigin);
  const camera = new PerspectiveCamera(60, 1, 0.05, 100_000);
  trackingOrigin.add(camera);
  let titleCameraPose = titleScreenCameraPoseForViewport(1, 1);
  const externalCameraRig = new Group();
  scene.add(externalCameraRig);
  const fixedCamera = new PerspectiveCamera(60, 1, 0.05, 100_000);
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
  let framePhase: "idle" | "physics" | "view" = "idle";
  let frameViewport: ViewportSize | null = null;
  let renderedTrackingMount: Pose | null = null;
  type PendingViewInputs = Readonly<{
    flightPose?: FlightRenderPose | null;
    cameraMode?: FlightCameraMode;
    cameraView?: CinematicCameraView | null;
    stereo?: StereoPresentationProfile | null;
    viewport?: ViewportSize;
    lake?: LakeVisualCondition;
  }>;
  let pendingViewInputs: PendingViewInputs | null = null;
  const stageViewInputs = (inputs: PendingViewInputs): void => {
    pendingViewInputs = Object.freeze({ ...pendingViewInputs, ...inputs });
  };
  const currentExternalCameraPose = (): Pose | null => {
    if (flightPose === null || flightCameraMode === "pilot") return null;
    if (flightCameraMode !== "chase" && fixedCameraView !== null) return fixedCameraView.pose;
    return flightRelativePose(flightPose, poseFrdToThree(replayCameraPoseFrd("chase")));
  };
  const currentPilotEyePose = (): Pose => flightPose === null
    ? IDENTITY_POSE
    : pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, flightPose.pilotPositionMeters, flightPose.initialPilotPositionMeters);
  const currentTrackingMountPose = (): Pose => currentExternalCameraPose() ?? currentPilotEyePose();

  const onSelect = (event: XRInputSourceEvent): void => {
    if (xrState.type !== "active" && xrState.type !== "attaching") return;
    const referenceSpace = renderer.xr.getReferenceSpace();
    if (referenceSpace === null || selectRayHandler === null || renderedTrackingMount === null) return;
    const ray = selectRayFromXrEvent(event.frame, event.inputSource, referenceSpace);
    if (ray !== null) selectRayHandler(transformRay(ray, renderedTrackingMount));
  };

  const onSessionEnd = (): void => {
    const session = sessionFromState(xrState);
    if (session === null) return;
    session.removeEventListener("select", onSelect);
    const expected = xrState.type === "stopping";
    xrState = { type: "idle" };
    renderedTrackingMount = null;
    if (expected) return;
    queueMicrotask(() => {
      clearSession(session);
      sessionEndHandler?.();
    });
  };

  const onReferenceSpaceReset = (event: XRReferenceSpaceEvent): void => {
    const transform = event.transform as XRRigidTransform | null | undefined;
    const previousReferenceFromNew = transform === null || transform === undefined ? null : {
      position: vec3(transform.position.x, transform.position.y, transform.position.z),
      orientation: quaternion(transform.orientation.w, transform.orientation.x, transform.orientation.y, transform.orientation.z)
    };
    const mount = renderedTrackingMount ?? currentTrackingMountPose();
    referenceSpaceResetHandler?.(previousReferenceFromNew === null ? null : composePose(
      composePose(mount, previousReferenceFromNew), inversePose(mount)
    ));
  };

  const rendererAdapter: RendererAdapter = {
    startLoop(callback) {
      ensureActive(disposed);
      if (loopRunning) throw new Error("Three.js frame loop is already active");
      loopRunning = true;
      renderer.setAnimationLoop((timestamp: number, xrFrame: XRFrame | null | undefined) => {
        const pending = pendingViewInputs;
        pendingViewInputs = null;
        if (pending !== null) {
          if ("flightPose" in pending) rendererAdapter.setFlightPose(pending.flightPose ?? null);
          if (pending.cameraMode !== undefined) rendererAdapter.setFlightCameraMode(pending.cameraMode);
          if ("cameraView" in pending) rendererAdapter.setCinematicCameraView(pending.cameraView ?? null);
          if ("stereo" in pending) rendererAdapter.setStereoPresentation(pending.stereo ?? null);
          if (pending.viewport !== undefined) rendererAdapter.resize(pending.viewport);
          if (pending.lake !== undefined) rendererAdapter.setLakeVisualCondition(pending.lake);
        }
        frameViewport = width > 0 && height > 0 ? Object.freeze({ x: width, y: height, pixelRatio }) : null;
        framePhase = "physics";
        try {
          const referenceSpace = renderer.xr.getReferenceSpace();
          if (renderer.xr.isPresenting) {
            const viewer = xrFrame !== undefined && xrFrame !== null && referenceSpace !== null ? xrFrame.getViewerPose(referenceSpace) : null;
            callback(timestamp, captureXrViewerFrame(viewer ?? null));
          } else if (stereoPresentation !== null) {
            geometryStereo.eyeSep = stereoPresentation.eyeSeparationMeters;
            updateFixedCameraProjection();
            callback(timestamp, captureConfiguredViewerFrame(currentExternalCameraPose() === null ? camera : fixedCamera, geometryStereo));
          } else {
            callback(timestamp, unavailableViewerFrame("not-stereo"));
          }
        } finally {
          framePhase = "idle";
          frameViewport = null;
        }
      });
    },
    beginViewFrame() {
      ensureActive(disposed);
      if (framePhase === "physics") framePhase = "view";
    },
    stopLoop() {
      if (disposed || !loopRunning) return;
      renderer.setAnimationLoop(null);
      loopRunning = false;
      renderedTrackingMount = null;
    },
    render(frame: BackendFrame) {
      ensureActive(disposed);
      if (!renderer.xr.isPresenting) {
        if (frameViewport !== null && (frame.viewport.x !== frameViewport.x || frame.viewport.y !== frameViewport.y || frame.viewport.pixelRatio !== frameViewport.pixelRatio)) stageViewInputs({ viewport: frame.viewport });
        resizeIfNeeded(frameViewport ?? frame.viewport);
      }
      const externalCameraPose = currentExternalCameraPose();
      const useExternalCamera = externalCameraPose !== null;
      const titlePresentationPose = flightPose === null && !renderer.xr.isPresenting ? titleCameraPose : IDENTITY_POSE;
      setPose(trackingOrigin, composePose(titlePresentationPose, currentPilotEyePose()));
      setPose(camera, frame.cameraPose);
      const externalPose = externalCameraPose ?? IDENTITY_POSE;
      setPose(externalCameraRig, externalPose);
      setPose(fixedCamera, renderer.xr.isPresenting ? IDENTITY_POSE : frame.cameraPose);
      updateFixedCameraProjection();
      setPose(aircraftRoot, flightPose === null ? IDENTITY_POSE : flightRelativePose(flightPose, IDENTITY_POSE));
      const worldFromTracking = useExternalCamera
        ? externalPose
        : flightPose === null
          ? titlePresentationPose
          : flightRelativePose(flightPose, currentTrackingMountPose());
      headHudSurface?.update(frame.headHud, worldFromTracking);
      airframe.setVisualState(
        flightPose?.airspeedMetersPerSecond ?? null,
        flightPose?.actuatorDeflectionRadians?.pitch ?? 0,
        flightPose?.actuatorDeflectionRadians?.yaw ?? 0
      );
      const simulationTimeSeconds = flightPose?.simulationTimeSeconds ?? 0;
      lakeUniforms.time.value = simulationTimeSeconds;
      // Flight and title scenes share the lake's 0 m datum; the shoreline
      // transition mesh rises from that plane into valid land elevation data.
      water.position.set(flightPose?.datumPositionNed.east ?? 0, 0, -(flightPose?.datumPositionNed.north ?? 0));
      farWater.position.set(flightPose?.datumPositionNed.east ?? 0, 0, -(flightPose?.datumPositionNed.north ?? 0));
      farWaterBackingCenter.set(farWater.position.x, farWater.position.z, 0, 0);
      // Terrain, shoreline, and islands are part of the shared world in every
      // scene; only the aircraft pose is absent on the title screen.
      venue.group.visible = true;
      // Keep the light's orthographic shadow volume around the moving airframe.
      sun.target.position.copy(aircraftRoot.position);
      sun.position.copy(aircraftRoot.position).add(sunOffset);
      if (useExternalCamera) {
        externalCameraRig.add(panelMesh);
        setPose(panelMesh, composePose(inversePose(externalPose), frame.panelPose));
      } else {
        scene.add(panelMesh);
        const titlePanelPose = frame.panel?.anchor === "menu" || frame.panel?.anchor === "head"
          ? composePose(titlePresentationPose, frame.panelPose)
          : frame.panelPose;
        setPose(panelMesh, flightPose === null ? titlePanelPose : flightRelativePose(flightPose, frame.panelPose));
      }
      const overlayPanel = frame.panel !== null && uiPanelComposition(frame.panel.anchor) === "overlay";
      if (panelMaterial.transparent !== overlayPanel) {
        panelMaterial.transparent = overlayPanel;
        panelMaterial.needsUpdate = true;
      }
      panelMaterial.depthTest = !overlayPanel;
      panelMaterial.depthWrite = !overlayPanel;
      panelMesh.renderOrder = overlayPanel ? PANEL_OVERLAY_RENDER_ORDER : 0;
      gazeCursorMaterial.depthTest = !overlayPanel;
      gazeCursor.renderOrder = overlayPanel ? PANEL_OVERLAY_RENDER_ORDER + 1 : 0;
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
      scene.updateMatrixWorld(true);
      const viewCamera = useExternalCamera ? fixedCamera : camera;
      const waterLevel = water.position.y;
      const eyePosition = viewCamera.getWorldPosition(reflectedPosition);
      if (eyePosition.y > waterLevel + 0.05) {
        // A real image of the airframe is rendered from the mirrored eye.
        // The water shader projects each displaced water vertex into this view.
        reflectionCamera.position.copy(eyePosition);
        reflectionCamera.position.y = 2 * waterLevel - eyePosition.y;
        viewCamera.getWorldDirection(reflectedForward);
        reflectedForward.y *= -1;
        reflectedUp.set(0, 1, 0).applyQuaternion(viewCamera.getWorldQuaternion(reflectionCamera.quaternion));
        reflectedUp.y *= -1;
        reflectionCamera.up.copy(reflectedUp);
        reflectedTarget.copy(reflectionCamera.position).add(reflectedForward);
        reflectionCamera.lookAt(reflectedTarget);
        reflectionCamera.projectionMatrix.copy(viewCamera.projectionMatrix);
        reflectionCamera.projectionMatrixInverse.copy(viewCamera.projectionMatrixInverse);
        reflectionCamera.updateMatrixWorld(true);
        reflectionMatrix.multiplyMatrices(reflectionCamera.projectionMatrix, reflectionCamera.matrixWorldInverse);
        reflectionTarget.setSize(Math.max(256, Math.min(1024, Math.ceil(width * pixelRatio / 2))),
          Math.max(128, Math.min(1024, Math.ceil(height * pixelRatio / 2))));
        const previousTarget = renderer.getRenderTarget();
        const previousBackground = scene.background;
        const previousAlpha = renderer.getClearAlpha();
        const previousXrEnabled = renderer.xr.enabled;
        renderer.getClearColor(savedClearColor);
        try {
          scene.background = null;
          renderer.xr.enabled = false;
          renderer.setRenderTarget(reflectionTarget);
          renderer.setClearColor(0x000000, 0);
          renderer.clear();
          renderer.render(scene, reflectionCamera);
          reflectionEnabled.value = 1;
        } finally {
          renderer.setRenderTarget(previousTarget);
          renderer.setClearColor(savedClearColor, previousAlpha);
          renderer.xr.enabled = previousXrEnabled;
          scene.background = previousBackground;
        }
      } else {
        reflectionEnabled.value = 0;
      }
      if (stereoPresentation !== null && !renderer.xr.isPresenting) {
        stereoEffect.render(scene, useExternalCamera ? fixedCamera : camera);
        renderer.setViewport(0, 0, width, height);
        renderer.setScissor(0, 0, width, height);
        renderer.setScissorTest(false);
      } else {
        renderer.render(scene, useExternalCamera ? fixedCamera : camera);
      }
      renderedTrackingMount = loopRunning && renderer.xr.isPresenting && frame.panelVisible ? currentTrackingMountPose() : null;
    },
    resize(viewport: ViewportSize) {
      ensureActive(disposed);
      if (renderer.xr.isPresenting) return;
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ viewport }); return; }
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
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ stereo: profile }); return; }
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
      pendingViewInputs = null;
      framePhase = "idle";
      frameViewport = null;
      renderedTrackingMount = null;
      panelTexture.dispose();
      headHudSurface?.dispose();
      gazeCursorGeometry.dispose();
      gazeCursorMaterial.dispose();
      panelGeometry.dispose();
      panelMaterial.dispose();
      waterGeometry.dispose();
      waterMaterial.dispose();
      farWaterGeometry.dispose();
      farWaterMaterial.dispose();
      reflectionTarget.dispose();
      skyTexture.dispose();
      lakeResources.near.texture.dispose();
      lakeResources.far.texture.dispose();
      airframe.dispose();
      venue.dispose();
      renderer.dispose();
      disposed = true;
    },
    setSelectRayHandler(handler) {
      selectRayHandler = handler;
    },
    setFlightPose(pose: FlightRenderPose | null) {
      if (framePhase === "view" || (framePhase === "idle" && pendingViewInputs !== null) || (framePhase === "physics" && (pose === null || flightPose === null))) {
        stageViewInputs({ flightPose: pose }); return;
      }
      flightPose = pose;
      if (pendingViewInputs !== null && "flightPose" in pendingViewInputs) stageViewInputs({ flightPose: pose });
    },
    setLakeVisualCondition(condition: LakeVisualCondition) {
      ensureActive(disposed);
      if (framePhase === "view" || pendingViewInputs !== null) { stageViewInputs({ lake: condition }); return; }
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
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ cameraMode: mode }); return; }
      flightCameraMode = mode;
    },
    setCinematicCameraView(view: CinematicCameraView | null) {
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ cameraView: view }); return; }
      fixedCameraView = view;
    },
    transformTrackingPose(pose: Pose): Pose {
      return flightPose === null ? pose : composePose(currentTrackingMountPose(), pose);
    }
  };

  const webxr: WebXrSessionPort = {
    transformTrackingPose(value: Pose): Pose {
      return composePose(currentTrackingMountPose(), value);
    },
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
    renderedTrackingMount = null;
  }

  function isAttaching(): boolean {
    return xrState.type === "attaching";
  }

  function updateFixedCameraProjection(): void {
    const fixedView = currentExternalCameraPose() !== null && flightCameraMode !== "chase" ? fixedCameraView : null;
    const fieldOfView = fixedView?.verticalFieldOfViewDegrees ?? 60;
    if (fixedCamera.fov === fieldOfView) return;
    fixedCamera.fov = fieldOfView;
    fixedCamera.updateProjectionMatrix();
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
    titleCameraPose = titleScreenCameraPoseForViewport(width, height);
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
uniform mat4 uReflectionMatrix;
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
varying vec4 vReflectionCoord;
#include <shadowmap_pars_vertex>
${lakeWavePacketShader}

void main() {
  // PlaneGeometry's XY winding points +Z; reversing its second axis while
  // mapping to world XZ keeps the resulting top face wound toward +Y.
  vec3 p = vec3(position.x, 0.0, -position.y);
  vec2 waveXZ = (modelMatrix * vec4(p, 1.0)).xz;
  vBaseXZ = waveXZ;
  vec2 detailDrift = uWindVelocity.xy * uTimeSeconds;
  vec4 nearDetail = texture2D(uDetailNear, (waveXZ - detailDrift * 0.42) / uDetailExtents.x);
  vec4 farDetail = texture2D(uDetailFar, (waveXZ - detailDrift * 0.89) / uDetailExtents.y);
  float nearHeight = nearDetail.a - 128.0 / 255.0;
  float farHeight = farDetail.a - 128.0 / 255.0;
  float nearVisibility = 1.0 - smoothstep(0.12, 0.32, aGridSpacing);
  float farVisibility = 1.0 - smoothstep(0.8, 2.8, aGridSpacing);
  p.y += (nearHeight * 0.6 * nearVisibility + farHeight * 0.55 * farVisibility) * uDetailScale;
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
  vec2 resolvedDetailSlope = (nearSlope * (0.6 * nearVisibility)
    + farSlope * (0.55 * farVisibility)) * uDetailScale;
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
  vReflectionCoord = uReflectionMatrix * worldPosition;
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    vDirectionalShadowCoord[0] = directionalShadowMatrix[0] *
      (worldPosition + vec4(vWorldNormal * directionalLightShadows[0].shadowNormalBias, 0.0));
  #endif
  vCrest = crest / max(slopeEnergy, 0.001);
  vCompression = clamp(1.0 - horizontalDeterminant, 0.0, 1.0);
  vGridSpacing = aGridSpacing;
  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
`;

const lakeFilteredWaterVertexShader = /* glsl */ `
uniform mat4 uReflectionMatrix;
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying float vCrest;
varying float vCompression;
varying float vGridSpacing;
varying vec2 vBaseXZ;
varying vec2 vRepresentedSlope;
varying vec4 vReflectionCoord;
#include <shadowmap_pars_vertex>
void main() {
  vec4 worldPosition = modelMatrix * vec4(position.x, 0.0, -position.y, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * vec3(0.0, 1.0, 0.0));
  vCrest = 0.0;
  vCompression = 0.0;
  vGridSpacing = aGridSpacing;
  vBaseXZ = worldPosition.xz;
  vRepresentedSlope = vec2(0.0);
  vReflectionCoord = uReflectionMatrix * worldPosition;
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    vDirectionalShadowCoord[0] = directionalShadowMatrix[0] *
      (worldPosition + vec4(vWorldNormal * directionalLightShadows[0].shadowNormalBias, 0.0));
  #endif
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
uniform vec3 uHazeColor;
uniform vec4 uHazeRange;
uniform vec4 uSunDirection;
uniform sampler2D uReflectionTexture;
uniform float uReflectionEnabled;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform sampler2D uSkyTexture;
uniform vec4 uDetailExtents;
uniform float uWaterBackingClip;
uniform vec4 uWaterBackingCenter;
uniform float uWaterBackingInnerExtent;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying float vCrest;
varying float vCompression;
varying float vGridSpacing;
varying vec2 vBaseXZ;
varying vec2 vRepresentedSlope;
varying vec4 vReflectionCoord;
#include <common>
#include <shadowmap_pars_fragment>
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
  const float fineAmplitude = 0.4;
  const float rippleAmplitude = 0.1;
  vec2 drift = uWindVelocity.xy * uTimeSeconds;
  vec4 nearSample = texture2D(uDetailNear, (worldXZ - drift * 0.42) / uDetailExtents.x);
  // These compact gravity-wave bands use phase-speed ratios proportional to
  // sqrt(wavelength): far/near/fine/ripple = 0.89/0.42/0.26/0.18.
  vec4 farSample = texture2D(uDetailFar, (worldXZ - drift * 0.89) / uDetailExtents.y);
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
  vec2 microSlope = nearSlope * 0.6 + farSlope * 0.55 + fineSlope + rippleSlope;
  // Blue stores slope squared. Mipmaps preserve the variance of unresolved
  // wavelets even after their mean slope approaches zero.
  float nearVariance = max(nearSample.b / 2.5 - dot(nearSlope, nearSlope), 0.0);
  float farVariance = max(farSample.b / 2.5 - dot(farSlope, farSlope), 0.0);
  float fineVariance = max(fineSample.b / 2.5 - dot(fineRawSlope, fineRawSlope), 0.0);
  float rippleVariance = max(rippleSample.b / 2.5 - dot(rippleRawSlope, rippleRawSlope), 0.0);
  float slopeMoment = dot(microSlope, microSlope)
    + 0.6 * 0.6 * nearVariance + 0.55 * 0.55 * farVariance
    + 2.6 * 2.6 * fineAmplitude * fineAmplitude * fineVariance
    + 5.2 * 5.2 * rippleAmplitude * rippleAmplitude * rippleVariance;
  float strength = smoothstep(0.05, 0.6, uWindSpeed);
  float scaledStrength = strength * uDetailScale;
  return vec3(microSlope * scaledStrength, slopeMoment * scaledStrength * scaledStrength);
}

vec3 sampleLakeColor(vec2 worldXZ, vec2 waveSlope, vec3 microDetail, float sunVisibility) {
  vec3 geometricNormal = normalize(vWorldNormal) * (gl_FrontFacing ? 1.0 : -1.0);
  float patches = lakeNoise(worldXZ * 0.17 + vec2(2.3, -7.1));
  vec2 microSlope = microDetail.xy;
  float unresolvedVariance = max(microDetail.z - dot(microSlope, microSlope), 0.0);
  // Subtract the exact vertex slope already represented by the geometric
  // normal; vertex LOD0 and filtered fragment samples need not be identical.
  vec2 geometricSlope = -geometricNormal.xz / max(geometricNormal.y, 0.01);
  vec2 totalSlope = geometricSlope + waveSlope + microSlope - vRepresentedSlope;
  vec3 normal = normalize(vec3(-totalSlope.x, 1.0, -totalSlope.y));
  vec3 viewDirection = normalize(cameraPosition - vec3(worldXZ.x, vWorldPosition.y, worldXZ.y));
  float ndv = max(dot(geometricNormal, viewDirection), 0.0);
  float geometricFresnel = 0.02 + 0.98 * pow(1.0 - ndv, 5.0);
  float facetNdotV = dot(normal, viewDirection);
  float facetFresnel = 0.02 + 0.98 * pow(1.0 - max(facetNdotV, 0.0), 5.0);
  float rayVisibility = smoothstep(0.0, 0.18, facetNdotV);
  float fresnelVisibility = rayVisibility * smoothstep(0.03, 0.22, ndv);
  vec3 reflectedDirection = normalize(mix(reflect(-viewDirection, geometricNormal),
    reflect(-viewDirection, normal), rayVisibility));
  vec3 reflection = lakeSkyRadiance(reflectedDirection);
  float fresnel = mix(geometricFresnel, facetFresnel, 0.4 * fresnelVisibility);
  float sunAlignment = max(dot(reflectedDirection, normalize(uSunDirection.xyz)), 0.0);
  float roughness = clamp(0.23 + uWindSpeed * 0.018 + sqrt(unresolvedVariance) * 0.6, 0.23, 0.62);
  float glitterLobe = pow(sunAlignment, mix(220.0, 45.0, roughness));
  float glitterNoise = 0.35 + 0.65 * clamp(sqrt(microDetail.z) * 3.0, 0.0, 1.0);
  float glitter = glitterLobe * glitterNoise * smoothstep(0.35, 2.0, uWindSpeed) * sunVisibility;
  float diffuse = 0.58 + 0.42 * sunVisibility * max(dot(normal, normalize(uSunDirection.xyz)), 0.0);
  float sharpCrest = smoothstep(0.42, 0.76, vCompression);
  float crestLight = smoothstep(0.05, 1.05, vCrest) * sharpCrest * mix(0.35, 1.0, patches);
  vec3 base = mix(uWaterDark, uWaterMid, diffuse * 0.72);
  base = mix(base, uWaterLight, crestLight * 0.16 + patches * 0.07);
  base *= mix(0.82, 1.0, sunVisibility);
  // Existing wave groups may remain steep after the local wind weakens.
  float whitecap = smoothstep(0.68, 0.84, vCompression) * smoothstep(0.7, 0.98, vCrest) * 0.12;
  // Dark water absorption must not also darken the reflected sky.
  base *= vec3(0.29, 0.34, 0.37);
  // Art-directed reflection strength for the dark reference water. This is
  // not a rough-surface BRDF and leaves a visible sky/water boundary.
  vec3 color = mix(base, reflection, fresnel * 0.45);
  if (uReflectionEnabled > 0.5 && vReflectionCoord.w > 0.0) {
    vec2 reflectedUv = vReflectionCoord.xy / vReflectionCoord.w * 0.5 + 0.5;
    // Small surface-slope offsets break up the image along moving wave facets.
    reflectedUv += totalSlope * 0.006;
    if (all(greaterThanEqual(reflectedUv, vec2(0.0))) &&
        all(lessThanEqual(reflectedUv, vec2(1.0)))) {
      vec4 aircraftReflection = texture2D(uReflectionTexture, reflectedUv);
      color = mix(color, aircraftReflection.rgb,
        aircraftReflection.a * clamp(0.16 + fresnel * 0.7, 0.0, 0.72));
    }
  }
  float depthTint = clamp(uVisualWaveHeight / 0.35, 0.0, 1.0);
  color = mix(color, color * vec3(0.92, 0.97, 1.04), depthTint * 0.12);
  color += vec3(1.0, 0.78, 0.52) * glitter * 0.15;
  color = mix(color, vec3(0.38, 0.43, 0.43), whitecap * 0.28);
  // Use the scene's same fog curve and sky-colored airlight so distant water
  // fades into the far shore without a separate dark horizon band.
  float distanceToEye = length(cameraPosition - vWorldPosition);
  float atmosphericFade = smoothstep(uHazeRange.x, uHazeRange.y, distanceToEye);
  color = mix(color, uHazeColor, atmosphericFade);
  return color;
}

void main() {
  // The 160 km calm backing supplies only the area beyond the 6 km animated
  // plane. Clipping its center prevents coplanar overlap from flattening wave
  // troughs or creating a visible square transition.
  if (uWaterBackingClip > 0.5 &&
      all(lessThan(abs(vBaseXZ - uWaterBackingCenter.xy), vec2(uWaterBackingInnerExtent)))) discard;
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
  float sunVisibility = 1.0;
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    sunVisibility = getShadow(directionalShadowMap[0], directionalLightShadows[0].shadowMapSize,
      directionalLightShadows[0].shadowIntensity, directionalLightShadows[0].shadowBias,
      directionalLightShadows[0].shadowRadius, vDirectionalShadowCoord[0]);
  #endif
  vec3 color = sampleLakeColor(vWorldPosition.xz, waveSlope, microDetail, sunVisibility);
  gl_FragColor = vec4(color, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;
