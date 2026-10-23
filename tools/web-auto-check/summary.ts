import type { LiveObservation } from "./browser-observer.js";

export function distribution(values: readonly number[]) {
  const valid = values.filter(value => Number.isFinite(value) && value >= 0).sort((first, second) => first - second);
  return { count: values.length, invalid: values.length - valid.length,
    meanMs: valid.length === 0 ? null : valid.reduce((sum, value) => sum + value, 0) / valid.length,
    p95Ms: valid[Math.ceil(valid.length * 0.95) - 1] ?? null, maximumMs: valid.at(-1) ?? null };
}

export function summarize(observation: LiveObservation) {
  const inside = (time: number) => time >= observation.startMs && time < observation.endMs;
  const frames = observation.frames.filter(frame => inside(frame.at));
  const gpu = observation.updates.flatMap(update => {
    const input = update.input;
    return input?.observation.kind === "gpu_completed" && inside(input.observation.query.source_ms)
      ? [{ query: input.observation.query, gpuWorkMs: input.observation.gpu_work_ms, receivedMs: update.observedMs,
        diagnostic: update.diagnostic }] : [];
  });
  return {
    scope: "Raw GPU completions delivered to the production owner, without a second Auto admission or freshness policy",
    windowMs: observation.endMs - observation.startMs, drainedMs: observation.drainedMs - observation.endMs,
    truncated: observation.truncated, observerMilliseconds: observation.observerMilliseconds,
    frameCount: frames.length, frames, gpu, gpuWork: distribution(gpu.map(sample => sample.gpuWorkMs)),
    rafIntervals: distribution(frames.slice(1).map((frame, index) => frame.timestamp - (frames[index]?.timestamp ?? frame.timestamp))),
    ownerUpdate: distribution(observation.updates.filter(update => inside(update.at)).map(update => update.updateMilliseconds)),
    resources: observation.resources
  };
}
