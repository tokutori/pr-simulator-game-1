import { describe, expect, it } from "vitest";
import { createLakeSkyCondition, lakeSkySunDirectionNed } from "../../web/src/render/contracts/lake-sky.js";

const skyCondition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, cloudBaseMeters: 1500, visibilityMeters: 25_000 };

describe("render sky direction", () => {
  it.each([[0, 1, 0], [90, 0, 1], [180, -1, 0], [270, 0, -1]])("maps north-clockwise azimuth %s to the independent cardinal NED vector", (azimuth, north, east) => {
    const direction = lakeSkySunDirectionNed({ ...skyCondition, sunAzimuthDegrees: azimuth, sunElevationDegrees: 0 });
    expect(direction.north).toBeCloseTo(north, 14);
    expect(direction.east).toBeCloseTo(east, 14);
    expect(direction.down).toBeCloseTo(0, 14);
    expect(Math.hypot(direction.north, direction.east, direction.down)).toBeCloseTo(1, 14);
  });

  it("accepts vertical sun and rejects nonfinite render inputs", () => {
    expect(lakeSkySunDirectionNed({ ...skyCondition, sunElevationDegrees: 90 }).down).toBe(-1);
    expect(lakeSkySunDirectionNed({ ...skyCondition, sunElevationDegrees: -90 }).down).toBe(1);
    for (const invalid of [NaN, Infinity, -Infinity]) {
      for (const field of ["sunAzimuthDegrees", "sunElevationDegrees", "cloudFraction", "cloudBaseMeters", "visibilityMeters"] as const) {
        expect(() => createLakeSkyCondition({ ...skyCondition, [field]: invalid })).toThrow(RangeError);
      }
    }
  });
});
