import type { ScreenRenderAutoBinding, ScreenRenderInvalidation } from "../../contracts/screen-render-auto.js";
import { parseScreenRenderAutoSnapshot } from "../../contracts/screen-render-auto.js";
import { GpuWorkTimer } from "./gpu-work-timer.js";
import type { GpuQueryObservation } from "./gpu-work-timer.js";

export class ScreenAutoRenderer {
  private frame = 0;
  private lastPublished = "";
  private lastFrame: number | null = null;
  private eligible = false;
  private prepared = false;
  private readonly timer: GpuWorkTimer;

  constructor(gl: WebGLRenderingContext | WebGL2RenderingContext, private readonly binding: ScreenRenderAutoBinding,
    private readonly applyScale: (factor: number) => boolean, private readonly now: () => number = () => performance.now()) {
    if (!("beginQuery" in gl)) throw new Error("The Screen renderer requires WebGL2");
    this.timer = new GpuWorkTimer(gl);
  }

  selectManual(factor: number): void {
    this.timer.clear();
    this.binding.policy.select_manual(factor * 100, this.now());
    this.publish();
  }

  selectAuto(enabled: boolean): void {
    if (!enabled) this.timer.clear();
    this.binding.policy.select_auto(enabled, this.now());
    this.publish();
  }

  boundary(eligible: boolean): void {
    const now = this.now();
    if (eligible !== this.eligible) this.invalidateAt("backend_changed", now);
    else if (this.lastFrame !== null && now - this.lastFrame > 500) this.invalidateAt("interrupted", now);
    this.eligible = eligible;
    this.lastFrame = now;
    if (eligible && this.binding.policy.measurement_enabled()) {
      try {
        for (const observation of this.timer.poll()) this.receive(observation, now);
        this.binding.policy.pending(now);
      } catch (error: unknown) { this.unavailable(error, now); }
    }
    const percent = this.binding.policy.requested_scale_percent();
    if (percent !== 0 && eligible) {
      const epoch = this.binding.policy.requested_epoch();
      try {
        this.timer.clear();
        if (this.applyScale(percent / 100)) {
          this.prepared = false;
          this.binding.policy.applied(epoch, percent, now);
        }
      } catch (error: unknown) { this.binding.policy.failed(epoch, percent, message(error), now); }
    }
    this.publish();
  }

  begin(): void {
    if (!this.eligible || !this.prepared || !this.binding.policy.measurement_enabled()) return;
    const now = this.now();
    if (this.frame === 0xffff_ffff) {
      this.invalidateAt("interrupted", now);
      this.frame = 0;
    }
    this.frame++;
    try {
      const observation = this.timer.begin({ epoch: this.binding.policy.epoch(), frame: this.frame, source: now });
      if (observation !== null) this.receive(observation, now);
    } catch (error: unknown) { this.unavailable(error, now); }
    this.publish();
  }

  end(complete: boolean): void {
    try { this.timer.end(complete); }
    catch (error: unknown) { this.unavailable(error, this.now()); }
    if (!complete) this.invalidate("interrupted");
    else if (this.eligible && !this.prepared) {
      this.invalidateAt("quality_changed", this.now());
      this.prepared = true;
    }
  }

  invalidate(reason: ScreenRenderInvalidation): void {
    this.invalidateAt(reason, this.now());
  }

  private invalidateAt(reason: ScreenRenderInvalidation, now: number): void {
    this.prepared = false;
    try { this.timer.clear(); }
    finally {
      this.binding.policy.invalidate(reason, now);
      this.publish();
    }
  }

  restored(): void {
    this.timer.restored();
    this.invalidate("context_restored");
  }

  dispose(): void {
    try { this.timer.clear(); }
    finally { this.binding.policy.free(); }
  }

  private receive(observation: GpuQueryObservation, now: number): void {
    if (observation.kind === "disjoint") this.invalidateAt("disjoint", now);
    else if (observation.kind === "unsupported") this.binding.policy.unsupported(observation.cause, now);
    else if (observation.source.epoch === this.binding.policy.epoch()) {
      this.binding.policy.observe(observation.source.epoch, observation.source.frame,
        observation.source.source, observation.milliseconds, now);
    }
  }

  private unavailable(error: unknown, now: number): void {
    try { this.timer.clear(); }
    finally { this.binding.policy.unsupported(message(error), now); }
    this.publish();
  }

  private publish(): void {
    const json = this.binding.policy.snapshot_json();
    if (json === this.lastPublished) return;
    const snapshot = parseScreenRenderAutoSnapshot(json);
    this.lastPublished = json;
    this.binding.onSnapshot(snapshot);
  }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
