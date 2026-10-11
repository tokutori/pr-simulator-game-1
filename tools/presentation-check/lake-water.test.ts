import "./lake-model-fixture.js";
import { describe, expect, it } from "vitest";
import { lakeWaterQualityProfile, DEFAULT_LAKE_VISUAL_CONDITION } from "../../web/src/render/contracts/lake-water.js";
import { createLakeWaveProjection } from "../../web/src/render/engines/three/lake-wave-projection.js";
import { LakeDetailImageBridge, LakeWaveProjectionBridge } from "../../web/pkg/birdman_game_wasm.js";

describe("shared lake water consumer", () => {
  it("uses the existing component and mesh budgets at each quality level", () => {
    for (const [quality, componentCount, meshSegments] of [["low", 6, 96], ["medium", 10, 144], ["high", 18, 224]] as const) {
      const projection = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, quality);
      expect(lakeWaterQualityProfile(quality)).toEqual({ componentCount, meshSegments });
      expect(projection.waveCount).toBe(componentCount);
      expect(projection.waveOmegaPhase.slice(componentCount).flat()).toEqual(new Array<number>((24 - componentCount) * 4).fill(0));
    }
  });

  it("preserves representative frequency bands without a TypeScript spectrum implementation", () => {
    const high = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, "high");
    for (const [quality, indices] of [["low", [0, 4, 8, 9, 13, 17]], ["medium", [0, 4, 5, 6, 8, 9, 10, 13, 14, 17]]] as const) {
      const projection = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, quality);
      expect(projection.waveOmegaPhase.slice(0, projection.waveCount)).toEqual(indices.map((index) => high.waveOmegaPhase[index]));
    }
  });

  it("keeps geometric phase independent of the detail seed", () => {
    const first = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, "high");
    const second = createLakeWaveProjection({ ...DEFAULT_LAKE_VISUAL_CONDITION, patternSeed: 41 }, "high");
    expect(second).toEqual(first);
  });

  it("rejects malformed input at the generated WASM boundary", () => {
    expect(() => new LakeWaveProjectionBridge(Number.NaN, 0, 600, 0)).toThrow();
    expect(() => new LakeWaveProjectionBridge(1, 0, 600, 3)).toThrow();
    expect(() => LakeDetailImageBridge.for_visual_condition(1, 0, 600, 1, 0.5, false)).toThrow();
    expect(() => new LakeDetailImageBridge(64, 48.5, 123, new Float64Array([1, 0]), new Float64Array())).toThrow();
    expect(() => new LakeDetailImageBridge(64, 48, 123, new Float64Array([1]), new Float64Array())).toThrow();
    expect(() => new LakeDetailImageBridge(64, 48, 123, new Float64Array([1, 0]), new Float64Array([1, 2]))).toThrow();
  });
});
