import type { Pose, Vec2, Vec3 } from "./math.js";
import type { CinematicCameraView } from "./camera.js";
import type { MenuScrollContext, UiPanel, UiViewModel } from "./ui.js";
import type { MenuDocumentResult, MenuViewport } from "./menu-layout.js";
import type { LakeVisualCondition, LakeWaterQuality } from "./lake-water.js";
import type { LakeSkyCondition } from "./lake-sky.js";
import type { HeadHudFrame } from "./head-hud.js";
import type { ViewerFrame, ViewerGeometryUnavailableReason } from "./viewer-frame.js";
import type { TailPhysicalFlightControls, TailPresentationGeometryAvailability } from "./flight-controls.js";

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
  readonly headHud: HeadHudFrame;
  readonly timestampMs: number;
  readonly cameraPose: Pose;
  readonly panel: PanelFrame;
  readonly viewport: ViewportSize;
}

export type PanelUnavailableReason = ViewerGeometryUnavailableReason | "insufficient-view-area" | "insufficient-ink-angle" | "context-unavailable" |
  Extract<MenuDocumentResult, { kind: "unavailable" }>["reason"] | "measurement-unavailable" | "drawing-unavailable";

export type MenuPresentation =
  | { readonly kind: "absent" }
  | { readonly kind: "pending" }
  | { readonly kind: "unavailable"; readonly reason: PanelUnavailableReason }
  | { readonly kind: "ready"; readonly panel: UiPanel; readonly viewport: MenuViewport; readonly context: MenuScrollContext };

export interface PreparedPresentationView {
  readonly viewModel: UiViewModel;
  readonly menu: MenuPresentation;
}

export type PanelFrame =
  | { readonly kind: "absent" }
  | { readonly kind: "visible"; readonly panel: UiPanel; readonly pose: Pose; readonly cursor: PanelCursor | null }
  | { readonly kind: "unavailable"; readonly reason: PanelUnavailableReason };

interface FlightRenderPoseBase {
  readonly datumPositionNed: Readonly<{ north: number; east: number; down: number }>;
  readonly attitudeBodyToNed: Readonly<{ w: number; x: number; y: number; z: number }>;
  readonly pilotPositionMeters: number;
  readonly initialPilotPositionMeters: number;
  /** Simulation clock, used by render-only environmental animation. */
  readonly simulationTimeSeconds?: number;
  /** Render-only wing flex proxy; does not affect the simulated aerodynamic model. */
  readonly airspeedMetersPerSecond?: number | null;
  /** Instantaneous scenario wind in NED, kept separate from the stable visual wave state. */
  readonly windVelocityNedMetersPerSecond?: Readonly<{ north: number; east: number }> | null;
}

export type FlightRenderPose = FlightRenderPoseBase & (
  | Readonly<{ controls?: never; tailGeometry?: never }>
  | Readonly<{ controls: TailPhysicalFlightControls; tailGeometry: TailPresentationGeometryAvailability }>
);

export type FlightCameraMode = "pilot" | "chase" | "orbit" | "platform" | "shore" | "overhead" | "side" | "front" | "telephoto";

export interface PanelCursor {
  readonly point: Vec2;
  readonly progress: number;
}

export interface SelectRay {
  readonly origin: Vec3;
  readonly direction: Vec3;
  readonly timestampMs: number;
}

export type LakeWaterQualityCleanupResult =
  | { readonly kind: "complete" }
  | { readonly kind: "failed"; readonly message: string };

export interface RendererAdapter {
  startLoop(callback: (timestampMs: number, viewer: ViewerFrame) => void): void;
  beginViewFrame(): void;
  stopLoop(): void;
  render(frame: BackendFrame): void;
  setFlightPose(pose: FlightRenderPose | null): void;
  setPreparedFlightPose(pose: FlightRenderPose | null): void;
  setLakeVisualCondition(condition: LakeVisualCondition): void;
  setLakeWaterQuality(quality: LakeWaterQuality): Promise<LakeWaterQualityCleanupResult>;
  setLakeSkyCondition(condition: LakeSkyCondition | null): void;
  setLakeVenueVisible(visible: boolean): void;
  setFlightCameraMode(mode: FlightCameraMode): void;
  setCinematicCameraView(view: CinematicCameraView | null): void;
  transformTrackingPose(pose: Pose): Pose;
  resize(viewport: ViewportSize): void;
  setStereoPresentation(profile: StereoPresentationProfile | null): void;
  setSelectRayHandler(handler: ((ray: SelectRay) => void) | null): void;
  dispose(): void;
}

export interface PresentationBackendAdapter {
  readonly mode: PresentationMode;
  start(): Promise<void>;
  stop(): Promise<void>;
  currentFrame(timestampMs: number, viewModel: UiViewModel, viewer: ViewerFrame, menu: MenuPresentation): BackendFrame;
}

export type RenderError =
  | { readonly type: "unsupported"; readonly mode: PresentationMode }
  | { readonly type: "backend-failed"; readonly mode: PresentationMode; readonly message: string }
  | { readonly type: "renderer-failed"; readonly message: string }
  | { readonly type: "disposed" };

export type RuntimeResult =
  | { readonly ok: true }
  | { readonly ok: false; readonly error: RenderError };
