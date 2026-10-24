import type { ApplicationGpuQuery } from "../../../generated/application-contracts.js";

interface TimerExtension {
  readonly TIME_ELAPSED_EXT: number;
  readonly GPU_DISJOINT_EXT: number;
  readonly QUERY_COUNTER_BITS_EXT: number;
}

export type GpuQueryObservation =
  | { readonly kind: "sample"; readonly source: ApplicationGpuQuery; readonly milliseconds: number | null }
  | { readonly kind: "disjoint" }
  | { readonly kind: "unsupported"; readonly cause: string };

type TimerCapability = { readonly kind: "supported"; readonly extension: TimerExtension }
  | { readonly kind: "unsupported"; readonly cause: string };

export class GpuWorkTimer {
  private capability: { readonly kind: "unchecked" } | TimerCapability = { kind: "unchecked" };
  private pending: { readonly query: WebGLQuery; readonly source: ApplicationGpuQuery }[] = [];
  private active: { readonly query: WebGLQuery; readonly source: ApplicationGpuQuery } | null = null;

  constructor(private readonly gl: WebGL2RenderingContext) {}

  poll(): readonly GpuQueryObservation[] {
    if (this.capability.kind !== "supported" || this.pending.length === 0) return [];
    const disjoint: unknown = this.gl.getParameter(this.capability.extension.GPU_DISJOINT_EXT);
    if (typeof disjoint !== "boolean") throw new Error("GPU timer returned an invalid disjoint state");
    if (disjoint) {
      this.clear();
      return [{ kind: "disjoint" }];
    }
    const observations: GpuQueryObservation[] = [];
    for (const pending of [...this.pending]) {
      const available: unknown = this.gl.getQueryParameter(pending.query, this.gl.QUERY_RESULT_AVAILABLE);
      if (typeof available !== "boolean") throw new Error("GPU timer returned an invalid availability state");
      if (!available) continue;
      const nanoseconds: unknown = this.gl.getQueryParameter(pending.query, this.gl.QUERY_RESULT);
      this.gl.deleteQuery(pending.query);
      this.pending = this.pending.filter((item) => item !== pending);
      if (typeof nanoseconds !== "number") {
        throw new Error("GPU timer returned an invalid elapsed duration");
      }
      observations.push({ kind: "sample", source: pending.source, milliseconds: Number.isFinite(nanoseconds) ? nanoseconds / 1_000_000 : null });
    }
    return observations;
  }

  availability(): Extract<GpuQueryObservation, { kind: "unsupported" }> | null {
    const capability = this.resolveCapability();
    return capability.kind === "unsupported" ? capability : null;
  }

  private resolveCapability(): TimerCapability {
    if (this.capability.kind === "unchecked") {
      const extension = this.gl.getExtension("EXT_disjoint_timer_query_webgl2") as TimerExtension | null;
      if (extension === null) this.capability = { kind: "unsupported", cause: "EXT_disjoint_timer_query_webgl2" };
      else {
        const bits: unknown = this.gl.getQuery(extension.TIME_ELAPSED_EXT, extension.QUERY_COUNTER_BITS_EXT);
        this.capability = typeof bits === "number" && Number.isInteger(bits) && bits > 0
          ? { kind: "supported", extension }
          : { kind: "unsupported", cause: "GPU elapsed-query counter is unavailable" };
      }
    }
    return this.capability;
  }

  begin(source: ApplicationGpuQuery): GpuQueryObservation | null {
    const capability = this.resolveCapability();
    if (capability.kind === "unsupported") return capability;
    const extension = capability.extension;
    if (this.pending.length >= 4 || this.active !== null) return null;
    if (this.gl.getQuery(extension.TIME_ELAPSED_EXT, this.gl.CURRENT_QUERY) !== null) return null;
    const query = this.gl.createQuery();
    try {
      this.gl.beginQuery(extension.TIME_ELAPSED_EXT, query);
      this.active = { query, source };
    } catch (error: unknown) {
      this.gl.deleteQuery(query);
      throw error;
    }
    return null;
  }

  end(complete: boolean): void {
    const active = this.active;
    if (active === null || this.capability.kind !== "supported") return;
    const extension = this.capability.extension;
    this.active = null;
    try {
      if (this.gl.getQuery(extension.TIME_ELAPSED_EXT, this.gl.CURRENT_QUERY) !== active.query) {
        throw new Error("GPU timer query ownership changed during rendering");
      }
      this.gl.endQuery(extension.TIME_ELAPSED_EXT);
      if (complete) this.pending.push(active);
      else this.gl.deleteQuery(active.query);
    } catch (error: unknown) {
      this.gl.deleteQuery(active.query);
      throw error;
    }
  }

  clear(): void {
    const failures: unknown[] = [];
    try { this.end(false); } catch (error: unknown) { failures.push(error); }
    const pending = this.pending;
    this.pending = [];
    for (const { query } of pending) {
      try { this.gl.deleteQuery(query); } catch (error: unknown) { failures.push(error); }
    }
    if (failures.length > 0) throw new AggregateError(failures, "GPU timer cleanup failed");
  }

  restored(): void {
    this.clear();
    this.capability = { kind: "unchecked" };
  }
}
