import type { ScreenRenderAutoPolicy } from "../../web/src/render/contracts/screen-render-auto.js";
type ObservedPolicy = ScreenRenderAutoPolicy & { snapshot_json(): string };

export interface EpochObservation {
  at: number;
  epoch: number;
  snapshotJson: string;
}

export interface ForwardedGpuObservation {
  epoch: number;
  frame: number;
  sourceMs: number;
  gpuWorkMs: number;
  arrivalMs: number;
  epochBefore: number;
}

export interface RafObservation {
  at: number;
  timestamp: number;
  phase: string | null;
  tick: number | null;
  width: number;
  height: number;
}

export interface LiveObservation {
  startMs: number;
  endMs: number;
  observedEndMs: number;
  interrupted: string | null;
  initialSnapshotJson: string;
  finalSnapshotJson: string;
  epochs: EpochObservation[];
  forwardedGpu: ForwardedGpuObservation[];
  frames: RafObservation[];
  phases: { at: number; phase: string; tick: number | null }[];
  truncated: boolean;
  observerErrors: string[];
  observerMilliseconds: number;
  observerCalls: number;
}

interface BrowserObserver {
  phase: string | null;
  tick: number | null;
  snapshotJson: string | null;
  policyJson: string | null;
  contextLosses: number;
  attach(prototype: object): void;
  measure(): Promise<LiveObservation>;
}

declare global {
  interface Window {
    __liveAuto: BrowserObserver;
  }
}

(() => {
  const limit = 4096;
  const parse = JSON.parse;
  let capture: { kind: "idle" } | { kind: "collecting" | "draining"; observation: LiveObservation } = { kind: "idle" };
  let lastEpoch: number | null = null;
  const epochs: EpochObservation[] = [];
  const errors: string[] = [];
  let observerMilliseconds = 0;
  let observerCalls = 0;
  let interrupt: ((cause: string) => void) | null = null;
  const object = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null;
  const append = <Value>(target: Value[], value: Value): void => {
    if (target.length < limit) target.push(value);
    else if (capture.kind !== "idle") capture.observation.truncated = true;
  };
  const failure = (cause: unknown): void => {
    if (errors.length < 16) errors.push(cause instanceof Error ? cause.message : String(cause));
  };
  const recordEpoch = (policy: ObservedPolicy, at: number): void => {
    const epoch = policy.epoch();
    if (epoch === lastEpoch) return;
    lastEpoch = epoch;
    const snapshotJson = policy.snapshot_json();
    observer.policyJson = snapshotJson;
    const event = { at, epoch, snapshotJson };
    append(epochs, event);
    if (capture.kind !== "idle") append(capture.observation.epochs, event);
  };
  const observer: BrowserObserver = {
    phase: null, tick: null, snapshotJson: null, policyJson: null, contextLosses: 0,
    attach(prototype) {
      const names = ["select_manual", "select_auto", "select_water_quality", "observe", "pending", "invalidate", "applied", "failed", "unsupported"] as const;
      const timeIndex = [1, 1, 1, 4, 0, 1, 4, 5, 1];
      for (const [index, name] of names.entries()) {
        const descriptor = Object.getOwnPropertyDescriptor(prototype, name);
        if (descriptor === undefined || typeof descriptor.value !== "function") throw new Error(`Missing existing policy method: ${name}`);
        const original: (...args: unknown[]) => unknown = descriptor.value as (...args: unknown[]) => unknown;
        Object.defineProperty(prototype, name, { ...descriptor, value: function (this: ObservedPolicy, ...args: unknown[]): unknown {
          const epochBefore = name === "observe" ? this.epoch() : 0;
          const returned: unknown = Reflect.apply(original, this, args);
          const started = performance.now();
          try {
            const at = args[timeIndex[index] ?? -1];
            if (typeof at !== "number" || !Number.isFinite(at)) throw new Error("Invalid policy source clock");
            if (name === "observe" && capture.kind !== "idle") {
              const [epoch, frame, sourceMs, gpuWorkMs, arrivalMs] = args;
              if (typeof epoch !== "number" || typeof frame !== "number" || typeof sourceMs !== "number" ||
                  typeof gpuWorkMs !== "number" || typeof arrivalMs !== "number") throw new Error("Invalid forwarded GPU observation");
              append(capture.observation.forwardedGpu, { epoch, frame, sourceMs, gpuWorkMs, arrivalMs, epochBefore });
            }
            recordEpoch(this, at);
          } catch (cause) { failure(cause); }
          observerMilliseconds += performance.now() - started;
          observerCalls++;
          return returned;
        } });
      }
    },
    measure() {
      if (capture.kind !== "idle" || observer.snapshotJson === null || observer.phase !== "flight_running") throw new Error("Measurement requires one live Rust FlightRunning snapshot");
      const startMs = performance.now();
      const beforeCost = observerMilliseconds;
      const beforeCalls = observerCalls;
      const observation: LiveObservation = {
        startMs, endMs: startMs + 2000, observedEndMs: startMs, interrupted: null,
        initialSnapshotJson: observer.snapshotJson, finalSnapshotJson: observer.snapshotJson,
        epochs: [...epochs], forwardedGpu: [], frames: [], phases: [], truncated: false,
        observerErrors: [], observerMilliseconds: 0, observerCalls: 0,
      };
      capture = { kind: "collecting", observation };
      return new Promise(resolve => {
        let frame = 0;
        const finish = (cause: string | null): void => {
          if (capture.kind !== "collecting") return;
          capture = { kind: "draining", observation };
          interrupt = null;
          cancelAnimationFrame(frame);
          clearTimeout(watchdog);
          observation.observedEndMs = performance.now();
          observation.interrupted = cause;
          observation.finalSnapshotJson = observer.snapshotJson ?? observation.initialSnapshotJson;
          setTimeout(() => {
            observation.observerMilliseconds = observerMilliseconds - beforeCost;
            observation.observerCalls = observerCalls - beforeCalls;
            observation.observerErrors = [...errors];
            capture = { kind: "idle" };
            resolve(observation);
          }, 500);
        };
        const watchdog = setTimeout(() => { finish("RAF observation deadline exceeded"); }, 3500);
        interrupt = cause => { finish(cause); };
        const observe = (timestamp: number): void => {
          const at = performance.now();
          if (observer.phase !== "flight_running") { finish(`Rust phase changed to ${String(observer.phase)}`); return; }
          if (at >= observation.endMs) { finish(null); return; }
          const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas");
          append(observation.frames, { at, timestamp, phase: observer.phase, tick: observer.tick,
            width: canvas?.width ?? 0, height: canvas?.height ?? 0 });
          observerMilliseconds += performance.now() - at;
          observerCalls++;
          frame = requestAnimationFrame(observe);
        };
        frame = requestAnimationFrame(observe);
      });
    },
  };
  window.__liveAuto = observer;
  JSON.parse = (...args: Parameters<typeof JSON.parse>): unknown => {
    const returned: unknown = Reflect.apply(parse, JSON, args);
    const started = performance.now();
    try {
      if (object(returned) && returned.control_layout === "tail_incidence" && typeof returned.phase === "string" && object(returned.frame)) {
        observer.snapshotJson = args[0];
        observer.phase = returned.phase;
        const state = returned.frame.state;
        observer.tick = object(state) && typeof state.tick === "number" ? state.tick : null;
        if (capture.kind === "collecting") {
          append(capture.observation.phases, { at: started, phase: returned.phase, tick: observer.tick });
          if (returned.phase !== "flight_running") interrupt?.(`Rust phase changed to ${String(returned.phase)}`);
        }
      }
      if (object(returned) && object(returned.selection) && typeof returned.applied === "string" && object(returned.status)) {
        observer.policyJson = args[0];
      }
    } catch (cause) { failure(cause); }
    observerMilliseconds += performance.now() - started;
    observerCalls++;
    return returned;
  };
  document.addEventListener("webglcontextlost", () => { observer.contextLosses++; interrupt?.("WebGL context lost"); }, true);
})();
