import type { ApplicationRuntimeBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { ApplicationGraphicsResource, ApplicationInput, ApplicationObservation, ApplicationOutput } from "../../web/src/generated/application-contracts.js";

export interface RuntimeUpdate {
  at: number;
  observedMs: number;
  input: Extract<ApplicationInput, { kind: "graphics" }> | null;
  diagnostic: string | null;
  updateMilliseconds: number;
}

export interface LiveObservation {
  startMs: number;
  endMs: number;
  drainedMs: number;
  updates: RuntimeUpdate[];
  resources: { at: number; resource: ApplicationGraphicsResource | null }[];
  frames: { at: number; timestamp: number; scene: string | null; overlay: string | null; width: number; height: number }[];
  truncated: boolean;
  observerMilliseconds: number;
}

declare global {
  interface Window {
    __runtimeMeasure: {
      attach(prototype: ApplicationRuntimeBridge): void;
      measure(): Promise<LiveObservation>;
    };
  }
}

(() => {
  let capture: LiveObservation | null = null;
  let resourceKey: string | undefined;
  const append = <Value>(values: Value[], value: Value): void => {
    if (values.length < 4096) values.push(value);
    else if (capture !== null) capture.truncated = true;
  };
  window.__runtimeMeasure = {
    attach(prototype) {
      const update = Object.getOwnPropertyDescriptor(prototype, "update")?.value as ApplicationRuntimeBridge["update"] | undefined;
      if (update === undefined) throw new Error("Production runtime update method is unavailable");
      prototype.update = function (this: ApplicationRuntimeBridge, ...args: Parameters<typeof update>): ReturnType<typeof update> {
        const at = performance.now();
        const output = Reflect.apply(update, this, args);
        const completed = performance.now();
        if (capture !== null) {
          const observation = JSON.parse(args[0]) as ApplicationObservation;
          const result = JSON.parse(output) as ApplicationOutput;
          append(capture.updates, { at, observedMs: observation.observed_ms, input: observation.input.kind === "graphics" ? observation.input : null,
            diagnostic: result.diagnostic ?? null, updateMilliseconds: completed - at });
          if (result.drawing != null) {
            const resource = result.drawing.graphics.resource ?? null;
            const key = JSON.stringify(resource);
            if (key !== resourceKey) { append(capture.resources, { at, resource }); resourceKey = key; }
          }
          capture.observerMilliseconds += performance.now() - completed;
        }
        return output;
      };
    },
    async measure() {
      const startMs = performance.now();
      const observation: LiveObservation = { startMs, endMs: startMs + 2000, drainedMs: 0,
        updates: [], resources: [], frames: [], truncated: false, observerMilliseconds: 0 };
      capture = observation;
      resourceKey = undefined;
      let frame: number;
      const record = (timestamp: number): void => {
        const at = performance.now();
        const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas:not([hidden])");
        const shell = document.querySelector<HTMLElement>(".screen-ui-shell");
        append(observation.frames, { at, timestamp, scene: shell?.dataset.scene ?? null, overlay: shell?.dataset.overlay ?? null,
          width: canvas?.width ?? 0, height: canvas?.height ?? 0 });
        observation.observerMilliseconds += performance.now() - at;
        frame = requestAnimationFrame(record);
      };
      frame = requestAnimationFrame(record);
      try {
        await new Promise<void>(complete => { setTimeout(complete, 2500); });
        observation.drainedMs = performance.now();
        return observation;
      } finally { cancelAnimationFrame(frame); capture = null; }
    }
  };
})();
