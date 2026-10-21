import {
  AmbientLight,
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
  Texture,
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
import type { BackendFrame, FlightCameraMode, FlightRenderPose, LakeWaterQualityCleanupResult, RendererAdapter, SelectRay, StereoPresentationProfile, ViewportSize } from "../../contracts/runtime.js";
import type { CinematicCameraView } from "../../contracts/camera.js";
import { uiPanelComposition } from "../../contracts/ui.js";
import { IDENTITY_POSE, multiplyQuaternion, pose, quaternion, rotateVec3, vec3 } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "../../../presentation/webxr-contracts.js";
import { selectRayFromXrEvent } from "./xr-select-ray.js";
import { captureConfiguredViewerFrame, captureXrViewerFrame } from "./viewer-frame.js";
import { createHeadHudSurface } from "./head-hud-surface.js";
import { unavailableViewerFrame } from "../../contracts/viewer-frame.js";
import { flightRelativePose, pilotEyePoseRender, poseFrdToRender, registeredPilotEyePoint,
  chaseCameraPoseFrd, XrCameraOrigin, projectXrPanelCameraPoses, projectXrReferenceFrame } from "../../camera/xr-spatial.js";
import type { XrCameraRig } from "../../../generated/xr-camera-rig.js";
import { DEFAULT_LAKE_VISUAL_CONDITION, lakeWaterQualityProfile } from "../../contracts/lake-water.js";
import type { LakeVisualCondition, LakeWaterQuality } from "../../contracts/lake-water.js";
import { createLakeSkyCondition, lakeSkySunDirectionNed, sameLakeSkyRenderCondition } from "../../contracts/lake-sky.js";
import type { LakeSkyCondition } from "../../contracts/lake-sky.js";
import { createProductionLakeDetailLayer } from "./lake-detail-texture.js";
import type { LakeDetailLayer } from "./lake-detail-texture.js";
import { createLakeSkyTexture, lakeSkyHaze } from "./lake-sky-texture.js";
import { lake_scene_ambient_color_srgb, lake_scene_ambient_irradiance,
  lake_scene_sunlight_color_srgb, lake_scene_sunlight_irradiance } from "../../../../pkg/birdman_game_wasm.js";
import { createLakeChunks, disposeLakeChunks, lakeWaterDisplacementBounds, updateLakeChunkBounds } from "./lake-water-geometry.js";
import type { LakeWaterDisplacement } from "./lake-water-geometry.js";
import { createLakeWaveProjection, LAKE_WAVE_PACKET_GLSL, LAKE_WAVE_PROJECTION_GLSL } from "./lake-wave-projection.js";
import { createBirdmanAirframe, NO_AIRFRAME_CONTROLS } from "./birdman-airframe.js";
import { createLakeVenue } from "./lake-venue-mesh.js";
import { LAUNCH_PLATFORM } from "../../contracts/launch-venue.js";
import type { ScreenRenderAutoBinding } from "../../contracts/screen-render-auto.js";
import type { ScreenRenderAutoSnapshot } from "../../../generated/screen-render-auto.js";
import { ScreenAutoRenderer } from "./screen-auto-renderer.js";
import { applyScreenRenderResources } from "./screen-render-resources.js";

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
  prepare(): Promise<void>;
}

// Place Takeshima just outside the centered title panel at every viewport
// aspect ratio. Its bearing is computed from the OSM island outline.
const TITLE_SCREEN_ISLAND_BEARING_RADIANS = 272.16851686166876 * Math.PI / 180;
const TITLE_SCREEN_PANEL_MAX_WIDTH_PIXELS = 42 * 16;
const TITLE_SCREEN_PANEL_ROOT_PADDING_PIXELS = 32;
const TITLE_SCREEN_PANEL_ISLAND_GAP_PIXELS = 24;
const TITLE_SCREEN_PITCH_HALF_RADIANS = 3 * Math.PI / 180;
const PANEL_OVERLAY_RENDER_ORDER = 2000;

export function titleScreenCameraPoseForViewport(width: number, height: number, verticalFieldOfViewDegrees: number): Pose {
  return titleCameraPoseForViewport(width, height, -TITLE_SCREEN_PITCH_HALF_RADIANS, verticalFieldOfViewDegrees);
}

function titleCameraPoseForViewport(width: number, height: number, pitchHalfRadians: number, verticalFieldOfViewDegrees: number): Pose {
  if (!Number.isFinite(width) || width <= 0 || !Number.isFinite(height) || height <= 0) {
    throw new RangeError("Viewport dimensions must be positive");
  }
  if (!Number.isFinite(verticalFieldOfViewDegrees) || verticalFieldOfViewDegrees <= 0 || verticalFieldOfViewDegrees >= 180) {
    throw new RangeError("Screen vertical field of view must be finite and between zero and 180 degrees");
  }
  const aspectRatio = width / height;
  const horizontalHalfFov = Math.atan(Math.tan(verticalFieldOfViewDegrees * Math.PI / 360) * aspectRatio);
  const panelWidth = Math.min(TITLE_SCREEN_PANEL_MAX_WIDTH_PIXELS, width - TITLE_SCREEN_PANEL_ROOT_PADDING_PIXELS);
  const panelHalfWidthNdc = panelWidth / width;
  const targetIslandX = -Math.min(0.98, panelHalfWidthNdc + TITLE_SCREEN_PANEL_ISLAND_GAP_PIXELS / width);
  const islandRelativeBearing = Math.atan(targetIslandX * Math.tan(horizontalHalfFov));
  const yaw = islandRelativeBearing - TITLE_SCREEN_ISLAND_BEARING_RADIANS;
  return pose(
    vec3(0, 12, 0),
    multiplyQuaternion(
      quaternion(Math.cos(yaw / 2), 0, Math.sin(yaw / 2), 0),
      quaternion(Math.cos(pitchHalfRadians), Math.sin(pitchHalfRadians), 0, 0)
    )
  );
}
interface LakeWaveResources {
  readonly displacement: LakeWaterDisplacement;
  readonly waveKAmplitude: readonly Vector4[];
  readonly waveOmegaPhase: readonly Vector4[];
  readonly waveCount: number;
  readonly visualWaveHeight: number;
  readonly choppiness: number;
  readonly spacingGradientMagnitude: number;
}

interface LakeVisualResources extends LakeWaveResources {
  readonly windSpeed: number;
  readonly near: LakeDetailLayer;
  readonly far: LakeDetailLayer;
}

function createLakeWaveResources(condition: LakeVisualCondition, quality: LakeWaterQuality): LakeWaveResources {
  const projection = createLakeWaveProjection(condition, quality);
  return {
    displacement: lakeWaterDisplacementBounds(projection),
    waveKAmplitude: projection.waveKAmplitude.map((wave) => new Vector4(...wave)),
    waveOmegaPhase: projection.waveOmegaPhase.map((wave) => new Vector4(...wave)),
    waveCount: projection.waveCount, visualWaveHeight: projection.visualWaveHeight,
    choppiness: projection.choppiness, spacingGradientMagnitude: projection.spacing.gradientMagnitude
  };
}

function createLakeVisualResources(condition: LakeVisualCondition, quality: LakeWaterQuality): LakeVisualResources {
  const windSpeed = Math.hypot(condition.windNorthMetersPerSecond, condition.windEastMetersPerSecond);
  if (!Number.isFinite(windSpeed) || windSpeed < 0.05 ||
      !Number.isFinite(condition.detailAmplitudeScale) || condition.detailAmplitudeScale <= 0 || condition.detailAmplitudeScale > 3 ||
      !Number.isSafeInteger(condition.patternSeed) || condition.patternSeed < 0) {
    throw new RangeError("Invalid render-only lake visual condition");
  }
  const waves = createLakeWaveResources(condition, quality);
  const near = createProductionLakeDetailLayer(condition, false);
  try {
    const far = createProductionLakeDetailLayer(condition, true);
    return { ...waves, windSpeed, near, far };
  } catch (error) {
    near.texture.dispose();
    throw error;
  }
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
  screenVerticalFieldOfViewDegrees: number,
  lakeQuality: LakeWaterQuality = "high",
  headHudCanvas?: HTMLCanvasElement,
  screenAutoBinding?: ScreenRenderAutoBinding
): ThreeRendererBundle {
  let titleCameraPose = titleScreenCameraPoseForViewport(1, 1, screenVerticalFieldOfViewDegrees);
  let titlePhoneCameraPose = titleCameraPoseForViewport(1, 1, 0, screenVerticalFieldOfViewDegrees);
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
  renderer.shadowMap.enabled = true;
  const stereoEffect = new StereoEffect(renderer);
  const geometryStereo = new StereoCamera();
  geometryStereo.aspect = 0.5;
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setClearColor(0x9fb0ad, 1);

  const scene = new Scene();
  let activeSkyCondition: LakeSkyCondition | null = null;
  let skyTexture = createLakeSkyTexture(activeSkyCondition);
  scene.background = skyTexture;
  const lakeFog = new Fog(...lakeSkyHaze(undefined));
  scene.fog = lakeFog;
  const ambient = new AmbientLight(lake_scene_ambient_color_srgb(), lake_scene_ambient_irradiance());
  ambient.layers.enable(1);
  scene.add(ambient);
  const sun = new DirectionalLight(lake_scene_sunlight_color_srgb(), 0);
  const sunDistanceMeters = Math.hypot(20, 35, 18);
  sun.position.set(0, sunDistanceMeters, 0);
  sun.layers.enable(1);
  sun.castShadow = false;
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
  const sunOffset = new Vector3(0, sunDistanceMeters, 0);

  let activeLakeQuality = lakeQuality;
  const waterQuality = lakeWaterQualityProfile(activeLakeQuality);
  let activeLakeCondition = DEFAULT_LAKE_VISUAL_CONDITION;
  let lakeResources = createLakeVisualResources(activeLakeCondition, lakeQuality);
  let waterGeometry = createLakeChunks(waterQuality.meshSegments, lakeResources.displacement);
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
      uChoppiness: { value: lakeResources.choppiness },
      uGridSpacingGradient: { value: lakeResources.spacingGradientMagnitude },
      uVisualWaveHeight: { value: lakeResources.visualWaveHeight },
      uWaterDark: { value: new Color(0x172831) },
      uWaterMid: { value: new Color(0x293b43) },
      uWaterLight: { value: new Color(0x50636a) },
      uHazeColor: { value: lakeFog.color },
      uHazeRange: { value: new Vector4(lakeFog.near, lakeFog.far, 0, 0) },
      uSunDirection: { value: new Vector4(0, 1, 0, 0) },
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
    choppiness: lakeUniform(waterMaterial, "uChoppiness", lakeResources.choppiness),
    spacingGradient: lakeUniform(waterMaterial, "uGridSpacingGradient", lakeResources.spacingGradientMagnitude),
    visualWaveHeight: lakeUniform(waterMaterial, "uVisualWaveHeight", lakeResources.visualWaveHeight),
    skyTexture: lakeUniform(waterMaterial, "uSkyTexture", skyTexture),
    sunDirection: lakeUniform(waterMaterial, "uSunDirection", new Vector4()),
    hazeRange: lakeUniform(waterMaterial, "uHazeRange", new Vector4())
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
    uChoppiness: lakeUniform(waterMaterial, "uChoppiness", lakeResources.choppiness),
    uGridSpacingGradient: lakeUniform(waterMaterial, "uGridSpacingGradient", lakeResources.spacingGradientMagnitude),
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
  const water = new Group();
  water.name = "lake-biwa-animated-water";
  const waterMeshes = waterGeometry.map((geometry, index) => {
    const mesh = new Mesh(geometry, waterMaterial);
    mesh.name = `lake-biwa-water-chunk-${String(index)}`;
    mesh.receiveShadow = true;
    water.add(mesh);
    return mesh;
  });
  scene.add(water);
  const venue = createLakeVenue();
  venue.group.visible = false;
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
  const pointerCursors = [gazeCursor];

  const aircraftRoot = new Group();
  aircraftRoot.name = "aircraft-presentation-root";
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
  const camera = new PerspectiveCamera(screenVerticalFieldOfViewDegrees, 1, 0.05, 100_000);
  trackingOrigin.add(camera);
  const externalCameraRig = new Group();
  scene.add(externalCameraRig);
  const fixedCamera = new PerspectiveCamera(screenVerticalFieldOfViewDegrees, 1, 0.05, 100_000);
  externalCameraRig.add(fixedCamera);
  let disposed = false;
  let loopRunning = false;
  let width = 0;
  let height = 0;
  let pixelRatio = 0;
  let screenRenderScale = 1;
  let currentPanel: Extract<BackendFrame["panel"], { readonly kind: "visible" }>["panel"] | null = null;
  let stereoPresentation: StereoPresentationProfile | null = null;
  let xrState: ThreeWebXrState = { type: "idle" };
  let requestGeneration = 0;
  let sessionEndHandler: (() => void) | null = null;
  let referenceSpaceResetHandler: ((previousReferenceFromNew: Pose | null) => void) | null = null;
  let activeReferenceSpace: XRReferenceSpace | null = null;
  let selectRayHandler: ((ray: SelectRay) => void) | null = null;
  let pointerSources = new WeakMap<XRInputSource, number>();
  let nextPointerSource = 1;
  let framePointerRays: readonly SelectRay[] = [];
  let inputFocusHandler: ((focused: boolean) => void) | null = null;
  let flightPose: FlightRenderPose | null = null;
  let preparedFlightPose: FlightRenderPose | null = null;
  let flightCameraMode: FlightCameraMode = "pilot";
  let fixedCameraView: CinematicCameraView | null = null;
  let framePhase: "idle" | "physics" | "view" = "idle";
  let frameViewport: ViewportSize | null = null;
  let renderedTrackingMount: Pose | null = null;
  type LakeQualityRequest = Readonly<{
    quality: LakeWaterQuality;
    resolve: (cleanup: LakeWaterQualityCleanupResult) => void;
    reject: (reason: unknown) => void;
  }>;
  type PreparedLakeQuality = Readonly<{ quality: LakeWaterQuality; waves: LakeWaveResources; geometry: ReturnType<typeof createLakeChunks> }>;
  let manualLakeQualityRequest: LakeQualityRequest | null = null;
  type PendingViewInputs = Readonly<{
    flightPose?: FlightRenderPose | null;
    preparedFlightPose?: FlightRenderPose | null;
    cameraMode?: FlightCameraMode;
    cameraView?: CinematicCameraView | null;
    stereo?: StereoPresentationProfile | null;
    screenRenderScale?: number;
    viewport?: ViewportSize;
    lake?: LakeVisualCondition;
    sky?: LakeSkyCondition | null;
    venueVisible?: boolean;
    lakeQuality?: LakeQualityRequest;
  }>;
  let pendingViewInputs: PendingViewInputs | null = null;
  const settleManualLakeQuality = (snapshot: ScreenRenderAutoSnapshot): void => {
    const request = manualLakeQualityRequest;
    if (request === null || screenAutoBinding === undefined || snapshot.status.kind === "applying"
        || screenAutoBinding.policy.requested_scale_percent() !== 0) return;
    manualLakeQualityRequest = null;
    if (snapshot.status.kind === "failed" || snapshot.status.kind === "unsupported") request.reject(new Error(snapshot.status.cause));
    else request.resolve({ kind: "complete" });
  };
  const screenAuto = screenAutoBinding === undefined ? null
    : new ScreenAutoRenderer(renderer.getContext(), {
      policy: screenAutoBinding.policy,
      snapshot: screenAutoBinding.snapshot,
      scene: screenAutoBinding.scene,
      onSnapshot(snapshot) {
        screenAutoBinding.onSnapshot(snapshot);
        settleManualLakeQuality(snapshot);
      }
    }, (factor, shadowMapSize, quality, screen) => {
      if (screen ? !hasScreenRenderArea(factor) : factor !== screenRenderScale || shadowMapSize !== sun.shadow.mapSize.x) return false;
      let next: PreparedLakeQuality | null = null;
      try {
        next = prepareLakeQuality(quality);
        const commitWater = (): string | undefined => {
          const cleanup = commitLakeQuality(next);
          next = null;
          return cleanup.kind === "failed" ? cleanup.message : undefined;
        };
        const result = applyScreenRenderResources(sun, renderer, shadowMapSize,
          () => screen ? applyScreenRenderScale(factor) : true, commitWater);
        if (result === false) {
          const unused = next;
          next = null;
          if (unused !== null) disposeLakeChunks(unused.geometry);
          return false;
        }
        const request = manualLakeQualityRequest;
        manualLakeQualityRequest = null;
        request?.resolve(result.cleanupFailure === undefined ? { kind: "complete" } : { kind: "failed", message: result.cleanupFailure });
        return result;
      } catch (cause: unknown) {
        let failure = cause;
        try { if (next !== null) disposeLakeChunks(next.geometry); }
        catch (cleanup: unknown) { failure = new AggregateError([cause, cleanup], "Screen water preparation cleanup failed", { cause }); }
        const request = manualLakeQualityRequest;
        manualLakeQualityRequest = null;
        request?.reject(failure);
        throw failure;
      }
    });
  const onContextLost = (): void => { screenAuto?.invalidate("interrupted"); };
  const onContextRestored = (): void => { screenAuto?.restored(); };
  if (screenAuto !== null) {
    canvas.addEventListener("webglcontextlost", onContextLost);
    canvas.addEventListener("webglcontextrestored", onContextRestored);
  }
  const stageViewInputs = (inputs: PendingViewInputs): void => {
    pendingViewInputs = Object.freeze({ ...pendingViewInputs, ...inputs });
  };
  const prepareLakeQuality = (quality: LakeWaterQuality): PreparedLakeQuality | null => {
    if (quality === activeLakeQuality) return null;
    const waves = createLakeWaveResources(activeLakeCondition, quality);
    return { quality, waves, geometry: createLakeChunks(lakeWaterQualityProfile(quality).meshSegments, waves.displacement) };
  };
  const commitLakeQuality = (next: PreparedLakeQuality | null): LakeWaterQualityCleanupResult => {
    if (next === null) return { kind: "complete" };
    if (next.geometry.length !== waterMeshes.length) throw new Error("Water chunk count changed during quality preparation");
    const previousGeometry = waterGeometry;
    lakeUniforms.waveKAmplitude.value = next.waves.waveKAmplitude;
    lakeUniforms.waveOmegaPhase.value = next.waves.waveOmegaPhase;
    lakeUniforms.waveCount.value = next.waves.waveCount;
    lakeUniforms.choppiness.value = next.waves.choppiness;
    lakeUniforms.spacingGradient.value = next.waves.spacingGradientMagnitude;
    lakeUniforms.visualWaveHeight.value = next.waves.visualWaveHeight;
    lakeResources = { ...lakeResources, ...next.waves };
    for (const [index, geometry] of next.geometry.entries()) {
      const mesh = waterMeshes[index];
      if (mesh === undefined) throw new Error("Missing retained water chunk");
      mesh.geometry = geometry;
    }
    waterGeometry = next.geometry;
    activeLakeQuality = next.quality;
    try {
      disposeLakeChunks(previousGeometry);
      return { kind: "complete" };
    } catch (cause: unknown) {
      return { kind: "failed", message: errorMessage(cause) };
    }
  };
  const applyLakeQuality = (request: LakeQualityRequest): void => {
    if (screenAuto !== null) {
      manualLakeQualityRequest = request;
      try {
        screenAuto.selectWaterQuality(request.quality);
        if (screenAutoBinding !== undefined) settleManualLakeQuality(screenAutoBinding.snapshot());
      }
      catch (cause: unknown) { manualLakeQualityRequest = null; request.reject(cause); }
      return;
    }
    try {
      request.resolve(commitLakeQuality(prepareLakeQuality(request.quality)));
    } catch (cause: unknown) { request.reject(cause); }
  };
  const currentExternalCameraPose = (): Pose | null => {
    if (flightPose === null || flightCameraMode === "pilot") return null;
    if (flightCameraMode !== "chase" && fixedCameraView !== null) return fixedCameraView.pose;
    return flightRelativePose(flightPose, poseFrdToRender(chaseCameraPoseFrd("chase")));
  };
  const pilotEyePoint = registeredPilotEyePoint();
  const currentPilotEyePose = (): Pose => flightPose === null
    ? IDENTITY_POSE
    : pilotEyePoseRender(pilotEyePoint, flightPose.pilotPositionMeters, flightPose.initialPilotPositionMeters);
  const usesExternalCamera = (): boolean => flightPose !== null && flightCameraMode !== "pilot";
  const cameraOrigin = new XrCameraOrigin();
  let cameraPlacement: { flight: FlightRenderPose | null; prepared: FlightRenderPose | null; mode: FlightCameraMode;
    cinematic: CinematicCameraView | null; title: Pose; value: { aircraft: Pose; external: Pose | null; rig: XrCameraRig } } | null = null;
  const currentCameraPlacement = () => {
    const title = flightPose === null && !renderer.xr.isPresenting
      ? stereoPresentation === null ? titleCameraPose : titlePhoneCameraPose : IDENTITY_POSE;
    if (cameraPlacement !== null && cameraPlacement.flight === flightPose && cameraPlacement.prepared === preparedFlightPose &&
        cameraPlacement.mode === flightCameraMode && cameraPlacement.cinematic === fixedCameraView && cameraPlacement.title === title) return cameraPlacement.value;
    const display = flightPose ?? preparedFlightPose;
    const heading = LAUNCH_PLATFORM.launchBearingDegrees * Math.PI / 180;
    const aircraft = display === null
      ? pose(vec3(0, LAUNCH_PLATFORM.frontLipAboveWaterMeters, 0), quaternion(Math.cos(heading / 2), 0, -Math.sin(heading / 2), 0))
      : flightRelativePose(display, IDENTITY_POSE);
    const external = currentExternalCameraPose();
    const mount = external !== null ? { kind: "external" as const, worldFromTracking: external }
      : flightPose !== null ? { kind: "aircraft" as const, aircraftFromTracking: currentPilotEyePose() }
        : { kind: "stationary" as const, worldFromTracking: title };
    const value = { aircraft, external, rig: cameraOrigin.rig(aircraft, mount) };
    cameraPlacement = { flight: flightPose, prepared: preparedFlightPose, mode: flightCameraMode, cinematic: fixedCameraView, title, value };
    return value;
  };
  const currentTrackingMountPose = (): Pose => currentCameraPlacement().rig.referenceFromTracking;

  const onSelect = (event: XRInputSourceEvent): void => {
    if (xrState.type !== "active" && xrState.type !== "attaching") return;
    if (!hasXrInputFocus()) return;
    const session = sessionFromState(xrState);
    if (event.inputSource.targetRayMode === "tracked-pointer" && (session === null || !Array.from(session.inputSources).includes(event.inputSource))) return;
    const referenceSpace = renderer.xr.getReferenceSpace();
    if (referenceSpace === null || selectRayHandler === null || renderedTrackingMount === null) return;
    const ray = selectRayFromXrEvent(event.frame, event.inputSource, referenceSpace);
    if (ray !== null) selectRayHandler(transformRay({ ...ray, timestampMs: performance.now(), source: pointerSource(event.inputSource) }, renderedTrackingMount));
  };

  const onInputFocusChange = (): void => {
    const focused = hasXrInputFocus();
    if (!focused) { framePointerRays = []; renderedTrackingMount = null; }
    inputFocusHandler?.(focused);
  };

  const onSessionEnd = (): void => {
    const session = sessionFromState(xrState);
    if (session === null) return;
    session.removeEventListener("select", onSelect);
    session.removeEventListener("visibilitychange", onInputFocusChange);
    const expected = xrState.type === "stopping";
    xrState = { type: "idle" };
    renderedTrackingMount = null;
    framePointerRays = [];
    if (expected) return;
    queueMicrotask(() => {
      clearSession(session);
      sessionEndHandler?.();
    });
  };

  const onReferenceSpaceReset = (event: XRReferenceSpaceEvent): void => {
    renderedTrackingMount = null;
    framePointerRays = [];
    let previousReferenceFromNew: Pose | null = null;
    try {
      const transform = event.transform as XRRigidTransform | null | undefined;
      if (transform !== null && transform !== undefined) {
        previousReferenceFromNew = {
          position: { x: transform.position.x, y: transform.position.y, z: transform.position.z },
          orientation: { w: transform.orientation.w, x: transform.orientation.x, y: transform.orientation.y, z: transform.orientation.z }
        };
        cameraOrigin.resetReference(previousReferenceFromNew);
        cameraPlacement = null;
      }
    } catch { referenceSpaceResetHandler?.(null); return; }
    referenceSpaceResetHandler?.(previousReferenceFromNew);
  };

  const rendererAdapter: RendererAdapter = {
    startLoop(callback) {
      ensureActive(disposed);
      if (loopRunning) throw new Error("Three.js frame loop is already active");
      loopRunning = true;
      renderer.setAnimationLoop((timestamp: number, xrFrame: XRFrame | null | undefined) => {
        screenAuto?.boundary(!renderer.xr.isPresenting && stereoPresentation === null && width > 0 && height > 0);
        const pending = pendingViewInputs;
        pendingViewInputs = null;
        if (pending !== null) {
          try {
            if ("flightPose" in pending) rendererAdapter.setFlightPose(pending.flightPose ?? null);
            if ("preparedFlightPose" in pending) rendererAdapter.setPreparedFlightPose(pending.preparedFlightPose ?? null);
            if (pending.cameraMode !== undefined) rendererAdapter.setFlightCameraMode(pending.cameraMode);
            if ("cameraView" in pending) rendererAdapter.setCinematicCameraView(pending.cameraView ?? null);
            if ("stereo" in pending) rendererAdapter.setStereoPresentation(pending.stereo ?? null);
            if (pending.screenRenderScale !== undefined) rendererAdapter.setScreenRenderScale(pending.screenRenderScale);
            if (pending.viewport !== undefined) rendererAdapter.resize(pending.viewport);
            if (pending.lake !== undefined) rendererAdapter.setLakeVisualCondition(pending.lake);
            if ("sky" in pending) rendererAdapter.setLakeSkyCondition(pending.sky ?? null);
            if (pending.venueVisible !== undefined) rendererAdapter.setLakeVenueVisible(pending.venueVisible);
          } catch (error: unknown) {
            pending.lakeQuality?.reject(error);
            throw error;
          }
          if (pending.lakeQuality !== undefined) applyLakeQuality(pending.lakeQuality);
        }
        frameViewport = width > 0 && height > 0 ? Object.freeze({ x: width, y: height, pixelRatio }) : null;
        framePhase = "physics";
        try {
          const referenceSpace = renderer.xr.getReferenceSpace();
          if (renderer.xr.isPresenting) {
            framePointerRays = [];
            const session = sessionFromState(xrState);
            if (xrFrame != null && referenceSpace !== null && session !== null && hasXrInputFocus()) {
              const rays: SelectRay[] = [];
              for (const inputSource of session.inputSources) {
                if (inputSource.targetRayMode !== "tracked-pointer") continue;
                try {
                  const ray = selectRayFromXrEvent(xrFrame, inputSource, referenceSpace);
                  if (ray !== null) rays.push({ ...ray, source: pointerSource(inputSource) });
                } catch { continue; }
              }
              framePointerRays = rays;
            }
            const viewer = xrFrame !== undefined && xrFrame !== null && referenceSpace !== null ? xrFrame.getViewerPose(referenceSpace) : null;
            callback(timestamp, captureXrViewerFrame(viewer ?? null));
          } else if (stereoPresentation !== null) {
            geometryStereo.eyeSep = stereoPresentation.eyeSeparationMeters;
            updateFixedCameraProjection();
            callback(timestamp, captureConfiguredViewerFrame(usesExternalCamera() ? fixedCamera : camera, geometryStereo));
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
      screenAuto?.invalidate("interrupted");
    },
    render(frame: BackendFrame) {
      ensureActive(disposed);
      if (!renderer.xr.isPresenting) {
        if (frameViewport !== null && (frame.viewport.x !== frameViewport.x || frame.viewport.y !== frameViewport.y || frame.viewport.pixelRatio !== frameViewport.pixelRatio)) stageViewInputs({ viewport: frame.viewport });
        resizeIfNeeded(frameViewport ?? frame.viewport);
      }
      const placement = currentCameraPlacement();
      const externalCameraPose = placement.external;
      const useExternalCamera = externalCameraPose !== null;
      const displayFlightPose = flightPose ?? preparedFlightPose;
      const aircraftPose = placement.aircraft;
      setPose(trackingOrigin, placement.rig.aircraftFromTracking);
      setPose(camera, frame.cameraPose);
      const externalPose = useExternalCamera ? placement.rig.worldFromTracking : IDENTITY_POSE;
      setPose(externalCameraRig, externalPose);
      setPose(fixedCamera, renderer.xr.isPresenting ? IDENTITY_POSE : frame.cameraPose);
      updateFixedCameraProjection();
      setPose(aircraftRoot, aircraftPose);
      headHudSurface?.update(frame.headHud, placement.rig.worldFromTracking);
      const controls = displayFlightPose?.controls;
      airframe.setVisualState(
        displayFlightPose?.wingDeformation ?? null,
        controls ?? NO_AIRFRAME_CONTROLS,
        displayFlightPose?.tailGeometry
      );
      const simulationTimeSeconds = flightPose?.simulationTimeSeconds ?? 0;
      lakeUniforms.time.value = simulationTimeSeconds;
      // Flight and title scenes share the lake's 0 m datum; the shoreline
      // transition mesh rises from that plane into valid land elevation data.
      water.position.set(flightPose?.datumPositionNed.east ?? 0, 0, -(flightPose?.datumPositionNed.north ?? 0));
      farWater.position.set(flightPose?.datumPositionNed.east ?? 0, 0, -(flightPose?.datumPositionNed.north ?? 0));
      farWaterBackingCenter.set(farWater.position.x, farWater.position.z, 0, 0);
      // Keep the light's orthographic shadow volume around the moving airframe.
      sun.target.position.copy(aircraftRoot.position);
      sun.position.copy(aircraftRoot.position).add(sunOffset);
      const visiblePanel = frame.panel.kind === "visible" ? frame.panel : null;
      const panel = visiblePanel?.panel ?? null;
      const panelPose = visiblePanel?.pose ?? IDENTITY_POSE;
      const panelCursor = visiblePanel?.cursor ?? null;
      const panelPoses = panel === null ? null : projectXrPanelCameraPoses(placement.rig, panelPose);
      if (useExternalCamera) {
        externalCameraRig.add(panelMesh);
        setPose(panelMesh, panelPoses?.trackingFromPanel ?? IDENTITY_POSE);
      } else {
        scene.add(panelMesh);
        setPose(panelMesh, panelPoses?.worldFromPanel ?? IDENTITY_POSE);
      }
      const overlayPanel = panel !== null && uiPanelComposition(panel.anchor) === "overlay";
      if (panelMaterial.transparent !== overlayPanel) {
        panelMaterial.transparent = overlayPanel;
        panelMaterial.needsUpdate = true;
      }
      panelMaterial.depthTest = !overlayPanel;
      panelMaterial.depthWrite = !overlayPanel;
      panelMesh.renderOrder = overlayPanel ? PANEL_OVERLAY_RENDER_ORDER : 0;
      gazeCursorMaterial.depthTest = !overlayPanel;
      for (const cursor of pointerCursors) cursor.renderOrder = overlayPanel ? PANEL_OVERLAY_RENDER_ORDER + 1 : 0;
      panelMesh.visible = visiblePanel !== null;
      if (panel !== currentPanel) {
        panelTexture.needsUpdate = true;
        currentPanel = panel;
      }
      const panelWidth = panel?.size.width ?? 2.4;
      const panelHeight = panel?.size.height ?? 1.8;
      panelMesh.scale.set(panelWidth / 2.4, panelHeight / 1.8, 1);
      const cursors = panelCursor === null ? [] : [panelCursor, ...(visiblePanel?.secondaryCursors ?? [])];
      while (pointerCursors.length < cursors.length) {
        const cursor = new Mesh(gazeCursorGeometry, gazeCursorMaterial);
        panelMesh.add(cursor);
        pointerCursors.push(cursor);
      }
      for (const [index, mesh] of pointerCursors.entries()) {
        const cursor = cursors[index];
        mesh.visible = cursor !== undefined && visiblePanel !== null;
        mesh.renderOrder = overlayPanel ? PANEL_OVERLAY_RENDER_ORDER + 1 : 0;
        if (cursor !== undefined) {
          const horizontalScale = panelWidth / 2.4;
          const verticalScale = panelHeight / 1.8;
          const scale = 0.5 + 0.5 * cursor.progress;
          mesh.position.set(cursor.point.x / horizontalScale, cursor.point.y / verticalScale, 0.015);
          mesh.scale.set(scale / horizontalScale, scale / verticalScale, 1);
        }
      }
      scene.updateMatrixWorld(true);
      const viewCamera = useExternalCamera ? fixedCamera : camera;
      screenAuto?.begin();
      let gpuSpanComplete = false;
      try {
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
        const reflectionPixelRatio = pixelRatio * (renderer.xr.isPresenting || stereoPresentation !== null ? 1 : screenRenderScale);
        reflectionTarget.setSize(Math.max(256, Math.min(1024, Math.ceil(width * reflectionPixelRatio / 2))),
          Math.max(128, Math.min(1024, Math.ceil(height * reflectionPixelRatio / 2))));
        const previousTarget = renderer.getRenderTarget();
        const previousBackground = scene.background;
        const previousAlpha = renderer.getClearAlpha();
        const previousXrEnabled = renderer.xr.enabled;
        const previousShadowAutoUpdate = renderer.shadowMap.autoUpdate;
        const previousShadowNeedsUpdate = renderer.shadowMap.needsUpdate;
        renderer.getClearColor(savedClearColor);
        try {
          if (sun.shadow.map !== null) {
            renderer.shadowMap.autoUpdate = false;
            renderer.shadowMap.needsUpdate = false;
          }
          scene.background = null;
          renderer.xr.enabled = false;
          renderer.setRenderTarget(reflectionTarget);
          renderer.setClearColor(0x000000, 0);
          renderer.clear();
          renderer.render(scene, reflectionCamera);
          reflectionEnabled.value = 1;
        } finally {
          renderer.shadowMap.autoUpdate = previousShadowAutoUpdate;
          renderer.shadowMap.needsUpdate = previousShadowNeedsUpdate;
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
      gpuSpanComplete = true;
      } finally { screenAuto?.end(gpuSpanComplete); }
      renderedTrackingMount = loopRunning && renderer.xr.isPresenting && visiblePanel !== null ? currentTrackingMountPose() : null;
    },
    resize(viewport: ViewportSize) {
      ensureActive(disposed);
      if (renderer.xr.isPresenting) return;
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ viewport }); return; }
      if (viewport.x !== width || viewport.y !== height || viewport.pixelRatio !== pixelRatio) screenAuto?.invalidate("resize");
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
      if (profile !== stereoPresentation) screenAuto?.invalidate("backend_changed");
      stereoPresentation = profile;
      camera.fov = profile?.verticalFieldOfViewDegrees ?? screenVerticalFieldOfViewDegrees;
      camera.focus = profile?.focusDistanceMeters ?? 10;
      if (profile !== null) stereoEffect.setEyeSeparation(profile.eyeSeparationMeters);
      camera.updateProjectionMatrix();
      if (width > 0 && height > 0 && !renderer.xr.isPresenting) resizeIfNeeded({ x: width, y: height, pixelRatio });
    },
    setScreenRenderScale(factor: number) {
      ensureActive(disposed);
      if (!Number.isFinite(factor) || factor <= 0 || factor > 1) throw new RangeError("Screen render scale must be within (0, 1]");
      if (screenAuto !== null) { screenAuto.selectManual(factor); return; }
      if (screenRenderScale === factor && pendingViewInputs?.screenRenderScale === undefined) return;
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ screenRenderScale: factor }); return; }
      if (!applyScreenRenderScale(factor)) screenRenderScale = factor;
    },
    setScreenRenderAuto(enabled: boolean) {
      ensureActive(disposed);
      if (screenAuto === null) throw new Error("Shared Screen Auto policy is unavailable");
      screenAuto.selectAuto(enabled);
    },
    dispose() {
      if (disposed) return;
      if (screenAuto !== null) {
        canvas.removeEventListener("webglcontextlost", onContextLost);
        canvas.removeEventListener("webglcontextrestored", onContextRestored);
      }
      try {
      if (loopRunning) renderer.setAnimationLoop(null);
      loopRunning = false;
      pendingViewInputs?.lakeQuality?.reject(new Error("Renderer was disposed before lake quality could be applied"));
      manualLakeQualityRequest?.reject(new Error("Renderer was disposed before lake quality could be applied"));
      manualLakeQualityRequest = null;
      pendingViewInputs = null;
      framePhase = "idle";
      cameraOrigin.dispose();
      cameraPlacement = null;
      frameViewport = null;
      renderedTrackingMount = null;
      panelTexture.dispose();
      headHudSurface?.dispose();
      gazeCursorGeometry.dispose();
      gazeCursorMaterial.dispose();
      panelGeometry.dispose();
      panelMaterial.dispose();
      disposeLakeChunks(waterGeometry);
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
      } finally {
        disposed = true;
        screenAuto?.dispose();
      }
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
    setPreparedFlightPose(pose: FlightRenderPose | null) {
      ensureActive(disposed);
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ preparedFlightPose: pose }); return; }
      preparedFlightPose = pose;
    },
    setLakeVisualCondition(condition: LakeVisualCondition) {
      ensureActive(disposed);
      if (framePhase === "view" || pendingViewInputs !== null) { stageViewInputs({ lake: condition }); return; }
      if (sameLakeVisualCondition(activeLakeCondition, condition)) return;
      const next = createLakeVisualResources(condition, activeLakeQuality);
      updateLakeChunkBounds(waterGeometry, next.displacement);
      lakeUniforms.windSpeed.value = next.windSpeed;
      lakeUniforms.windVelocity.value.set(condition.windEastMetersPerSecond, -condition.windNorthMetersPerSecond, 0, 0);
      lakeUniforms.detailScale.value = condition.detailAmplitudeScale;
      lakeUniforms.detailNear.value = next.near.texture;
      lakeUniforms.detailFar.value = next.far.texture;
      lakeUniforms.waveKAmplitude.value = next.waveKAmplitude;
      lakeUniforms.waveOmegaPhase.value = next.waveOmegaPhase;
      lakeUniforms.waveCount.value = next.waveCount;
      lakeUniforms.choppiness.value = next.choppiness;
      lakeUniforms.spacingGradient.value = next.spacingGradientMagnitude;
      lakeUniforms.visualWaveHeight.value = next.visualWaveHeight;
      lakeResources.near.texture.dispose();
      lakeResources.far.texture.dispose();
      lakeResources = next;
      activeLakeCondition = condition;
      screenAuto?.invalidate("quality_changed");
    },
    setLakeWaterQuality(quality: LakeWaterQuality) {
      return new Promise<LakeWaterQualityCleanupResult>((resolve, reject) => {
        ensureActive(disposed);
        if (!["low", "medium", "high"].includes(quality)) throw new RangeError("Invalid lake water quality");
        if (pendingViewInputs?.lakeQuality !== undefined || manualLakeQualityRequest !== null) throw new Error("Lake water quality is already pending");
        const request: LakeQualityRequest = Object.freeze({ quality, resolve, reject });
        if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ lakeQuality: request }); return; }
        applyLakeQuality(request);
      });
    },
    setLakeSkyCondition(condition: LakeSkyCondition | null) {
      ensureActive(disposed);
      const nextCondition = condition === null ? null : createLakeSkyCondition(condition);
      if (framePhase === "view" || pendingViewInputs !== null) { stageViewInputs({ sky: nextCondition }); return; }
      if (activeSkyCondition === null ? nextCondition === null : nextCondition !== null && sameLakeSkyRenderCondition(activeSkyCondition, nextCondition)) return;
      const [, hazeNear, hazeFar] = lakeSkyHaze(nextCondition?.visibilityMeters);
      const nextTexture = createLakeSkyTexture(nextCondition);
      const direction = nextCondition === null ? null : lakeSkySunDirectionNed(nextCondition);
      const daylight = nextCondition !== null && nextCondition.sunElevationDegrees >= 0 ? 1 : 0;
      const north = direction?.north ?? 0;
      const east = direction?.east ?? 0;
      const up = direction === null ? 1 : -direction.down;
      sunOffset.set(east, up, -north).multiplyScalar(sunDistanceMeters);
      sun.position.copy(sun.target.position).add(sunOffset);
      sun.intensity = lake_scene_sunlight_irradiance() * daylight;
      sun.castShadow = daylight === 1;
      lakeUniforms.sunDirection.value.set(east, up, -north, daylight);
      lakeFog.far = hazeFar;
      lakeFog.near = hazeNear;
      lakeUniforms.hazeRange.value.set(lakeFog.near, lakeFog.far, 0, 0);
      const previousTexture = skyTexture;
      scene.background = nextTexture;
      lakeUniforms.skyTexture.value = nextTexture;
      skyTexture = nextTexture;
      activeSkyCondition = nextCondition;
      screenAuto?.invalidate("quality_changed");
      previousTexture.dispose();
    },
    setLakeVenueVisible(visible: boolean) {
      ensureActive(disposed);
      if (framePhase === "view" || pendingViewInputs !== null) { stageViewInputs({ venueVisible: visible }); return; }
      venue.group.visible = visible;
    },
    setFlightCameraMode(mode: FlightCameraMode) {
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ cameraMode: mode }); return; }
      flightCameraMode = mode;
    },
    setCinematicCameraView(view: CinematicCameraView | null) {
      if (framePhase !== "idle" || pendingViewInputs !== null) { stageViewInputs({ cameraView: view }); return; }
      fixedCameraView = view;
    },
    projectTrackingFrame(pose: Pose) {
      return projectXrReferenceFrame(currentCameraPlacement().rig, pose);
    }
  };

  const webxr: WebXrSessionPort = {
    setInputFocusHandler(handler) { inputFocusHandler = handler; },
    readPointerFrame() {
      return { focused: hasXrInputFocus(), rays: framePointerRays.map((ray) => transformRay(ray, currentTrackingMountPose())) };
    },
    projectTrackingFrame(value: Pose) {
      return projectXrReferenceFrame(currentCameraPlacement().rig, value);
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
      session.addEventListener("visibilitychange", onInputFocusChange);
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
    webxr,
    async prepare(): Promise<void> {
      ensureActive(disposed);
      if (loopRunning || renderer.xr.isPresenting || stereoPresentation !== null) throw new Error("Initial renderer preparation requires an idle Screen renderer");
      screenAuto?.boundary(width > 0 && height > 0);
      const textures = new Set<Texture>([skyTexture]);
      scene.traverse(object => {
        if (!(object instanceof Mesh)) return;
        const mesh = object as Mesh;
        for (const material of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
          const values: readonly unknown[] = Object.values(material);
          for (const value of values) if (isTexture(value)) textures.add(value);
          if (material instanceof ShaderMaterial) for (const uniform of Object.values(material.uniforms)) {
            if (isTexture(uniform.value)) textures.add(uniform.value);
          }
        }
      });
      for (const texture of textures) if (!texture.isRenderTargetTexture) renderer.initTexture(texture);
      renderer.initRenderTarget(reflectionTarget);
      scene.updateMatrixWorld(true);
      await renderer.compileAsync(scene, camera);
      ensureActive(disposed);
      const previousTarget = renderer.getRenderTarget();
      const previousBackground = scene.background;
      const previousXrEnabled = renderer.xr.enabled;
      let compilation: Promise<unknown>;
      try {
        scene.background = null;
        renderer.xr.enabled = false;
        renderer.setRenderTarget(reflectionTarget);
        compilation = renderer.compileAsync(scene, reflectionCamera);
      } finally {
        renderer.setRenderTarget(previousTarget);
        renderer.xr.enabled = previousXrEnabled;
        scene.background = previousBackground;
      }
      await compilation;
      ensureActive(disposed);
    }
  });

  function clearSession(session = sessionFromState(xrState)): void {
    if (session !== null) {
      session.removeEventListener("select", onSelect);
      session.removeEventListener("visibilitychange", onInputFocusChange);
      session.removeEventListener("end", onSessionEnd);
    }
    activeReferenceSpace?.removeEventListener("reset", onReferenceSpaceReset);
    activeReferenceSpace = null;
    cameraOrigin.dispose();
    cameraPlacement = null;
    xrState = { type: "idle" };
    renderer.xr.enabled = false;
    renderedTrackingMount = null;
    framePointerRays = [];
    pointerSources = new WeakMap();
    nextPointerSource = 1;
  }

  function hasXrInputFocus(): boolean {
    const session = sessionFromState(xrState);
    return session !== null && session.visibilityState !== "hidden" && session.visibilityState !== "visible-blurred";
  }

  function pointerSource(source: XRInputSource): NonNullable<SelectRay["source"]> {
    if (source.targetRayMode !== "tracked-pointer") return { kind: "gaze" };
    let identifier = pointerSources.get(source);
    if (identifier === undefined) { identifier = nextPointerSource++; pointerSources.set(source, identifier); }
    return { kind: "controller", id: identifier };
  }

  function isAttaching(): boolean {
    return xrState.type === "attaching";
  }

  function updateFixedCameraProjection(): void {
    const fixedView = usesExternalCamera() && flightCameraMode !== "chase" ? fixedCameraView : null;
    const fieldOfView = fixedView?.verticalFieldOfViewDegrees ?? screenVerticalFieldOfViewDegrees;
    if (fixedCamera.fov === fieldOfView) return;
    fixedCamera.fov = fieldOfView;
    fixedCamera.updateProjectionMatrix();
  }

  function hasScreenRenderArea(factor: number): boolean {
    return Math.floor(width * pixelRatio * factor) >= 1 && Math.floor(height * pixelRatio * factor) >= 1;
  }

  function applyScreenRenderScale(factor: number): boolean {
    if (!hasScreenRenderArea(factor)) return false;
    if (factor === screenRenderScale) return true;
    const previous = screenRenderScale;
    const previousReflection = { width: reflectionTarget.width, height: reflectionTarget.height };
    try {
      screenRenderScale = factor;
      if (width > 0 && height > 0 && !renderer.xr.isPresenting && stereoPresentation === null) {
        resizeIfNeeded({ x: width, y: height, pixelRatio });
        reflectionTarget.setSize(Math.max(256, Math.min(1024, Math.ceil(width * pixelRatio * factor / 2))),
          Math.max(128, Math.min(1024, Math.ceil(height * pixelRatio * factor / 2))));
      }
      return true;
    } catch (error: unknown) {
      screenRenderScale = previous;
      try {
        if (width > 0 && height > 0 && !renderer.xr.isPresenting && stereoPresentation === null) resizeIfNeeded({ x: width, y: height, pixelRatio });
        reflectionTarget.setSize(previousReflection.width, previousReflection.height);
      } catch (rollback: unknown) {
        throw new AggregateError([error, rollback], "Screen render scale application and rollback failed", { cause: rollback });
      }
      throw error;
    }
  }

  function resizeIfNeeded(viewport: ViewportSize): void {
    if (viewport.x <= 0 || viewport.y <= 0) return;
    const drawingRatio = viewport.pixelRatio * (stereoPresentation !== null ? 1 : screenRenderScale);
    const drawable = Math.floor(viewport.x * drawingRatio) > 0 && Math.floor(viewport.y * drawingRatio) > 0;
    if (viewport.x === width && viewport.y === height && viewport.pixelRatio === pixelRatio && (!drawable || renderer.getPixelRatio() === drawingRatio)) return;
    width = viewport.x;
    height = viewport.y;
    pixelRatio = viewport.pixelRatio;
    if (drawable) {
      renderer.setPixelRatio(drawingRatio);
      if (stereoPresentation !== null) stereoEffect.setSize(width, height);
      else renderer.setSize(width, height, false);
    }
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    titleCameraPose = titleScreenCameraPoseForViewport(width, height, screenVerticalFieldOfViewDegrees);
    titlePhoneCameraPose = titleCameraPoseForViewport(width, height, 0, screenVerticalFieldOfViewDegrees);
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
    ...(ray.source === undefined ? {} : { source: ray.source }),
    origin: vec3(offset.x + worldFromTracking.position.x, offset.y + worldFromTracking.position.y, offset.z + worldFromTracking.position.z),
    direction: rotateVec3(worldFromTracking.orientation, ray.direction),
    timestampMs: ray.timestampMs
  });
}

function ensureActive(disposed: boolean): void {
  if (disposed) throw new Error("Three.js renderer has been disposed");
}

function isTexture(value: unknown): value is Texture {
  return value instanceof Texture;
}

function lakeUniform<T>(material: ShaderMaterial, name: string, expectedValue: T): { value: T } {
  const uniform = material.uniforms[name];
  if (uniform === undefined) throw new Error(`Lake shader uniform is missing: ${name}`);
  if (typeof uniform.value !== typeof expectedValue) throw new TypeError(`Lake shader uniform has an unexpected value type: ${name}`);
  return uniform as { value: T };
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

const lakeWaterVertexShader = /* glsl */ `
uniform float uTimeSeconds;
uniform vec4 uWindVelocity;
uniform float uDetailScale;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform vec4 uDetailExtents;
uniform float uChoppiness;
uniform float uGridSpacingGradient;
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying vec3 vSurfaceState;
varying vec4 vSurfaceGradient;
#include <shadowmap_pars_vertex>
${LAKE_WAVE_PROJECTION_GLSL}

void main() {
  // PlaneGeometry's XY winding points +Z; reversing its second axis while
  // mapping to world XZ keeps the resulting top face wound toward +Y.
  vec3 p = vec3(position.x, 0.0, -position.y);
  vec2 waveXZ = (modelMatrix * vec4(p, 1.0)).xz;
  vSurfaceGradient.xy = waveXZ;
  vec2 detailDrift = uWindVelocity.xy * uTimeSeconds;
  vec4 nearDetail = texture2D(uDetailNear, (waveXZ - detailDrift * 0.42) / uDetailExtents.x);
  vec4 farDetail = texture2D(uDetailFar, (waveXZ - detailDrift * 0.89) / uDetailExtents.y);
  float nearHeight = nearDetail.a - 128.0 / 255.0;
  float farHeight = farDetail.a - 128.0 / 255.0;
  float nearVisibility = 1.0 - smoothstep(0.12, 0.32, aGridSpacing);
  float farVisibility = 1.0 - smoothstep(0.8, 2.8, aGridSpacing);
  p.y += (nearHeight * 0.6 * nearVisibility + farHeight * 0.55 * farVisibility) * uDetailScale;
  vec2 nearSlope = (nearDetail.rg * 255.0 - 128.0) / 127.0;
  vec2 farSlope = (farDetail.rg * 255.0 - 128.0) / 127.0;
  vec2 resolvedDetailSlope = (nearSlope * (0.6 * nearVisibility)
    + farSlope * (0.55 * farVisibility)) * uDetailScale;
  LakeWaveProjection projected = projectLakeWaves(p, waveXZ, aGridSpacing,
    uGridSpacingGradient, uTimeSeconds, uChoppiness);
  p = projected.position;
  float dxx = projected.horizontalJacobian.x;
  float dxz = projected.horizontalJacobian.y;
  float dzx = projected.horizontalJacobian.z;
  float dzz = projected.horizontalJacobian.w;
  vec2 heightGradient = projected.heightGradient + resolvedDetailSlope;
  float horizontalDeterminant = dxx * dzz - dxz * dzx;
  // The geometric normal is measured after Gerstner horizontal displacement.
  // Convert the represented detail slope into that same coordinate system.
  vSurfaceGradient.zw = vec2(
    dzz * resolvedDetailSlope.x - dzx * resolvedDetailSlope.y,
    -dxz * resolvedDetailSlope.x + dxx * resolvedDetailSlope.y
  ) / max(horizontalDeterminant, 0.01);
  vec3 tangentX = vec3(dxx, heightGradient.x, dzx);
  vec3 tangentZ = vec3(dxz, heightGradient.y, dzz);
  vec3 localNormal = normalize(cross(tangentZ, tangentX));
  vec4 worldPosition = modelMatrix * vec4(p, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * localNormal);
  #if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
    vDirectionalShadowCoord[0] = directionalShadowMatrix[0] *
      (worldPosition + vec4(vWorldNormal * directionalLightShadows[0].shadowNormalBias, 0.0));
  #endif
  vSurfaceState = vec3(projected.crest / max(projected.slopeEnergy, 0.001),
    clamp(1.0 - horizontalDeterminant, 0.0, 1.0), aGridSpacing);
  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
`;

const lakeFilteredWaterVertexShader = /* glsl */ `
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying vec3 vSurfaceState;
varying vec4 vSurfaceGradient;
#include <shadowmap_pars_vertex>
void main() {
  vec4 worldPosition = modelMatrix * vec4(position.x, 0.0, -position.y, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * vec3(0.0, 1.0, 0.0));
  vSurfaceState = vec3(0.0, 0.0, aGridSpacing);
  vSurfaceGradient = vec4(worldPosition.xz, 0.0, 0.0);
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
uniform mat4 uReflectionMatrix;
uniform sampler2D uDetailNear;
uniform sampler2D uDetailFar;
uniform sampler2D uSkyTexture;
uniform vec4 uDetailExtents;
uniform float uWaterBackingClip;
uniform vec4 uWaterBackingCenter;
uniform float uWaterBackingInnerExtent;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying vec3 vSurfaceState;
varying vec4 vSurfaceGradient;
#include <common>
#include <shadowmap_pars_fragment>
${LAKE_WAVE_PACKET_GLSL}

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
  int componentCount = min(uWaveCount, 24);
  for (int i = 0; i < componentCount; i++) {
    vec4 ka = uWaveKAmplitude[i];
    vec4 op = uWaveOmegaPhase[i];
    float k = ka.z;
    vec2 direction = ka.xy;
    mat3 modulation = lakeWaveModulation(worldXZ, direction, k, op.y);
    vec3 warp = modulation[1];
    float phase = k * dot(direction, worldXZ) + warp.x - op.x * uTimeSeconds + op.y;
    float geometricVisibility = 1.0 - smoothstep(1.3, 2.5, k * vSurfaceState.z);
    float pixelVisibility = 1.0 - smoothstep(2.0, 5.0, fwidth(phase));
    float normalWeight = (1.0 - geometricVisibility) * pixelVisibility * 0.65;
    if (normalWeight != 0.0) {
      vec2 phaseGradient = k * direction + warp.yz;
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
  vec2 totalSlope = geometricSlope + waveSlope + microSlope - vSurfaceGradient.zw;
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
  float glitter = glitterLobe * glitterNoise * smoothstep(0.35, 2.0, uWindSpeed) * sunVisibility * uSunDirection.w;
  float diffuse = 0.58 + 0.42 * sunVisibility * max(dot(normal, normalize(uSunDirection.xyz)), 0.0) * uSunDirection.w;
  float sharpCrest = smoothstep(0.42, 0.76, vSurfaceState.y);
  float crestLight = smoothstep(0.05, 1.05, vSurfaceState.x) * sharpCrest * mix(0.35, 1.0, patches);
  vec3 base = mix(uWaterDark, uWaterMid, diffuse * 0.72);
  base = mix(base, uWaterLight, crestLight * 0.16 + patches * 0.07);
  base *= mix(0.82, 1.0, sunVisibility);
  // Existing wave groups may remain steep after the local wind weakens.
  float whitecap = smoothstep(0.68, 0.84, vSurfaceState.y) * smoothstep(0.7, 0.98, vSurfaceState.x) * 0.12;
  // Dark water absorption must not also darken the reflected sky.
  base *= vec3(0.29, 0.34, 0.37);
  // Art-directed reflection strength for the dark reference water. This is
  // not a rough-surface BRDF and leaves a visible sky/water boundary.
  vec3 color = mix(base, reflection, fresnel * 0.45);
  if (uReflectionEnabled > 0.5) {
    vec4 reflectionCoord = uReflectionMatrix * vec4(vWorldPosition, 1.0);
    if (reflectionCoord.w > 0.0) {
      vec2 reflectedUv = reflectionCoord.xy / reflectionCoord.w * 0.5 + 0.5;
    // Small surface-slope offsets break up the image along moving wave facets.
      reflectedUv += totalSlope * 0.006;
      if (all(greaterThanEqual(reflectedUv, vec2(0.0))) &&
          all(lessThanEqual(reflectedUv, vec2(1.0)))) {
        vec4 aircraftReflection = texture2D(uReflectionTexture, reflectedUv);
        color = mix(color, aircraftReflection.rgb,
          aircraftReflection.a * clamp(0.16 + fresnel * 0.7, 0.0, 0.72));
      }
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
      all(lessThan(abs(vSurfaceGradient.xy - uWaterBackingCenter.xy), vec2(uWaterBackingInnerExtent)))) discard;
  vec2 footprintX = dFdx(vSurfaceGradient.xy) * 0.25;
  vec2 footprintY = dFdy(vSurfaceGradient.xy) * 0.25;
  vec2 center = vSurfaceGradient.xy;
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
