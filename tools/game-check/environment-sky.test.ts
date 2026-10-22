import { describe, expect, it } from "vitest";
import { createLakeSkyCondition } from "../../web/src/render/contracts/lake-sky.js";

const skyCondition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, cloudBaseMeters: 1500, visibilityMeters: 25_000 };

describe("render sky direction", () => {
  it("rejects nonfinite render inputs", () => {
    for (const invalid of [NaN, Infinity, -Infinity]) {
      for (const field of ["sunAzimuthDegrees", "sunElevationDegrees", "cloudFraction", "cloudBaseMeters", "visibilityMeters"] as const) {
        expect(() => createLakeSkyCondition({ ...skyCondition, [field]: invalid })).toThrow(RangeError);
      }
    }
  });
});
