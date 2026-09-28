import {
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

export function createThreeRenderer(
  canvas: HTMLCanvasElement,
  panelCanvas: HTMLCanvasElement,
  xrSystem: XRSystem | null
): ThreeRendererBundle {
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
  const stereoEffect = new StereoEffect(renderer);
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.setClearColor(0x8aadb0, 1);

  const scene = new Scene();
  scene.background = new Color(0x8aadb0);

  const water = new Mesh(
    new PlaneGeometry(600, 600),
    new MeshBasicMaterial({ color: 0x527e82 })
  );
  water.rotation.x = -Math.PI / 2;
  water.position.y = 0;
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
      water.geometry.dispose();
      water.material.dispose();
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
