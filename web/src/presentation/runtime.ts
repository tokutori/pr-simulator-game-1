import type { RendererAdapter, PresentationBackendAdapter, PresentationMode, PresentationCleanupFailure, RuntimeResult, ViewportSize, PanelUnavailableReason } from "../render/contracts/runtime.js";
import type { PreparedPresentationView } from "../render/contracts/runtime.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import { FrameRateCounter } from "./frame-rate.js";

export class PresentationRuntime {
  private readonly backends: ReadonlyMap<PresentationMode, PresentationBackendAdapter>;
  private activeBackend: PresentationBackendAdapter | null = null;
  private loopStarted = false;
  private disposed = false;
  private disposal: Promise<RuntimeResult> | null = null;
  private queue: Promise<unknown> = Promise.resolve();
  private readonly frameRate = new FrameRateCounter();

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

  get framesPerSecond(): number | null {
    return this.frameRate.framesPerSecond;
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
    this.disposal ??= this.enqueue(() => this.disposeNow());
    return this.disposal;
  }

  private async disposeNow(): Promise<RuntimeResult> {
    this.disposed = true;
    const backend = this.activeBackend;
    this.activeBackend = null;
    const failures: PresentationCleanupFailure[] = [];
    const loopFailure = this.stopLoop();
    if (loopFailure !== null) failures.push(loopFailure);
    if (backend !== null) {
      try {
        await backend.stop();
      } catch (cause: unknown) {
        failures.push({ type: "backend-failed", operation: "stop-backend", mode: backend.mode, message: errorMessage(cause), cause });
      }
    }
    try {
      this.renderer.dispose();
    } catch (cause: unknown) {
      failures.push({ type: "renderer-failed", operation: "dispose-renderer", message: errorMessage(cause), cause });
    }
    return cleanupResult(failures);
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
    this.frameRate.reset();
    if (!this.loopStarted) {
      try {
        this.renderer.startLoop((timestampMs, viewer) => { this.renderFrame(timestampMs, viewer); });
        this.loopStarted = true;
      } catch (error) {
        this.activeBackend = null;
        try {
          await backend.stop();
        } catch (stopError) {
          return cleanupResult([
            { type: "renderer-failed", operation: "start-loop", message: errorMessage(error), cause: error },
            { type: "backend-failed", operation: "stop-backend", mode: backend.mode, message: errorMessage(stopError), cause: stopError }
          ]);
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
      const failures: PresentationCleanupFailure[] = [
        { type: "backend-failed", operation: "stop-backend", mode: previous.mode, message: errorMessage(error), cause: error }
      ];
      const loopFailure = this.stopLoop();
      if (loopFailure !== null) failures.push(loopFailure);
      return cleanupResult(failures);
    }
    this.activeBackend = null;
    try {
      await next.start();
      this.activeBackend = next;
      this.frameRate.reset();
      return { ok: true };
    } catch (error) {
      const startMessage = errorMessage(error);
      try {
        await next.stop();
      } catch (cleanupError) {
        const failures: PresentationCleanupFailure[] = [
          { type: "backend-failed", operation: "start-backend", mode, message: startMessage, cause: error },
          { type: "backend-failed", operation: "stop-backend", mode, message: errorMessage(cleanupError), cause: cleanupError }
        ];
        const loopFailure = this.stopLoop();
        if (loopFailure !== null) failures.push(loopFailure);
        return cleanupResult(failures);
      }
      const screen = this.backends.get("screen");
      if (screen !== undefined && screen !== next) {
        try {
          await screen.start();
          this.activeBackend = screen;
          this.frameRate.reset();
          return { ok: false, error: { type: "backend-failed", mode, message: startMessage } };
        } catch (fallbackError) {
          this.activeBackend = null;
          const failures: PresentationCleanupFailure[] = [
            { type: "backend-failed", operation: "start-backend", mode, message: startMessage, cause: error },
            { type: "backend-failed", operation: "start-backend", mode: "screen", message: errorMessage(fallbackError), cause: fallbackError }
          ];
          const loopFailure = this.stopLoop();
          if (loopFailure !== null) failures.push(loopFailure);
          return cleanupResult(failures);
        }
      }
      const loopFailure = this.stopLoop();
      if (loopFailure !== null) return cleanupResult([
        { type: "backend-failed", operation: "start-backend", mode, message: startMessage, cause: error }, loopFailure
      ]);
      return { ok: false, error: { type: "backend-failed", mode, message: startMessage } };
    }
  }

  private stopLoop(): PresentationCleanupFailure | null {
    this.frameRate.reset();
    if (!this.loopStarted) return null;
    this.loopStarted = false;
    try {
      this.renderer.stopLoop();
      return null;
    } catch (cause: unknown) {
      return { type: "renderer-failed", operation: "stop-loop", message: errorMessage(cause), cause };
    }
  }

  private renderFrame(timestampMs: number, viewer: ViewerFrame): void {
    const backend = this.activeBackend;
    if (backend === null || this.disposed) return;
    this.onFrame(timestampMs);
    this.renderer.beginViewFrame();
    const prepared = this.viewModel(viewer);
    const frame = backend.currentFrame(timestampMs, prepared.viewModel, viewer, prepared.menu);
    this.renderer.render(frame);
    this.frameRate.observe(timestampMs);
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
  try {
    return error instanceof Error ? error.message : String(error);
  } catch {
    return "Presentation operation failed with an unprintable cause";
  }
}

function cleanupResult(failures: readonly PresentationCleanupFailure[]): RuntimeResult {
  const first = failures[0];
  return first === undefined ? { ok: true } : { ok: false, error: { type: "cleanup-failed", failures: [first, ...failures.slice(1)] } };
}
