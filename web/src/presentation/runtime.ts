import type { RendererAdapter, PresentationBackendAdapter, PresentationMode, RenderError, RuntimeResult, ViewportSize, PanelUnavailableReason } from "../render/contracts/runtime.js";
import type { PreparedPresentationView } from "../render/contracts/runtime.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";

export class PresentationRuntime {
  private readonly backends: ReadonlyMap<PresentationMode, PresentationBackendAdapter>;
  private activeBackend: PresentationBackendAdapter | null = null;
  private loopStarted = false;
  private disposed = false;
  private queue: Promise<unknown> = Promise.resolve();

  constructor(
    private readonly renderer: RendererAdapter,
    backends: readonly PresentationBackendAdapter[],
    private readonly viewModel: (viewer: ViewerFrame) => PreparedPresentationView,
    private readonly onFrame: (timestampMs: number) => void = () => undefined,
    private readonly onMenuUnavailable: (mode: "webxr" | "phone-vr", reason: PanelUnavailableReason) => void = () => undefined
  ) {
    const entries = backends.map((backend) => [backend.mode, backend] as const);
    if (new Set(entries.map(([mode]) => mode)).size !== entries.length) throw new Error("Presentation backend modes must be unique");
    this.backends = new Map(entries);
  }

  get currentMode(): PresentationMode | null {
    return this.activeBackend?.mode ?? null;
  }

  start(mode: PresentationMode): Promise<RuntimeResult> {
    return this.enqueue(() => this.startNow(mode));
  }

  switchTo(mode: PresentationMode): Promise<RuntimeResult> {
    return this.enqueue(() => this.switchNow(mode));
  }

  resize(viewport: ViewportSize): RuntimeResult {
    if (this.disposed) return { ok: false, error: { type: "disposed" } };
    try {
      validateViewport(viewport);
      this.renderer.resize(viewport);
      return { ok: true };
    } catch (error) {
      return { ok: false, error: { type: "renderer-failed", message: errorMessage(error) } };
    }
  }

  dispose(): Promise<RuntimeResult> {
    return this.enqueue(async () => {
      if (this.disposed) return { ok: true };
      this.disposed = true;
      if (this.loopStarted) this.renderer.stopLoop();
      this.loopStarted = false;
      let stopError: RenderError | null = null;
      if (this.activeBackend !== null) {
        const backend = this.activeBackend;
        this.activeBackend = null;
        try {
          await backend.stop();
        } catch (error) {
          stopError = { type: "backend-failed", mode: backend.mode, message: errorMessage(error) };
        }
      }
      this.renderer.dispose();
      return stopError === null ? { ok: true } : { ok: false, error: stopError };
    });
  }

  private async startNow(mode: PresentationMode): Promise<RuntimeResult> {
    if (this.disposed) return { ok: false, error: { type: "disposed" } };
    if (this.activeBackend !== null) {
      return this.activeBackend.mode === mode ? { ok: true } : this.switchNow(mode);
    }
    const backend = this.backends.get(mode);
    if (backend === undefined) return { ok: false, error: { type: "unsupported", mode } };
    try {
      await backend.start();
    } catch (error) {
      return { ok: false, error: { type: "backend-failed", mode, message: errorMessage(error) } };
    }
    this.activeBackend = backend;
    if (!this.loopStarted) {
      try {
        this.renderer.startLoop((timestampMs, viewer) => { this.renderFrame(timestampMs, viewer); });
        this.loopStarted = true;
      } catch (error) {
        this.activeBackend = null;
        try {
          await backend.stop();
        } catch (stopError) {
          return {
            ok: false,
            error: {
              type: "backend-failed",
              mode: backend.mode,
              message: `Renderer start failed: ${errorMessage(error)}; backend cleanup failed: ${errorMessage(stopError)}`
            }
          };
        }
        return { ok: false, error: { type: "renderer-failed", message: errorMessage(error) } };
      }
    }
    return { ok: true };
  }

  private async switchNow(mode: PresentationMode): Promise<RuntimeResult> {
    if (this.disposed) return { ok: false, error: { type: "disposed" } };
    if (this.activeBackend === null) return this.startNow(mode);
    if (this.activeBackend.mode === mode) return { ok: true };
    const next = this.backends.get(mode);
    if (next === undefined) return { ok: false, error: { type: "unsupported", mode } };
    const previous = this.activeBackend;
    try {
      await previous.stop();
    } catch (error) {
      this.activeBackend = null;
      this.stopLoop();
      return { ok: false, error: { type: "backend-failed", mode: previous.mode, message: errorMessage(error) } };
    }
    this.activeBackend = null;
    try {
      await next.start();
      this.activeBackend = next;
      return { ok: true };
    } catch (error) {
      const startMessage = errorMessage(error);
      try {
        await next.stop();
      } catch (cleanupError) {
        this.stopLoop();
        return {
          ok: false,
          error: {
            type: "backend-failed",
            mode,
            message: `Backend start failed: ${startMessage}; cleanup failed: ${errorMessage(cleanupError)}`
          }
        };
      }
      const screen = this.backends.get("screen");
      if (screen !== undefined && screen !== next) {
        try {
          await screen.start();
          this.activeBackend = screen;
          return { ok: false, error: { type: "backend-failed", mode, message: startMessage } };
        } catch (fallbackError) {
          this.activeBackend = null;
          this.stopLoop();
          return {
            ok: false,
            error: {
              type: "backend-failed",
              mode,
              message: `Backend start failed: ${startMessage}; Screen recovery failed: ${errorMessage(fallbackError)}`
            }
          };
        }
      }
      this.stopLoop();
      return { ok: false, error: { type: "backend-failed", mode, message: startMessage } };
    }
  }

  private stopLoop(): void {
    if (!this.loopStarted) return;
    this.renderer.stopLoop();
    this.loopStarted = false;
  }

  private renderFrame(timestampMs: number, viewer: ViewerFrame): void {
    const backend = this.activeBackend;
    if (backend === null || this.disposed) return;
    this.onFrame(timestampMs);
    this.renderer.beginViewFrame();
    const prepared = this.viewModel(viewer);
    const frame = backend.currentFrame(timestampMs, prepared.viewModel, viewer, prepared.menu);
    this.renderer.render(frame);
    if (frame.panel.kind === "unavailable" && frame.panel.reason !== "viewer-unavailable" && backend.mode !== "screen") {
      this.onMenuUnavailable(backend.mode, frame.panel.reason);
    }
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.queue.then(operation, operation);
    this.queue = next;
    return next;
  }
}

function validateViewport(viewport: ViewportSize): void {
  if (!Number.isFinite(viewport.x) || !Number.isFinite(viewport.y) || !Number.isFinite(viewport.pixelRatio) ||
      viewport.x <= 0 || viewport.y <= 0 || viewport.pixelRatio <= 0) {
    throw new RangeError("Viewport dimensions and pixel ratio must be positive and finite");
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
