import { expect, it } from "vitest";
import { distribution, summarize } from "./summary.js";

it("summarizes raw source-window durations and retains owner diagnostics without another acceptance policy", () => {
  const observation: Parameters<typeof summarize>[0] = { startMs: 1000, endMs: 3000, drainedMs: 3500,
    frames: [], resources: [], truncated: false, observerMilliseconds: 1, updates: [900, 1001, 2999, 3100].map((source, index) => ({
      at: 3100 + index, updateMilliseconds: 2, observedMs: 3100 + index, input: { kind: "graphics", renderer_resource_id: 1,
        observation: { kind: "gpu_completed", query: { renderer_resource_id: 1, epoch: index, frame: index, source_ms: source }, gpu_work_ms: 10 + index } },
      diagnostic: "original owner diagnostic"
    })) };
  const result = summarize(observation);
  expect(result.gpu.map(sample => sample.query.source_ms)).toEqual([1001, 2999]);
  expect(result.gpu.map(sample => sample.diagnostic)).toEqual(["original owner diagnostic", "original owner diagnostic"]);
  expect(result.gpuWork).toMatchObject({ count: 2, meanMs: 11.5, p95Ms: 12 });
  expect(distribution([]).p95Ms).toBeNull();
  expect(distribution([1, Number.NaN, -1])).toMatchObject({ count: 3, invalid: 2, meanMs: 1 });
});
