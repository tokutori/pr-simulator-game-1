import { describe, expect, it } from "vitest";
import { parseRuntimeEnvironmentSnapshot } from "../../web/src/game/runtime-environment.js";
import { createLakeSkyCondition, lakeSkySunDirectionNed } from "../../web/src/render/contracts/lake-sky.js";

const skyCondition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, visibilityMeters: 25_000 };
const recordedSky = { kind: "defined", value: {
  sun_azimuth_degrees: 135, sun_elevation_degrees: 55, cloud_fraction: 0.25, cloud_base_m: 1500, visibility_m: 25_000
} };

function environment(sky: unknown) {
  return { schema_version: 1, context: { kind: "session", phase_code: 3 }, projection: {
    kind: "available", source: "sealed", identity: { catalog_version: 2, scenario_id: 6, scenario_version: 1,
      aircraft_model_version: 2, environment_version: 6, controller_profile_version: 1, seed_low: 21, seed_high: 22 },
    metadata: { local_frame: { kind: "unavailable" }, sky,
      waves: { wind_velocity_ne_mps: [0, 0], fetch_m: 600, detail_amplitude_scale: 1, pattern_seed: 1 } }
  } };
}

describe("environment sky projection", () => {
  it("retains immutable sky metadata and explicit unrecorded availability", () => {
    const projection = parseRuntimeEnvironmentSnapshot(JSON.stringify(environment(recordedSky)), 3);
    expect(projection).toMatchObject({ kind: "available", value: { sky: { kind: "available",
      value: { condition: skyCondition, cloudBaseMeters: 1500 } } } });
    if (projection.kind !== "available" || projection.value.sky.kind !== "available") throw new Error("Expected recorded sky");
    expect(Object.isFrozen(projection.value.sky.value.condition)).toBe(true);
    expect(parseRuntimeEnvironmentSnapshot(JSON.stringify(environment({ kind: "unavailable" })), 3))
      .toMatchObject({ kind: "available", value: { sky: { kind: "unavailable", reason: "sky_not_recorded" } } });
  });

  it.each([
    null, {}, { kind: "available", value: recordedSky.value }, { kind: "unavailable", value: recordedSky.value },
    { ...recordedSky, surplus: 1 }, { kind: "defined", value: { ...recordedSky.value, surplus: 1 } },
    ...[{ sun_azimuth_degrees: -1 }, { sun_azimuth_degrees: 360 }, { sun_elevation_degrees: -91 }, { sun_elevation_degrees: 91 },
      { cloud_fraction: -0.01 }, { cloud_fraction: 1.01 }, { cloud_base_m: -1 }, { cloud_base_m: null },
      { visibility_m: 0 }, { visibility_m: "25000" }].map((change) => ({ kind: "defined", value: { ...recordedSky.value, ...change } }))
  ])("rejects malformed sky %j", (sky) => {
    expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify(environment(sky)), 3)).toThrow(RangeError);
  });

  it.each([[1, "selected"], [3, "sealed"], [9, "record"], [9, "archive"], [10, "attract"]] as const)("accepts sky only from its phase %s / source %s", (phase, source) => {
    const document = environment(recordedSky);
    expect(parseRuntimeEnvironmentSnapshot(JSON.stringify({ ...document, context: { kind: "session", phase_code: phase },
      projection: { ...document.projection, source } }), phase)).toMatchObject({ kind: "available", value: { sky: { kind: "available" } } });
    expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify(document), phase + 1)).toThrow(RangeError);
  });

  it("preserves every saved identity field and rejects an inconsistent source", () => {
    const document = environment(recordedSky);
    const projection = parseRuntimeEnvironmentSnapshot(JSON.stringify(document), 3);
    if (projection.kind !== "available") throw new Error("Expected environment");
    for (const field of ["catalogVersion", "scenarioId", "scenarioVersion", "aircraftModelVersion", "environmentVersion",
      "controllerProfileVersion", "seedLow", "seedHigh"] as const) {
      expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify(document), 3,
        { ...projection.value.identity, [field]: projection.value.identity[field] + 1 })).toThrow(RangeError);
    }
    expect(() => parseRuntimeEnvironmentSnapshot(JSON.stringify({ ...document,
      projection: { ...document.projection, source: "archive" } }), 3)).toThrow(RangeError);
    expect(parseRuntimeEnvironmentSnapshot(JSON.stringify({ ...document, projection: {
      kind: "unavailable", source: "sealed", identity: { ...document.projection.identity, environment_version: 99 }
    } }), 3)).toEqual({ kind: "unavailable", reason: "unregistered_environment_identity" });
  });

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
      for (const field of ["sunAzimuthDegrees", "sunElevationDegrees", "cloudFraction", "visibilityMeters"] as const) {
        expect(() => createLakeSkyCondition({ ...skyCondition, [field]: invalid })).toThrow(RangeError);
      }
    }
  });
});
