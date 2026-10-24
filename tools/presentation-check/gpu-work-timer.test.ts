import { describe, expect, it } from "vitest";
import { GpuWorkTimer } from "../../web/src/render/engines/three/gpu-work-timer.js";
import { gpuTimerFixture } from "./gpu-timer-fixture.js";

describe("WebGL GPU-work timer boundary", () => {
  it("rejects malformed API result types without accepting a sample", () => {
    for (const boundary of ["disjoint state", "availability state", "elapsed duration"] as const) {
      const { gl, context } = gpuTimerFixture();
      const timer = new GpuWorkTimer(context);
      timer.begin({ renderer_resource_id: 100, epoch: 1, frame: 1, source_ms: 0 }); timer.end(true);
      if (boundary === "disjoint state") gl.getParameter.mockReturnValue(null);
      else gl.getQueryParameter.mockImplementation((_query, parameter) => parameter === 2 ? boundary === "elapsed duration" ? true : 1 : null);
      expect(() => timer.poll()).toThrow(`invalid ${boundary}`);
      timer.clear();
      expect(gl.deleteQuery).toHaveBeenCalledTimes(1);
    }
  });

  it("defers pending results and preserves nanosecond precision without a 32-bit cast", () => {
    const { gl, context } = gpuTimerFixture();
    const timer = new GpuWorkTimer(context);
    const source = { renderer_resource_id: 100, epoch: 1, frame: 2, source_ms: 10 };
    timer.begin(source);
    timer.end(true);
    gl.getQueryParameter.mockImplementation((_query, parameter) => parameter === 2 ? false : 8_000_000_000);
    expect(timer.poll()).toEqual([]);
    expect(gl.getQueryParameter.mock.calls.every((call) => call[1] === 2)).toBe(true);
    gl.getQueryParameter.mockImplementation((_query, parameter) => parameter === 2 ? true : 8_000_000_000);
    expect(timer.poll()).toEqual([{ kind: "sample", source, milliseconds: 8000 }]);
    expect(gl.deleteQuery).toHaveBeenCalledTimes(1);
    expect(timer.poll()).toEqual([]);
    for (const [nanoseconds, milliseconds] of [[-1_000_000, -1], [Number.NaN, null]] as const) {
      gl.deleteQuery.mockClear();
      gl.getQueryParameter.mockImplementation((_query, parameter) => parameter === 2 ? true : nanoseconds);
      timer.begin(source);
      timer.end(true);
      expect(timer.poll()).toEqual([{ kind: "sample", source, milliseconds }]);
      expect(gl.deleteQuery).toHaveBeenCalledTimes(1);
      expect(timer.poll()).toEqual([]);
      expect(gl.deleteQuery).toHaveBeenCalledTimes(1);
    }
  });

  it("never modifies a foreign query and bounds owned pending queries", () => {
    const { gl, context, foreign } = gpuTimerFixture();
    const timer = new GpuWorkTimer(context);
    const query = {} as WebGLQuery;
    foreign(query);
    timer.begin({ renderer_resource_id: 100, epoch: 1, frame: 1, source_ms: 0 });
    timer.end(true);
    expect(gl.beginQuery).not.toHaveBeenCalled();
    expect(gl.endQuery).not.toHaveBeenCalled();
    expect(gl.deleteQuery).not.toHaveBeenCalled();
    foreign(null);
    for (let frame = 2; frame < 10; frame++) {
      timer.begin({ renderer_resource_id: 100, epoch: 1, frame, source_ms: frame });
      timer.end(true);
    }
    expect(gl.createQuery).toHaveBeenCalledTimes(4);
    timer.clear();
    expect(gl.deleteQuery).toHaveBeenCalledTimes(4);
  });

  it("discards all owned results on disjoint and reports unsupported again after reselection", () => {
    const { gl, context } = gpuTimerFixture();
    const timer = new GpuWorkTimer(context);
    timer.begin({ renderer_resource_id: 100, epoch: 1, frame: 1, source_ms: 0 });
    timer.end(true);
    gl.getParameter.mockReturnValue(true);
    expect(timer.poll()).toEqual([{ kind: "disjoint" }]);
    expect(gl.getQueryParameter).not.toHaveBeenCalled();
    timer.restored();
    gl.getExtension.mockReturnValue(null);
    for (let frame = 2; frame < 4; frame++) {
      expect(timer.begin({ renderer_resource_id: 100, epoch: 2, frame, source_ms: frame })).toMatchObject({ kind: "unsupported" });
    }
    expect(gl.getExtension).toHaveBeenCalledTimes(2);
  });

  it("deletes its owned query after an interrupted render or end failure", () => {
    const { gl, context } = gpuTimerFixture();
    const timer = new GpuWorkTimer(context);
    timer.begin({ renderer_resource_id: 100, epoch: 1, frame: 1, source_ms: 0 });
    timer.end(false);
    expect(gl.deleteQuery).toHaveBeenCalledTimes(1);
    timer.begin({ renderer_resource_id: 100, epoch: 1, frame: 2, source_ms: 1 });
    gl.endQuery.mockImplementationOnce(() => { throw new Error("context failure"); });
    expect(() => { timer.end(true); }).toThrow("context failure");
    expect(gl.deleteQuery).toHaveBeenCalledTimes(2);
  });
});
