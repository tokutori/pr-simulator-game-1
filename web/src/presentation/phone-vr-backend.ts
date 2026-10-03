import {
  composePose,
  IDENTITY_POSE,
  inversePose,
  multiplyQuaternion,
  quaternion,
  rotateVec3,
  vec3
} from "../render/contracts/math.js";
import type { Pose, Quaternion } from "../render/contracts/math.js";
import type { BackendFrame, PresentationBackendAdapter, RendererAdapter, ViewportSize } from "../render/contracts/runtime.js";
import type { UiActionDispatcher, UiViewModel } from "../render/contracts/ui.js";
import { MenuAnchorPlacement, placeMenuPanel, resolveAnchorPose } from "../render/anchors.js";
import { GazeDwellSelector } from "./gaze-dwell.js";
import { intersectPanel } from "./panel-interaction.js";
import type { PhoneVrAvailability, PhoneVrGamepadInputPort, PhoneVrOpticalProfile, PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "./phone-vr-contracts.js";
import { NO_PHONE_VR_GAMEPAD_INPUT, PHONE_VR_OPTICAL_PROFILE } from "./phone-vr-contracts.js";
import { phoneOrientationQuaternion } from "./phone-vr-orientation.js";
import { GamepadUiSelector } from "./gamepad-ui-selector.js";

type PhoneVrBackendState =
  | { readonly type: "idle" }
  | { readonly type: "requesting" }
  | { readonly type: "requested" }
  | { readonly type: "starting"; readonly attempt: PhoneVrTrackingAttempt }
  | { readonly type: "active"; readonly attempt: PhoneVrTrackingAttempt }
  | { readonly type: "stopping" }
  | { readonly type: "failed"; readonly message: string };

interface PhoneVrTrackingAttempt {
  sampleResult:
    | { readonly type: "waiting" }
    | { readonly type: "ready" }
    | { readonly type: "failed"; readonly error: Error };
  timer:
    | { readonly type: "cleared" }
    | { readonly type: "pending"; readonly handle: ReturnType<typeof globalThis.setTimeout> };
  resources: "owned" | "released";
  readonly signalSample: () => void;
}

export interface PhoneVrBackendOptions {
  readonly opticalProfile?: PhoneVrOpticalProfile;
  readonly firstSampleTimeoutMs?: number;
  readonly gamepadInput?: PhoneVrGamepadInputPort;
  readonly nowMs?: () => number;
}

export class PhoneVrPresentationBackend implements PresentationBackendAdapter {
  readonly mode = "phone-vr" as const;
  private state: PhoneVrBackendState = { type: "idle" };
  private requestGeneration = 0;
  private screenOrientationAngle: number | null = null;
  private latestRawReading: PhoneVrSensorReading | null = null;
  private latestSensorOrientation: Quaternion | null = null;
  private latestViewerOrientation: Quaternion = IDENTITY_POSE.orientation;
  private calibrationOrientation: Quaternion | null = null;
  private latestSampleTimestampMs: number | null = null;
  private referenceFromWorld: Pose = IDENTITY_POSE;
  private menuPlacement = new MenuAnchorPlacement();
  private gazeDwell: GazeDwellSelector;
  private gamepadSelector: GamepadUiSelector;
  private readonly opticalProfile: PhoneVrOpticalProfile;
  private readonly firstSampleTimeoutMs: number;
  private readonly gamepadInput: PhoneVrGamepadInputPort;
  private readonly nowMs: () => number;

  constructor(
    private readonly sensors: PhoneVrSensorPort,
    private readonly renderer: RendererAdapter,
    private readonly viewport: () => ViewportSize,
    private readonly dispatch: UiActionDispatcher,
    private readonly onTrackingUnavailable: (message: string) => void,
    options: PhoneVrBackendOptions = {}
  ) {
    const opticalProfile = options.opticalProfile ?? PHONE_VR_OPTICAL_PROFILE;
    const firstSampleTimeoutMs = options.firstSampleTimeoutMs ?? 3000;
    if (!Number.isFinite(firstSampleTimeoutMs) || firstSampleTimeoutMs <= 0) {
      throw new RangeError("Phone VR first-sample timeout must be positive and finite");
    }
    this.firstSampleTimeoutMs = firstSampleTimeoutMs;
    this.opticalProfile = opticalProfile;
    this.gamepadInput = options.gamepadInput ?? NO_PHONE_VR_GAMEPAD_INPUT;
    this.nowMs = options.nowMs ?? (() => performance.now());
    this.gazeDwell = new GazeDwellSelector(dispatch);
    this.gamepadSelector = new GamepadUiSelector(dispatch);
  }

  checkAvailability(): Promise<PhoneVrAvailability> {
    return this.sensors.checkAvailability();
  }

  requestPermissionFromUserGesture(): Promise<PhoneVrPermissionResult> {
    if (this.state.type !== "idle" && this.state.type !== "failed") {
      return Promise.resolve({ ok: false, message: "A Phone VR sensor session is already active or stopping" });
    }
    const generation = ++this.requestGeneration;
    this.state = { type: "requesting" };
    let permission: Promise<PhoneVrPermissionResult>;
    try {
      permission = this.sensors.requestPermissionFromUserGesture();
    } catch (error) {
      const message = `Phone VR permission request failed: ${errorMessage(error)}`;
      this.state = { type: "failed", message };
      return Promise.resolve({ ok: false, message });
    }
    return permission.then((result) => {
      if (generation !== this.requestGeneration) return { ok: false, message: "Phone VR permission request was canceled" };
      this.state = result.ok ? { type: "requested" } : { type: "failed", message: result.message };
      return result;
    }, (error: unknown) => {
      const message = `Phone VR permission request failed: ${errorMessage(error)}`;
      if (generation === this.requestGeneration) this.state = { type: "failed", message };
      return { ok: false, message };
    });
  }

  async start(): Promise<void> {
    if (this.state.type !== "requested") throw new Error("Phone VR sensors require an explicit user permission action");
    const firstSample = new Promise<PhoneVrTrackingAttempt>((resolve) => {
      const attempt: PhoneVrTrackingAttempt = {
        sampleResult: { type: "waiting" },
        timer: { type: "cleared" },
        resources: "owned",
        signalSample: () => { resolve(attempt); }
      };
      this.state = { type: "starting", attempt };
      this.resetTracking();
      this.menuPlacement = new MenuAnchorPlacement();
      try {
        this.screenOrientationAngle = this.sensors.getScreenOrientationAngle();
        if (this.screenOrientationAngle === null || !Number.isFinite(this.screenOrientationAngle)) {
          throw new Error("Phone VR screen orientation is unavailable");
        }
        if (!this.acceptsAttempt(attempt)) return;
        attempt.timer = { type: "pending", handle: setTimeout(() => {
          if (this.acceptsAttempt(attempt) && attempt.sampleResult.type === "waiting") {
            this.failStartup(attempt, new Error("Phone VR received no valid orientation event before timeout"));
          }
        }, this.firstSampleTimeoutMs) };
        this.renderer.setStereoPresentation(this.opticalProfile);
        if (!this.acceptsAttempt(attempt)) return;
        this.sensors.startListening(
          (reading) => { if (this.acceptsAttempt(attempt)) this.handleSensorReading(reading); },
          (angle) => { if (this.acceptsAttempt(attempt)) this.handleScreenOrientationChange(angle); }
        );
      } catch (error) {
        this.failStartup(attempt, startupError(error));
      }
    });
    const attempt = await firstSample;
    try {
      if (attempt.sampleResult.type === "failed") throw attempt.sampleResult.error;
      if (!this.isStartingAttempt(attempt) ||
          attempt.sampleResult.type !== "ready" || this.latestSensorOrientation === null || this.latestSampleTimestampMs === null) {
        throw new Error("Phone VR tracking ended during startup");
      }
      this.state = { type: "active", attempt };
    } catch (error) {
      if (this.ownsAttempt(attempt)) {
        this.state = { type: "stopping" };
        try {
          this.releaseAttempt(attempt);
        } catch {
          this.failStartup(attempt, startupError(error));
        } finally {
          this.state = { type: "failed", message: errorMessage(error) };
        }
      }
      throw error;
    } finally {
      this.clearAttemptTimer(attempt);
    }
  }

  stop(): Promise<void> {
    return this.stopTracking("Phone VR startup was stopped");
  }

  cancelPendingRequest(): Promise<void> {
    if (this.state.type !== "starting" && this.state.type !== "requesting" && this.state.type !== "requested") return Promise.resolve();
    return this.stopTracking("Phone VR startup was canceled");
  }

  currentFrame(timestampMs: number, viewModel: UiViewModel): BackendFrame {
    const cameraPose = this.currentViewerPose(timestampMs);
    const viewerPose = cameraPose === null ? null : this.renderer.transformTrackingPose(cameraPose);
    const panel = viewModel.panels[0] ?? null;
    const headPose = viewerPose ?? poseAtOrigin(this.latestViewerOrientation);
    if (viewerPose !== null) this.menuPlacement.open(viewerPose, 2.4);
    const menuPose = this.menuPlacement.current() ?? placeMenuPanel(headPose, 2.4);
    const panelPose = panel === null ? menuPose : resolveAnchorPose({ kind: panel.anchor, localPose: panel.localPose }, {
      world: this.referenceFromWorld,
      cockpit: this.referenceFromWorld,
      menu: menuPose,
      head: headPose
    });
    const gamepadState = viewerPose === null ? null : this.gamepadInput.readState();
    const gamepadCursor = this.gamepadSelector.update(panel, gamepadState, timestampMs);
    const gazeCursor = gamepadState !== null
      ? this.gazeDwell.update(null, null, timestampMs)
      : viewerPose === null
        ? this.gazeDwell.update(null, null, timestampMs)
        : this.updateGaze(panel, panelPose, viewerPose, timestampMs);
    return Object.freeze({
      timestampMs,
      cameraPose: cameraPose ?? poseAtOrigin(this.latestViewerOrientation),
      panelPose,
      panel,
      panelVisible: panel !== null && viewerPose !== null,
      gazeCursor: gamepadCursor ?? gazeCursor,
      viewport: this.viewport()
    });
  }

  recenterTracking(): void {
    if (this.state.type !== "active" || this.latestSensorOrientation === null) return;
    const mountedHead = this.renderer.transformTrackingPose(poseAtOrigin(this.latestViewerOrientation));
    const mountedOrigin = this.renderer.transformTrackingPose(IDENTITY_POSE);
    const newFromOld = composePose(mountedOrigin, inversePose(mountedHead));
    this.referenceFromWorld = composePose(newFromOld, this.referenceFromWorld);
    this.menuPlacement.applyReferenceTransform(newFromOld);
    this.calibrationOrientation = this.latestSensorOrientation;
    this.latestViewerOrientation = IDENTITY_POSE.orientation;
    this.gazeDwell.reset();
  }

  recenterMenu(): void {
    if (this.state.type === "active") {
      this.menuPlacement.recenter(this.renderer.transformTrackingPose(poseAtOrigin(this.latestViewerOrientation)), 2.4);
    }
  }

  private currentViewerPose(timestampMs: number): Pose | null {
    if (this.state.type !== "active" || this.latestSampleTimestampMs === null || this.latestSensorOrientation === null) return null;
    if (!Number.isFinite(timestampMs)) {
      this.failTracking("Phone VR frame timestamp is invalid");
      return null;
    }
    return poseAtOrigin(this.latestViewerOrientation);
  }

  private handleSensorReading(reading: PhoneVrSensorReading): void {
    if (this.state.type !== "starting" && this.state.type !== "active") return;
    const receivedTimestampMs = this.nowMs();
    if (!Number.isFinite(receivedTimestampMs) || receivedTimestampMs < 0 ||
        !Number.isFinite(reading.timestampMs) || reading.timestampMs < 0 || reading.timestampMs > receivedTimestampMs ||
        (this.latestSampleTimestampMs !== null && reading.timestampMs < this.latestSampleTimestampMs)) {
      this.handleInvalidReading("Phone VR orientation timestamp is invalid");
      return;
    }
    const screenAngle = this.screenOrientationAngle;
    const sensorOrientation = screenAngle === null ? null : phoneOrientationQuaternion(reading, screenAngle);
    if (sensorOrientation === null) {
      this.latestRawReading = null;
      this.latestSensorOrientation = null;
      this.handleInvalidReading("Phone VR orientation data contains null or non-finite values");
      return;
    }
    this.latestRawReading = reading;
    this.latestSensorOrientation = sensorOrientation;
    this.latestSampleTimestampMs = reading.timestampMs;
    this.updateViewerOrientation(sensorOrientation);
    if (this.state.type === "starting") {
      this.state.attempt.sampleResult = { type: "ready" };
      this.clearAttemptTimer(this.state.attempt);
      this.state.attempt.signalSample();
    }
  }

  private handleScreenOrientationChange(angle: number | null): void {
    if (this.state.type !== "starting" && this.state.type !== "active") return;
    if (angle === null || !Number.isFinite(angle)) {
      this.handleInvalidReading("Phone VR screen orientation became unavailable");
      return;
    }
    this.screenOrientationAngle = angle;
    const reading = this.latestRawReading;
    if (reading === null || this.calibrationOrientation === null) return;
    const sensorOrientation = phoneOrientationQuaternion(reading, angle);
    if (sensorOrientation === null) {
      this.handleInvalidReading("Phone VR orientation could not be recalibrated after screen rotation");
      return;
    }
    this.latestSensorOrientation = sensorOrientation;
    this.updateViewerOrientation(sensorOrientation);
  }

  private updateViewerOrientation(sensorOrientation: Quaternion): void {
    if (this.calibrationOrientation === null) {
      this.calibrationOrientation = sensorOrientation;
      this.latestViewerOrientation = IDENTITY_POSE.orientation;
      return;
    }
    const inverseCalibration = quaternion(
      this.calibrationOrientation.w,
      -this.calibrationOrientation.x,
      -this.calibrationOrientation.y,
      -this.calibrationOrientation.z
    );
    this.latestViewerOrientation = multiplyQuaternion(inverseCalibration, sensorOrientation);
  }

  private ownsAttempt(attempt: PhoneVrTrackingAttempt): boolean {
    return (this.state.type === "starting" || this.state.type === "active") && this.state.attempt === attempt;
  }

  private isStartingAttempt(attempt: PhoneVrTrackingAttempt): boolean {
    return this.state.type === "starting" && this.state.attempt === attempt;
  }

  private acceptsAttempt(attempt: PhoneVrTrackingAttempt): boolean {
    return this.ownsAttempt(attempt) && attempt.sampleResult.type !== "failed";
  }

  private failStartup(attempt: PhoneVrTrackingAttempt, error: Error): void {
    if (attempt.sampleResult.type === "failed") return;
    attempt.sampleResult = { type: "failed", error };
    attempt.signalSample();
  }

  private handleInvalidReading(message: string): void {
    if (this.state.type === "starting") {
      this.failStartup(this.state.attempt, new Error(message));
      return;
    }
    if (this.state.type === "active") this.failTracking(message);
  }

  private failTracking(message: string): void {
    if (this.state.type !== "active") return;
    const attempt = this.state.attempt;
    this.state = { type: "stopping" };
    try {
      this.releaseAttempt(attempt);
    } catch {
      this.failStartup(attempt, new Error(message));
    } finally {
      this.state = { type: "failed", message };
    }
    this.onTrackingUnavailable(message);
  }

  private updateGaze(
    panel: UiViewModel["panels"][number] | null,
    panelPose: Pose,
    viewerPose: Pose,
    timestampMs: number
  ) {
    if (panel === null) return this.gazeDwell.update(null, null, timestampMs);
    const point = intersectPanel({
      origin: viewerPose.position,
      direction: rotateVec3(viewerPose.orientation, vec3(0, 0, -1))
    }, panelPose);
    return this.gazeDwell.update(panel, point, timestampMs);
  }

  private stopTracking(message: string): Promise<void> {
    if (this.state.type === "stopping") return Promise.resolve();
    const previousState = this.state;
    if (previousState.type === "starting") this.failStartup(previousState.attempt, new Error(message));
    this.requestGeneration++;
    this.state = { type: "stopping" };
    try {
      if (previousState.type === "starting" || previousState.type === "active") this.releaseAttempt(previousState.attempt);
      return Promise.resolve();
    } catch (error) {
      return Promise.reject(startupError(error));
    } finally {
      this.state = { type: "idle" };
    }
  }

  private releaseAttempt(attempt: PhoneVrTrackingAttempt): void {
    if (attempt.resources === "released") return;
    attempt.resources = "released";
    this.clearAttemptTimer(attempt);
    const gazeDwell = this.gazeDwell;
    const gamepadSelector = this.gamepadSelector;
    this.resetTracking();
    let result: { readonly type: "released" } | { readonly type: "failed"; readonly error: Error } = { type: "released" };
    for (const cleanup of [
      () => { this.renderer.setSelectRayHandler(null); },
      () => { this.sensors.stopListening(); },
      () => { this.renderer.setStereoPresentation(null); },
      () => { gazeDwell.reset(); },
      () => { gamepadSelector.reset(); }
    ]) {
      try {
        cleanup();
      } catch (error) {
        if (result.type === "released") result = { type: "failed", error: startupError(error) };
      }
    }
    if (result.type === "failed") throw result.error;
  }

  private resetTracking(): void {
    this.latestRawReading = null;
    this.latestSensorOrientation = null;
    this.latestSampleTimestampMs = null;
    this.latestViewerOrientation = IDENTITY_POSE.orientation;
    this.calibrationOrientation = null;
    this.referenceFromWorld = IDENTITY_POSE;
    this.gazeDwell = new GazeDwellSelector(this.dispatch);
    this.gamepadSelector = new GamepadUiSelector(this.dispatch);
  }

  private clearAttemptTimer(attempt: PhoneVrTrackingAttempt): void {
    if (attempt.timer.type === "pending") clearTimeout(attempt.timer.handle);
    attempt.timer = { type: "cleared" };
  }
}

function startupError(error: unknown): Error {
  try {
    return error instanceof Error ? error : new Error(String(error), { cause: error });
  } catch {
    return new Error("Phone VR operation failed with an unprintable cause", { cause: error });
  }
}

function poseAtOrigin(orientation: Quaternion): Pose {
  return Object.freeze({ position: vec3(0, 0, 0), orientation });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
