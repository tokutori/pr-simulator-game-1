import { parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";
import { parseScreenRenderAutoSnapshot } from "../../web/src/render/contracts/screen-render-auto.js";
import type { EpochObservation, LiveObservation } from "./browser-observer.js";

export function distribution(values: readonly number[]) {
  const valid = values.filter(value => Number.isFinite(value) && value >= 0).sort((left, right) => left - right);
  return { count: values.length, invalid: values.length - valid.length,
    meanMs: valid.length === 0 ? null : valid.reduce((sum, value) => sum + value, 0) / valid.length,
    p95Ms: valid[Math.ceil(valid.length * 0.95) - 1] ?? null, maximumMs: valid.at(-1) ?? null };
}

export function summarize(observation: LiveObservation) {
  const initial = parseTailSessionSnapshot(observation.initialSnapshotJson);
  const final = parseTailSessionSnapshot(observation.finalSnapshotJson);
  const inWindow = (at: number) => at >= observation.startMs && at < Math.min(observation.endMs, observation.observedEndMs);
  const forwarded = observation.forwardedGpu.filter(sample => inWindow(sample.sourceMs));
  const groups = new Map<number, { source: EpochObservation; values: number[]; frames: number[]; firstSourceMs: number; lastSourceMs: number }>();
  let unattributed = 0;
  let stale = 0;
  let repeatedOrRegressive = 0;
  for (const sample of forwarded) {
    const source = observation.epochs.findLast(event => event.epoch === sample.epoch && event.at <= sample.sourceMs);
    if (source === undefined) { unattributed++; continue; }
    if (sample.epoch !== sample.epochBefore || sample.arrivalMs - sample.sourceMs > 500 || sample.arrivalMs < sample.sourceMs) { stale++; continue; }
    const group = groups.get(sample.epoch) ?? { source, values: [], frames: [], firstSourceMs: sample.sourceMs, lastSourceMs: sample.sourceMs };
    const previous = group.frames.at(-1);
    if (previous !== undefined && sample.frame <= previous) { repeatedOrRegressive++; continue; }
    group.values.push(sample.gpuWorkMs);
    group.frames.push(sample.frame);
    group.lastSourceMs = sample.sourceMs;
    groups.set(sample.epoch, group);
  }
  const gpu = [...groups.entries()].map(([epoch, group]) => {
    const snapshot = parseScreenRenderAutoSnapshot(group.source.snapshotJson);
    const first = Math.min(...group.frames);
    const last = Math.max(...group.frames);
    const unique = new Set(group.frames).size;
    const statistics = distribution(group.values);
    const limited = statistics.count < 20;
    return { epoch, confirmedAtMs: group.source.at, selection: snapshot.selection, applied: snapshot.applied,
      shadow: snapshot.shadow, statistics, limited, sourceSpanMs: group.lastSourceMs - group.firstSourceMs,
      gpuWorkBudgetMet: limited || statistics.p95Ms === null ? null : statistics.p95Ms <= 12.5, uniqueSourceFrames: unique,
      observedSourceFrameSpan: last - first + 1, withinObservedSpanCoverage: unique / (last - first + 1) };
  });
  const frames = observation.frames.filter(frame => inWindow(frame.at));
  const intervals = frames.slice(1).map((frame, index) => frame.timestamp - (frames[index]?.timestamp ?? frame.timestamp));
  const sourceTimes = [...groups.values()].flatMap(group => [group.firstSourceMs, group.lastSourceMs]);
  const uniqueGpuSamples = [...groups.values()].reduce((count, group) => count + group.frames.length, 0);
  const sourceStart = sourceTimes.length === 0 ? null : Math.min(...sourceTimes);
  const sourceEnd = sourceTimes.length === 0 ? null : Math.max(...sourceTimes);
  const sourceWindowCoverage = sourceStart === null || sourceEnd === null ? 0 : (sourceEnd - sourceStart) / (observation.endMs - observation.startMs);
  const tickAdvance = (initial.phaseCode === 5 || initial.phaseCode === 6) && (final.phaseCode === 5 || final.phaseCode === 6 || final.phaseCode === 7)
    ? final.frame.state.tick - initial.frame.state.tick : null;
  const problems = [...observation.observerErrors];
  if (observation.interrupted !== null) problems.push(observation.interrupted);
  if (observation.truncated) problems.push("Bounded observation capacity exceeded");
  if (initial.phaseCode !== 5 || final.phaseCode !== 5 || frames.some(frame => frame.phase !== 5) || observation.phases.some(state => state.phase !== 5)) problems.push("The window contains a non-Running Rust phase");
  if (tickAdvance === null || tickAdvance <= 0) problems.push("Rust physical tick did not advance");
  if (frames.length < 2) problems.push("Insufficient RAF observations");
  if (gpu.length === 0) problems.push("No attributable fresh policy-forwarded GPU sample");
  if (uniqueGpuSamples < 20 || sourceStart === null || sourceEnd === null || sourceWindowCoverage < 0.9 ||
      sourceStart - observation.startMs > 100 || observation.endMs - sourceEnd > 100) problems.push("GPU source observations do not span the fixed window");
  if (unattributed > 0 || stale > 0 || repeatedOrRegressive > 0) problems.push("GPU source metadata is incomplete, stale, repeated, or regressive");
  if (gpu.some(group => group.statistics.invalid > 0 || group.withinObservedSpanCoverage < 0.9 || group.selection.kind !== "auto")) problems.push("GPU values, within-span coverage, or Auto selection are incomplete");
  if (frames.some(frame => {
    const source = observation.epochs.findLast(event => event.at <= frame.at);
    if (source === undefined) return true;
    const snapshot = parseScreenRenderAutoSnapshot(source.snapshotJson);
    const scale = Number(snapshot.applied) / 100;
    return frame.width !== 1280 * scale || frame.height !== 720 * scale;
  })) problems.push("Actual canvas dimensions do not match the source-time confirmed scale");
  return { complete: problems.length === 0, problems, gpuWorkBudgetMs: 12.5, nominalWindowMs: observation.endMs - observation.startMs,
    observedWindowMs: observation.observedEndMs - observation.startMs, initial, final, tickAdvance,
    rafIntervals: distribution(intervals), forwardedSourceSamples: forwarded.length, uniqueGpuSamples, sourceStart, sourceEnd, sourceWindowCoverage, unattributed, stale, repeatedOrRegressive, gpu,
    scope: "Policy-forwarded real GPU samples, grouped by source epoch. Within-span coverage excludes unissued/disjoint/old-epoch queries and does not establish whole-window query coverage." };
}
