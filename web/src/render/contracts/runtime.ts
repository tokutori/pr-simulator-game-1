import type { Pose, Vec2 } from "./math.js";
import type { UiViewModel } from "./ui.js";

export type PresentationMode = "screen" | "webxr" | "phone-vr";

export interface ViewportSize extends Vec2 {
  readonly pixelRatio: number;
}

export interface BackendFrame {
  readonly timestampMs: number;
  readonly cameraPose: Pose;
  readonly menuPose: Pose;
  readonly panelVisible: boolean;
  readonly panelRevision: number;
  readonly viewport: ViewportSize;
}

export interface RendererAdapter {
  startLoop(callback: (timestampMs: number) => void): void;
  stopLoop(): void;
  render(frame: BackendFrame): void;
  resize(viewport: ViewportSize): void;
  dispose(): void;
}

export interface PresentationBackendAdapter {
  readonly mode: PresentationMode;
  start(): Promise<void>;
  stop(): Promise<void>;
  currentFrame(timestampMs: number, viewModel: UiViewModel): BackendFrame;
}

export type RenderError =
  | { readonly type: "unsupported"; readonly mode: PresentationMode }
  | { readonly type: "backend-failed"; readonly mode: PresentationMode; readonly message: string }
  | { readonly type: "renderer-failed"; readonly message: string }
  | { readonly type: "disposed" };

export type RuntimeResult =
  | { readonly ok: true }
  | { readonly ok: false; readonly error: RenderError };
