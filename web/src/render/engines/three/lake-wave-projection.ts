import { LakeWaveProjectionBridge, lake_mesh_spacing_domain, lake_wave_visibility_values,
  lake_grid_spacing_gradient_values, lake_wave_derivative_bound } from "../../../../pkg/birdman_game_wasm.js";
import { lakeWaterQualityCode } from "../../contracts/lake-water.js";
import type { LakeVisualCondition, LakeWaterQuality } from "../../contracts/lake-water.js";

export type LakeWaveUpload = readonly [number, number, number, number];

export interface LakeMeshSpacingDomain {
  readonly minimum: number;
  readonly maximum: number;
  readonly gradientMagnitude: number;
}

export interface LakeWaveProjectionInputs {
  readonly waveKAmplitude: readonly LakeWaveUpload[];
  readonly waveOmegaPhase: readonly LakeWaveUpload[];
  readonly waveCount: number;
  readonly visualWaveHeight: number;
  readonly spacing: LakeMeshSpacingDomain;
  readonly horizontalDerivativeBound: number;
  readonly choppiness: number;
}

function requiredNumber(values: Float32Array | Float64Array, index: number): number {
  const value = values[index];
  if (value === undefined) throw new RangeError("Incomplete shared lake bulk output");
  return value;
}

function sharedModelCall<Value>(operation: () => Value): Value {
  try { return operation(); }
  catch (error: unknown) {
    if (typeof error === "string") throw new RangeError(error, { cause: error });
    throw error;
  }
}

function spacingDomain(values: Float64Array): LakeMeshSpacingDomain {
  if (values.length !== 3) throw new RangeError("Invalid shared lake spacing output");
  return Object.freeze({ minimum: requiredNumber(values, 0), maximum: requiredNumber(values, 1),
    gradientMagnitude: requiredNumber(values, 2) });
}

export function lakeMeshSpacingDomain(segments: number): LakeMeshSpacingDomain {
  return spacingDomain(sharedModelCall(() => lake_mesh_spacing_domain(segments)));
}

export function lakeWaveVisibility(waveNumber: number, spacing: number): Readonly<{ value: number; derivative: number }> {
  const values = lake_wave_visibility_values(waveNumber, spacing);
  return Object.freeze({ value: requiredNumber(values, 0), derivative: requiredNumber(values, 1) });
}

export function lakeGridSpacingGradient(localX: number, localZ: number, magnitude: number): readonly [number, number] {
  const values = lake_grid_spacing_gradient_values(localX, localZ, magnitude);
  return Object.freeze([requiredNumber(values, 0), requiredNumber(values, 1)]);
}

export function lakeWaveDisplacementDerivativeBound(wave: LakeWaveUpload, domain: LakeMeshSpacingDomain): number {
  return sharedModelCall(() => lake_wave_derivative_bound(new Float32Array(wave),
    new Float64Array([domain.minimum, domain.maximum, domain.gradientMagnitude])));
}

function waveUploads(values: Float32Array): readonly LakeWaveUpload[] {
  if (values.length !== 96) throw new RangeError("Invalid shared lake wave upload output");
  return Object.freeze(Array.from({ length: 24 }, (_, index): LakeWaveUpload => Object.freeze([
    requiredNumber(values, index * 4), requiredNumber(values, index * 4 + 1),
    requiredNumber(values, index * 4 + 2), requiredNumber(values, index * 4 + 3)
  ])));
}

export function createLakeWaveProjection(condition: LakeVisualCondition, quality: LakeWaterQuality): LakeWaveProjectionInputs {
  const projection = sharedModelCall(() => new LakeWaveProjectionBridge(condition.windNorthMetersPerSecond,
    condition.windEastMetersPerSecond, condition.fetchMeters, lakeWaterQualityCode(quality)));
  try {
    return Object.freeze({ waveKAmplitude: waveUploads(projection.wave_k_amplitude()),
      waveOmegaPhase: waveUploads(projection.wave_omega_phase()), waveCount: projection.wave_count,
      visualWaveHeight: projection.visual_wave_height, spacing: spacingDomain(projection.spacing()),
      horizontalDerivativeBound: projection.horizontal_derivative_bound, choppiness: projection.choppiness });
  } finally { projection.free(); }
}

export const LAKE_WAVE_PACKET_GLSL = /* glsl */ `
float lakeWaveHash(vec2 cell) {
  return fract(sin(dot(cell, vec2(127.1, 311.7))) * 43758.5453);
}

vec3 lakeValueNoise(vec2 point) {
  vec2 cell = floor(point);
  vec2 fraction = fract(point);
  vec2 blend = fraction * fraction * (3.0 - 2.0 * fraction);
  vec2 blendGradient = 6.0 * fraction * (1.0 - fraction);
  float lowerLeft = lakeWaveHash(cell);
  float lowerRight = lakeWaveHash(cell + vec2(1.0, 0.0));
  float upperLeft = lakeWaveHash(cell + vec2(0.0, 1.0));
  float upperRight = lakeWaveHash(cell + vec2(1.0, 1.0));
  float value = mix(mix(lowerLeft, lowerRight, blend.x), mix(upperLeft, upperRight, blend.x), blend.y);
  vec2 gradient = vec2(
    blendGradient.x * mix(lowerRight - lowerLeft, upperRight - upperLeft, blend.y),
    blendGradient.y * mix(upperLeft - lowerLeft, upperRight - lowerRight, blend.x)
  );
  return vec3(value * 2.0 - 1.0, gradient * 2.0);
}

mat3 lakeWaveModulation(vec2 point, vec2 direction, float waveNumber, float seed) {
  vec2 across = vec2(-direction.y, direction.x);
  float frequency = max(0.11, waveNumber * 0.055);
  vec2 coordinate = vec2(dot(point, across), dot(point, direction)) * frequency + vec2(seed * 1.7, seed * 2.3);
  vec2 secondCoordinate = vec2(coordinate.x * 1.7 + coordinate.y * 0.37,
    coordinate.y * 1.9 - coordinate.x * 0.24) + vec2(13.7, -8.2);
  vec3 broad = lakeValueNoise(coordinate);
  vec3 fine = lakeValueNoise(secondCoordinate);
  vec2 broadGradient = frequency * (across * broad.y + direction * broad.z);
  vec2 fineGradient = frequency * (fine.y * (across * 1.7 + direction * 0.37)
    + fine.z * (across * -0.24 + direction * 1.9));
  float packet = 1.0 + 0.38 * broad.x + 0.12 * fine.x;
  vec2 packetGradient = 0.38 * broadGradient + 0.12 * fineGradient;
  float offset = 4.6 * broad.x + 1.4 * fine.x;
  vec2 offsetGradient = 4.6 * broadGradient + 1.4 * fineGradient;
  return mat3(vec3(packet, packetGradient), vec3(offset, offsetGradient), vec3(0.0));
}
`;

export const LAKE_WAVE_PROJECTION_GLSL = /* glsl */ `
uniform vec4 uWaveKAmplitude[24];
uniform vec4 uWaveOmegaPhase[24];
uniform int uWaveCount;
${LAKE_WAVE_PACKET_GLSL}

struct LakeWaveProjection {
  vec3 position;
  vec4 horizontalJacobian;
  vec2 heightGradient;
  float crest;
  float slopeEnergy;
};

vec2 lakeGridSpacingGradient(vec2 localPoint, float magnitude) {
  vec2 absolutePoint = abs(localPoint);
  if (absolutePoint.x > absolutePoint.y) return vec2(sign(localPoint.x) * magnitude, 0.0);
  if (absolutePoint.y > absolutePoint.x) return vec2(0.0, sign(localPoint.y) * magnitude);
  return sign(localPoint) * (0.5 * magnitude);
}

LakeWaveProjection projectLakeWaves(vec3 localPosition, vec2 worldXZ, float gridSpacing,
    float spacingGradientMagnitude, float timeSeconds, float choppiness) {
  LakeWaveProjection result;
  result.position = localPosition;
  result.horizontalJacobian = vec4(1.0, 0.0, 0.0, 1.0);
  result.heightGradient = vec2(0.0);
  result.crest = 0.0;
  result.slopeEnergy = 0.0;
  vec2 spacingGradient = lakeGridSpacingGradient(localPosition.xz, spacingGradientMagnitude);
  for (int index = 0; index < 24; index++) {
    if (index < uWaveCount) {
      vec4 wave = uWaveKAmplitude[index];
      vec4 timing = uWaveOmegaPhase[index];
      vec2 direction = wave.xy;
      float waveNumber = wave.z;
      float visibility = 1.0 - smoothstep(1.3, 2.5, waveNumber * gridSpacing);
      float transition = clamp((waveNumber * gridSpacing - 1.3) / 1.2, 0.0, 1.0);
      vec2 visibilityGradient = (-6.0 * waveNumber / 1.2 * transition * (1.0 - transition)) * spacingGradient;
      result.slopeEnergy += waveNumber * wave.w;
      if (visibility == 0.0 && all(equal(visibilityGradient, vec2(0.0)))) continue;
      mat3 modulation = lakeWaveModulation(worldXZ, direction, waveNumber, timing.y);
      vec3 packet = modulation[0];
      vec3 warp = modulation[1];
      float amplitude = wave.w * visibility * packet.x;
      vec2 amplitudeGradient = wave.w * (visibility * packet.yz + packet.x * visibilityGradient);
      float phase = waveNumber * dot(direction, worldXZ) + warp.x - timing.x * timeSeconds + timing.y;
      vec2 phaseGradient = waveNumber * direction + warp.yz;
      float phaseSine = sin(phase);
      float phaseCosine = cos(phase);
      float crestBasis = 0.5 + 0.5 * phaseSine;
      result.position.xz += choppiness * amplitude * direction * phaseCosine;
      result.position.y += amplitude * (2.0 * crestBasis * crestBasis * crestBasis - 0.625);
      vec2 horizontalGradient = choppiness * (phaseCosine * amplitudeGradient - amplitude * phaseSine * phaseGradient);
      result.horizontalJacobian += vec4(direction.x * horizontalGradient, direction.y * horizontalGradient);
      result.heightGradient += amplitude * 3.0 * crestBasis * crestBasis * phaseCosine * phaseGradient
        + (2.0 * crestBasis * crestBasis * crestBasis - 0.625) * amplitudeGradient;
      result.crest += phaseSine * waveNumber * amplitude;
    }
  }
  return result;
}
`;
