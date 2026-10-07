import { IDENTITY_POSE } from "../render/contracts/math.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import type { BackendFrame, PresentationBackendAdapter, ViewportSize } from "../render/contracts/runtime.js";

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
      headHud: NO_HEAD_HUD,
      cameraPose: IDENTITY_POSE,
      panel: Object.freeze({ kind: "absent" }),
      viewport: this.viewport()
    });
  }
}
