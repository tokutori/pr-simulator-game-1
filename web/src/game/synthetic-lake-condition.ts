import { DEFAULT_LAKE_VISUAL_CONDITION } from "../render/contracts/lake-water.js";
import type { LakeVisualCondition } from "../render/contracts/lake-water.js";

// The synthetic scenarios do not specify fetch, wind history, or measured wave
// state. These stable render-only values preserve existing waves even when the
// instantaneous flight wind is zero; they are not wave forecasts.
const WEATHER_CONDITIONS: readonly LakeVisualCondition[] = Object.freeze([
  DEFAULT_LAKE_VISUAL_CONDITION,
  Object.freeze({ windNorthMetersPerSecond: 0, windEastMetersPerSecond: 1.32, fetchMeters: 600, detailAmplitudeScale: 1.10, patternSeed: 1 }),
  Object.freeze({ windNorthMetersPerSecond: -0.35, windEastMetersPerSecond: 1.42, fetchMeters: 600, detailAmplitudeScale: 1.20, patternSeed: 2 }),
  Object.freeze({ windNorthMetersPerSecond: -0.65, windEastMetersPerSecond: 1.54, fetchMeters: 600, detailAmplitudeScale: 1.32, patternSeed: 3 }),
  Object.freeze({ windNorthMetersPerSecond: -0.93, windEastMetersPerSecond: 1.67, fetchMeters: 600, detailAmplitudeScale: 1.45, patternSeed: 4 })
]);

export function syntheticLakeVisualCondition(weatherCode: number): LakeVisualCondition {
  const condition = WEATHER_CONDITIONS[weatherCode];
  if (!Number.isInteger(weatherCode) || condition === undefined) {
    throw new RangeError("Unknown synthetic weather class for lake appearance");
  }
  return condition;
}
