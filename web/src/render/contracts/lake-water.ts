import type { GameUiPlatformInput } from "../../generated/game-ui-input.js";
import { lake_water_quality_profile } from "../../../pkg/birdman_game_wasm.js";

export interface LakeVisualCondition {
  readonly windNorthMetersPerSecond: number;
  readonly windEastMetersPerSecond: number;
  readonly fetchMeters: number;
  readonly detailAmplitudeScale: number;
  readonly patternSeed: number;
}

export const DEFAULT_LAKE_VISUAL_CONDITION: LakeVisualCondition = Object.freeze({
  windNorthMetersPerSecond: 0.54,
  windEastMetersPerSecond: 1.07,
  fetchMeters: 600,
  detailAmplitudeScale: 1,
  patternSeed: 0
});

export type LakeWaterQuality = GameUiPlatformInput["water_quality"];

export interface LakeWaterQualityProfile {
  readonly componentCount: number;
  readonly meshSegments: number;
}

export function lakeWaterQualityCode(quality: LakeWaterQuality): number {
  switch (quality) {
    case "low": return 0;
    case "medium": return 1;
    case "high": return 2;
    default: throw new RangeError("Invalid lake water quality");
  }
}

export function lakeWaterQualityProfile(quality: LakeWaterQuality): LakeWaterQualityProfile {
  const values = lake_water_quality_profile(lakeWaterQualityCode(quality));
  const componentCount = values[0];
  const meshSegments = values[1];
  if (values.length !== 2 || componentCount === undefined || meshSegments === undefined) {
    throw new RangeError("Invalid shared lake quality profile");
  }
  return Object.freeze({ componentCount, meshSegments });
}
