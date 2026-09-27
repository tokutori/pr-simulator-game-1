import { IDENTITY_POSE } from "../render/contracts/math.js";
import type { BackendFrame, PresentationBackendAdapter, ViewportSize } from "../render/contracts/runtime.js";
import { placeMenuPanel } from "../render/anchors.js";

export class ScreenPresentationBackend implements PresentationBackendAdapter {
  readonly mode = "screen" as const;
  private active = false;

  constructor(private readonly viewport: () => ViewportSize) {}

  start(): Promise<void> {
    if (this.active) throw new Error("Screen backend is already active");
    this.active = true;
    return Promise.resolve();
  }

  stop(): Promise<void> {
    this.active = false;
    return Promise.resolve();
  }

  currentFrame(timestampMs: number): BackendFrame {
    if (!this.active) throw new Error("Screen backend is inactive");
    return Object.freeze({
      timestampMs,
      cameraPose: IDENTITY_POSE,
      menuPose: placeMenuPanel(IDENTITY_POSE, 2.4),
      panelVisible: false,
      panelRevision: 0,
      viewport: this.viewport()
    });
  }
}
