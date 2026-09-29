import { describe, expect, it } from "vitest";
import { LAUNCH_ORIGIN_WGS84, LAUNCH_PLATFORM, nedToWgs84 } from "./launch-venue.js";

describe("Lake Biwa launch frame", () => {
  it("locates the NED origin at the supplied launch coordinate", () => {
    expect(nedToWgs84(0, 0)).toEqual(LAUNCH_ORIGIN_WGS84);
  });

  it("moves latitude north and longitude west along the northwest launch course", () => {
    const point = nedToWgs84(100 / Math.SQRT2, -100 / Math.SQRT2);
    expect(point.latitudeDegrees).toBeGreaterThan(LAUNCH_ORIGIN_WGS84.latitudeDegrees);
    expect(point.longitudeDegrees).toBeLessThan(LAUNCH_ORIGIN_WGS84.longitudeDegrees);
    expect(LAUNCH_PLATFORM.frontLipAboveWaterMeters +
      LAUNCH_PLATFORM.lengthMeters * Math.tan(LAUNCH_PLATFORM.downwardSlopeDegrees * Math.PI / 180))
      .toBeCloseTo(11.223, 2);
  });
});
