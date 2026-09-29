import {
  BufferAttribute,
  CanvasTexture,
  CircleGeometry,
  Color,
  DoubleSide,
  Group,
  LinearFilter,
  BoxGeometry,
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
import { IDENTITY_POSE, quaternion, vec3 } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import { composePose } from "../../contracts/math.js";
import type { WebXrAvailability, WebXrSessionPort, WebXrSessionRequest } from "../../../presentation/webxr-contracts.js";
import { selectRayFromXrEvent } from "./xr-select-ray.js";
import { flightRelativePose } from "./flight-pose.js";
import { pilotEyePoseThree, poseFrdToThree, SYNTHETIC_PILOT_EYE_POINT } from "../../camera/pilot-eye-point.js";
import { replayCameraPoseFrd } from "../../camera/replay-camera.js";
import { createLakeWaveSpectrum, lakeWaterQualityProfile } from "../../contracts/lake-water.js";
import type { LakeWaterQuality } from "../../contracts/lake-water.js";
import { createLakeDetailLayer } from "./lake-detail-texture.js";

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
// A single reference appearance is shared by the title, flight, and replay
// scenes until the scenario provides a coherent wave-state descriptor.
const LAKE_REFERENCE_WIND_NORTH = 1.2;
const LAKE_REFERENCE_WIND_EAST = 2.4;

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
  scene.background = new Color(0x9fb0ad);

  const waterQuality = lakeWaterQualityProfile(lakeQuality);
  const waterGeometry = createLakeGeometry(waterQuality.meshSegments);
  const lakeWaves = createLakeWaveSpectrum(LAKE_REFERENCE_WIND_NORTH, LAKE_REFERENCE_WIND_EAST, 600, waterQuality.componentCount);
  const referenceWindSpeed = Math.hypot(LAKE_REFERENCE_WIND_NORTH, LAKE_REFERENCE_WIND_EAST);
  const detailDirectionX = LAKE_REFERENCE_WIND_EAST / referenceWindSpeed;
  const detailDirectionZ = -LAKE_REFERENCE_WIND_NORTH / referenceWindSpeed;
  const detailNear = createLakeDetailLayer(64, 2300, 1717, detailDirectionX, detailDirectionZ);
  const detailFar = createLakeDetailLayer(93, 1800, 2917, detailDirectionX, detailDirectionZ);
  const waveKAmplitude = Array.from({ length: 24 }, (_, index) => {
    const wave = lakeWaves.components[index];
    return wave === undefined ? new Vector4() : new Vector4(
      wave.directionEast,
      -wave.directionNorth,
      wave.waveNumberRadiansPerMeter,
      wave.amplitudeMeters * lakeVisualAmplitudeScale(wave.waveNumberRadiansPerMeter)
    );
  });
  const waveOmegaPhase = Array.from({ length: 24 }, (_, index) => {
    const wave = lakeWaves.components[index];
    return wave === undefined ? new Vector4() : new Vector4(
      wave.angularFrequencyRadiansPerSecond,
      wave.phaseRadians,
      0,
      0
    );
  });
  const waterMaterial = new ShaderMaterial({
    uniforms: {
      uTimeSeconds: { value: 0 },
      uWindSpeed: { value: referenceWindSpeed },
      uWindDirection: { value: new Vector4(detailDirectionX, detailDirectionZ, 0, 0) },
      uDetailNear: { value: detailNear.texture },
      uDetailFar: { value: detailFar.texture },
      uDetailExtents: { value: new Vector4(detailNear.extentMeters, detailFar.extentMeters, 0, 0) },
      uWaveKAmplitude: { value: waveKAmplitude },
      uWaveOmegaPhase: { value: waveOmegaPhase },
      uWaveCount: { value: lakeWaves.components.length },
      uVisualWaveHeight: { value: lakeWaves.significantWaveHeightMeters * LAKE_VISUAL_HEIGHT_SCALE },
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
    time: lakeUniform(waterMaterial, "uTimeSeconds", 0)
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
  const camera = new PerspectiveCamera(60, 1, 0.05, 2000);
  aircraftRoot.add(camera);
  const cockpitMaterial = new MeshBasicMaterial({ color: 0x343f3d });
  const cockpitWing = new Mesh(new BoxGeometry(3.8, 0.055, 0.24), cockpitMaterial);
  cockpitWing.position.set(0, -0.58, -1.25);
  aircraftRoot.add(cockpitWing);
  const cockpitNose = new Mesh(new BoxGeometry(0.16, 0.12, 1.7), cockpitMaterial);
  cockpitNose.position.set(0, -0.5, -1.95);
  aircraftRoot.add(cockpitNose);
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

  const onSelect = (event: XRInputSourceEvent): void => {
    if (xrState.type !== "active" && xrState.type !== "attaching") return;
    const referenceSpace = renderer.xr.getReferenceSpace();
    if (referenceSpace === null || selectRayHandler === null) return;
    const ray = selectRayFromXrEvent(event.frame, event.inputSource, referenceSpace);
    if (ray !== null) selectRayHandler(ray);
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
      const useChaseCamera = flightPose !== null && flightCameraMode === "chase" && !renderer.xr.isPresenting && stereoPresentation === null;
      if (useChaseCamera) {
        setPose(camera, poseFrdToThree(replayCameraPoseFrd("chase")));
      } else {
        const pilotEyePose = flightPose === null
          ? IDENTITY_POSE
          : pilotEyePoseThree(
            SYNTHETIC_PILOT_EYE_POINT,
            flightPose.pilotPositionMeters,
            flightPose.initialPilotPositionMeters
          );
        setPose(camera, composePose(pilotEyePose, frame.cameraPose));
      }
      setPose(aircraftRoot, flightPose === null ? IDENTITY_POSE : flightRelativePose(flightPose, IDENTITY_POSE));
      const simulationTimeSeconds = flightPose?.simulationTimeSeconds ?? 0;
      lakeUniforms.time.value = simulationTimeSeconds;
      // The non-flight scenic camera is at the origin. Keep its water below
      // eye level so amplified wave crests do not cut across the horizon.
      water.position.set(flightPose?.datumPositionNed.east ?? 0, flightPose === null ? -1.5 : 0, -(flightPose?.datumPositionNed.north ?? 0));
      setPose(panelMesh, flightPose === null ? frame.panelPose : flightRelativePose(flightPose, frame.panelPose));
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
        stereoEffect.render(scene, camera);
        renderer.setViewport(0, 0, width, height);
        renderer.setScissor(0, 0, width, height);
        renderer.setScissorTest(false);
      } else {
        renderer.render(scene, camera);
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
      detailNear.texture.dispose();
      detailFar.texture.dispose();
      cockpitWing.geometry.dispose();
      cockpitNose.geometry.dispose();
      cockpitMaterial.dispose();
      renderer.dispose();
      disposed = true;
    },
    setSelectRayHandler(handler) {
      selectRayHandler = handler;
    },
    setFlightPose(pose: FlightRenderPose | null) {
      flightPose = pose;
    },
    setFlightCameraMode(mode: FlightCameraMode) {
      flightCameraMode = mode;
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
  }
}

function setPose(object: Object3D, pose: Pose): void {
  object.position.set(pose.position.x, pose.position.y, pose.position.z);
  object.quaternion.set(pose.orientation.x, pose.orientation.y, pose.orientation.z, pose.orientation.w);
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
  return 1.8 + (LAKE_VISUAL_HEIGHT_SCALE - 1.8) * smoothWeight;
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
uniform vec4 uWaveKAmplitude[24];
uniform vec4 uWaveOmegaPhase[24];
uniform int uWaveCount;
attribute float aGridSpacing;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying float vCrest;
varying float vCompression;
varying float vGridSpacing;
${lakeWavePacketShader}

void main() {
  // PlaneGeometry's XY winding points +Z; reversing its second axis while
  // mapping to world XZ keeps the resulting top face wound toward +Y.
  vec3 p = vec3(position.x, 0.0, -position.y);
  vec2 waveXZ = (modelMatrix * vec4(p, 1.0)).xz;
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
  vec3 tangentX = vec3(dxx, dhdx, dzx);
  vec3 tangentZ = vec3(dxz, dhdz, dzz);
  vec3 localNormal = normalize(cross(tangentZ, tangentX));
  vec4 worldPosition = modelMatrix * vec4(p, 1.0);
  vWorldPosition = worldPosition.xyz;
  vWorldNormal = normalize(mat3(modelMatrix) * localNormal);
  vCrest = crest / max(slopeEnergy, 0.001);
  vCompression = clamp(1.0 - (dxx * dzz - dxz * dzx), 0.0, 1.0);
  vGridSpacing = aGridSpacing;
  gl_Position = projectionMatrix * viewMatrix * worldPosition;
}
`;

const lakeWaterFragmentShader = /* glsl */ `
uniform float uWindSpeed;
uniform vec4 uWindDirection;
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
uniform vec4 uDetailExtents;
varying vec3 vWorldPosition;
varying vec3 vWorldNormal;
varying float vCrest;
varying float vCompression;
varying float vGridSpacing;
${lakeWavePacketShader}

vec3 analyticSky(vec3 ray) {
  float elevation = clamp(ray.y * 0.5 + 0.5, 0.0, 1.0);
  vec3 horizon = vec3(0.54, 0.64, 0.66);
  vec3 zenith = vec3(0.18, 0.36, 0.52);
  vec3 sky = mix(horizon, zenith, pow(elevation, 0.72));
  float sun = pow(max(dot(normalize(ray), normalize(uSunDirection.xyz)), 0.0), 900.0);
  float glow = pow(max(dot(normalize(ray), normalize(uSunDirection.xyz)), 0.0), 32.0);
  return sky + vec3(1.0, 0.72, 0.43) * (sun * 12.0 + glow * 0.12);
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
  vec2 drift = uWindDirection.xy * uTimeSeconds;
  vec4 nearSample = texture2D(uDetailNear, (worldXZ - drift * 0.42) / uDetailExtents.x);
  vec4 farSample = texture2D(uDetailFar, (worldXZ - drift * 0.23) / uDetailExtents.y);
  vec2 nearSlope = nearSample.rg * 2.0 - 1.0;
  vec2 farSlope = farSample.rg * 2.0 - 1.0;
  vec2 microSlope = nearSlope * 0.65 + farSlope * 0.4;
  // Blue stores slope squared. Mipmaps preserve the variance of unresolved
  // wavelets even after their mean slope approaches zero.
  float nearVariance = max(nearSample.b - dot(nearSlope, nearSlope), 0.0);
  float farVariance = max(farSample.b - dot(farSlope, farSlope), 0.0);
  float slopeMoment = dot(microSlope, microSlope)
    + 0.65 * 0.65 * nearVariance + 0.4 * 0.4 * farVariance;
  // Preserve visible fine-scale contrast as the pilot eye rises above the
  // water; projected waves otherwise lose nearly all of their normal detail.
  float eyeHeight = max(cameraPosition.y - vWorldPosition.y, 0.0);
  float detailGain = 1.0 + 2.0 * smoothstep(1.5, 7.0, eyeHeight);
  float strength = smoothstep(0.05, 0.6, uWindSpeed) * detailGain;
  return vec3(microSlope * strength, slopeMoment * strength * strength);
}

vec3 sampleLakeColor(vec2 worldXZ, vec2 waveSlope, vec3 microDetail) {
  vec3 geometricNormal = normalize(vWorldNormal);
  float patches = lakeNoise(worldXZ * 0.17 + vec2(2.3, -7.1));
  vec2 microSlope = microDetail.xy;
  float unresolvedVariance = max(microDetail.z - dot(microSlope, microSlope), 0.0);
  vec3 normal = normalize(geometricNormal + vec3(-waveSlope.x - microSlope.x, 0.0, -waveSlope.y - microSlope.y));
  vec3 viewDirection = normalize(cameraPosition - vec3(worldXZ.x, vWorldPosition.y, worldXZ.y));
  vec3 reflectedDirection = reflect(-viewDirection, normal);
  vec3 reflection = analyticSky(reflectedDirection);
  float ndv = max(dot(normal, viewDirection), 0.0);
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
  vec3 color = mix(base, reflection, 0.08 + 0.42 * fresnel);
  float depthTint = clamp(uVisualWaveHeight / 0.35, 0.0, 1.0);
  color = mix(color, color * vec3(0.92, 0.97, 1.04), depthTint * 0.12);
  float distanceToEye = length(cameraPosition.xz - worldXZ);
  // Match the dark water in the visual reference without dimming the sky.
  color *= vec3(0.29, 0.34, 0.37);
  color += vec3(1.0, 0.78, 0.52) * glitter * 0.7;
  color = mix(color, vec3(0.38, 0.43, 0.43), whitecap * 0.28);
  // Airlight belongs after the water tint. Tinting the haze made the horizon
  // darker than the foreground and produced a black horizontal band.
  float atmosphericFade = smoothstep(300.0, 1600.0, distanceToEye);
  vec3 hazeColor = vec3(0.12, 0.18, 0.20);
  color = mix(color, hazeColor, atmosphericFade * 0.16);
  return color;
}

void main() {
  vec2 footprintX = dFdx(vWorldPosition.xz) * 0.25;
  vec2 footprintY = dFdy(vWorldPosition.xz) * 0.25;
  vec2 center = vWorldPosition.xz;
  vec2 waveSlope = lakeWaveSlope(center);
  // Supersample subpixel normal detail, then shade the filtered normal once.
  vec3 microDetail = (
    lakeMicroDetail(center - footprintX - footprintY) +
    lakeMicroDetail(center + footprintX - footprintY) +
    lakeMicroDetail(center - footprintX + footprintY) +
    lakeMicroDetail(center + footprintX + footprintY)
  ) * 0.25;
  vec3 color = sampleLakeColor(center, waveSlope, microDetail);
  gl_FragColor = vec4(color, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;
