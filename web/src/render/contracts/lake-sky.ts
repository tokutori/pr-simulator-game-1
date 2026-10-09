export interface LakeSkyCondition {
  readonly sunAzimuthDegrees: number;
  readonly sunElevationDegrees: number;
  readonly cloudFraction: number;
  readonly visibilityMeters: number;
}

export function createLakeSkyCondition(condition: LakeSkyCondition): LakeSkyCondition {
  if (!Number.isFinite(condition.sunAzimuthDegrees) || condition.sunAzimuthDegrees < 0 || condition.sunAzimuthDegrees >= 360 ||
      !Number.isFinite(condition.sunElevationDegrees) || condition.sunElevationDegrees < -90 || condition.sunElevationDegrees > 90 ||
      !Number.isFinite(condition.cloudFraction) || condition.cloudFraction < 0 || condition.cloudFraction > 1 ||
      !Number.isFinite(condition.visibilityMeters) || condition.visibilityMeters <= 0) {
    throw new RangeError("Invalid lake sky condition");
  }
  return Object.freeze({ sunAzimuthDegrees: condition.sunAzimuthDegrees, sunElevationDegrees: condition.sunElevationDegrees,
    cloudFraction: condition.cloudFraction, visibilityMeters: condition.visibilityMeters });
}

export function sameLakeSkyCondition(first: LakeSkyCondition, second: LakeSkyCondition): boolean {
  return first.sunAzimuthDegrees === second.sunAzimuthDegrees && first.sunElevationDegrees === second.sunElevationDegrees &&
    first.cloudFraction === second.cloudFraction && first.visibilityMeters === second.visibilityMeters;
}

export function lakeSkySunDirectionNed(condition: LakeSkyCondition): Readonly<{ north: number; east: number; down: number }> {
  const validated = createLakeSkyCondition(condition);
  const azimuth = validated.sunAzimuthDegrees * Math.PI / 180;
  const elevation = validated.sunElevationDegrees * Math.PI / 180;
  const horizontal = Math.cos(elevation);
  return Object.freeze({ north: horizontal * Math.cos(azimuth), east: horizontal * Math.sin(azimuth), down: -Math.sin(elevation) });
}
