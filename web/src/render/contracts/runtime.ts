import type { Pose, Vec2, Vec3 } from "./math.js";
import type { UiPanel, UiViewModel } from "./ui.js";

export type PresentationMode = "screen" | "webxr" | "phone-vr";

export interface ViewportSize extends Vec2 {
  readonly pixelRatio: number;
}

export interface StereoPresentationProfile {
  readonly eyeSeparationMeters: number;
  readonly verticalFieldOfViewDegrees: number;
  readonly focusDistanceMeters: number;
  readonly distortion: "disabled";
}

export interface BackendFrame {
  readonly timestampMs: number;
  readonly cameraPose: Pose;
  readonly panelPose: Pose;
  readonly panel: UiPanel | null;
  readonly panelVisible: boolean;
  readonly gazeCursor: PanelCursor | null;
  readonly viewport: ViewportSize;
}

export interface FlightRenderPose {
  readonly datumPositionNed: Readonly<{ north: number; east: number; down: number }>;
  readonly attitudeBodyToNed: Readonly<{ w: number; x: number; y: number; z: number }>;
  readonly pilotPositionMeters: number;
  readonly initialPilotPositionMeters: number;
}

export type FlightCameraMode = "pilot" | "chase";

export interface PanelCursor {
  readonly point: Vec2;
  readonly progress: number;
}

export interface SelectRay {
  readonly origin: Vec3;
  readonly direction: Vec3;
  readonly timestampMs: number;
}

export interface RendererAdapter {
  startLoop(callback: (timestampMs: number, viewerPose: Pose | null) => void): void;
  stopLoop(): void;
  render(frame: BackendFrame): void;
  setFlightPose(pose: FlightRenderPose | null): void;
  setFlightCameraMode(mode: FlightCameraMode): void;
  resize(viewport: ViewportSize): void;
  setStereoPresentation(profile: StereoPresentationProfile | null): void;
  setSelectRayHandler(handler: ((ray: SelectRay) => void) | null): void;
  dispose(): void;
}

export interface PresentationBackendAdapter {
  readonly mode: PresentationMode;
  start(): Promise<void>;
  stop(): Promise<void>;
  currentFrame(timestampMs: number, viewModel: UiViewModel, viewerPose: Pose | null): BackendFrame;
}

export type RenderError =
  | { readonly type: "unsupported"; readonly mode: PresentationMode }
  | { readonly type: "backend-failed"; readonly mode: PresentationMode; readonly message: string }
  | { readonly type: "renderer-failed"; readonly message: string }
  | { readonly type: "disposed" };

export type RuntimeResult =
  | { readonly ok: true }
  | { readonly ok: false; readonly error: RenderError };
