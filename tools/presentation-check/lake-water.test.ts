import { describe, expect, it } from "vitest";
import {
  createLakeWaveSpectrum,
  lakeWaterQualityProfile,
  selectLakeWaveComponentsForQuality
} from "../../web/src/render/contracts/lake-water.js";

describe("lake water quality", () => {
  const spectrum = createLakeWaveSpectrum(0.54, 1.07, 600, 18);

  it("uses the configured wave-component budget at each quality level", () => {
    for (const quality of ["low", "medium", "high"] as const) {
      expect(selectLakeWaveComponentsForQuality(spectrum, quality)).toHaveLength(
        lakeWaterQualityProfile(quality).componentCount
      );
    }
  });

  it("preserves representative frequency bands at low and medium quality", () => {
    const indicesFor = (quality: "low" | "medium" | "high"): number[] =>
      selectLakeWaveComponentsForQuality(spectrum, quality)
        .map((component) => spectrum.components.indexOf(component));

    const low = indicesFor("low");
    const medium = indicesFor("medium");
    const high = indicesFor("high");

    expect(low).toEqual([1, 4, 7, 10, 13, 16]);
    expect(medium).toEqual([1, 3, 4, 6, 7, 9, 10, 12, 13, 16]);
    expect(high).toEqual(Array.from({ length: 18 }, (_, index) => index));
    expect(low.every((index) => medium.includes(index))).toBe(true);
    expect(medium.every((index) => high.includes(index))).toBe(true);
  });
});
