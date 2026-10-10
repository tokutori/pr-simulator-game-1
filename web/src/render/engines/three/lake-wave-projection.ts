import { createLakeWaveSpectrum, lakeWaterQualityProfile, selectLakeWaveComponentsForQuality } from "../../contracts/lake-water.js";
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

const FLOAT32_RESERVE = 2 ** -12;
const NOISE_DERIVATIVE_FACTOR = 48.861639589;

export function lakeMeshSpacingDomain(segments: number): LakeMeshSpacingDomain {
  if (!Number.isSafeInteger(segments) || segments < 1) throw new RangeError("Invalid lake mesh segments");
  const minimum = 6000 * 7.2 / (segments * Math.expm1(7.2));
  return Object.freeze({ minimum: Math.fround(minimum), maximum: Math.fround(minimum + 3000 * 14.4 / segments),
    gradientMagnitude: Math.fround(14.4 / segments) });
}

export function lakeWaveVisibility(waveNumber: number, spacing: number): Readonly<{ value: number; derivative: number }> {
  const transition = Math.max(0, Math.min(1, (waveNumber * spacing - 1.3) / 1.2));
  return Object.freeze({ value: 1 - transition * transition * (3 - 2 * transition),
    derivative: -6 * waveNumber * transition * (1 - transition) / 1.2 });
}

export function lakeGridSpacingGradient(localX: number, localZ: number, magnitude: number): readonly [number, number] {
  const absoluteX = Math.abs(localX);
  const absoluteZ = Math.abs(localZ);
  const gradient: [number, number] = absoluteX > absoluteZ ? [Math.sign(localX) * magnitude, 0]
    : absoluteZ > absoluteX ? [0, Math.sign(localZ) * magnitude]
    : [Math.sign(localX) * magnitude * 0.5, Math.sign(localZ) * magnitude * 0.5];
  return Object.freeze(gradient);
}

export function lakeWaveDisplacementDerivativeBound(wave: LakeWaveUpload, domain: LakeMeshSpacingDomain): number {
  const [directionX, directionZ, waveNumber, amplitude] = wave;
  if (![...wave, domain.minimum, domain.maximum, domain.gradientMagnitude].every(Number.isFinite)
      || waveNumber < 0 || domain.minimum < 0 || domain.maximum < domain.minimum || domain.gradientMagnitude < 0) {
    throw new RangeError("Invalid lake wave derivative inputs");
  }
  if (waveNumber * domain.minimum >= 2.5 || amplitude === 0) return 0;
  const directionNorm = Math.hypot(directionX, directionZ);
  const frequency = Math.max(0.11, 0.055 * waveNumber);
  const phaseAndPacketBound = directionNorm * (1.5 * waveNumber + NOISE_DERIVATIVE_FACTOR * frequency);
  const evaluate = (spacing: number): number => {
    const visibility = lakeWaveVisibility(waveNumber, spacing);
    return visibility.value * phaseAndPacketBound + 1.5 * Math.abs(visibility.derivative) * domain.gradientMagnitude;
  };
  let maximum = Math.max(evaluate(domain.minimum), evaluate(domain.maximum));
  if (waveNumber > 0 && phaseAndPacketBound > 0) {
    const derivativeTerm = 7.5 * waveNumber * domain.gradientMagnitude;
    const stationary = derivativeTerm / (3 * phaseAndPacketBound + derivativeTerm
      + Math.hypot(3 * phaseAndPacketBound, derivativeTerm));
    const spacing = (1.3 + 1.2 * stationary) / waveNumber;
    if (spacing >= domain.minimum && spacing <= domain.maximum) maximum = Math.max(maximum, evaluate(spacing));
    const start = 1.3 / waveNumber;
    if (start >= domain.minimum && start <= domain.maximum) maximum = Math.max(maximum, evaluate(start));
  }
  const bound = directionNorm * Math.abs(amplitude) * maximum * (1 + 2 ** -20);
  if (!Number.isFinite(bound)) throw new RangeError("Lake wave derivative bound overflowed");
  return bound;
}

export function createLakeWaveProjection(condition: LakeVisualCondition, quality: LakeWaterQuality): LakeWaveProjectionInputs {
  const spectrum = createLakeWaveSpectrum(condition.windNorthMetersPerSecond, condition.windEastMetersPerSecond, condition.fetchMeters, 18);
  const waves = selectLakeWaveComponentsForQuality(spectrum, quality);
  const bands = waves.map((wave) => Math.floor(spectrum.components.indexOf(wave) / 3));
  const directionCounts = new Array<number>(6).fill(0);
  for (const band of bands) {
    if (band >= 0 && band < directionCounts.length) directionCounts[band] = (directionCounts[band] ?? 0) + 1;
  }
  const waveKAmplitude = Array.from({ length: 24 }, (_waveSlot, index): LakeWaveUpload => {
    const wave = waves[index];
    if (wave === undefined) return Object.freeze([0, 0, 0, 0]);
    const directionCount = directionCounts[bands[index] ?? -1] ?? 0;
    const directionScale = directionCount > 0 ? Math.sqrt(3 / directionCount) : 1;
    const shortWeight = Math.max(0, Math.min(1, (wave.waveNumberRadiansPerMeter - 1.5) / 4.5));
    const smoothWeight = shortWeight * shortWeight * (3 - 2 * shortWeight);
    const visualScale = 0.65 + 1.35 * smoothWeight;
    return Object.freeze([Math.fround(wave.directionEast), Math.fround(-wave.directionNorth),
      Math.fround(wave.waveNumberRadiansPerMeter), Math.fround(wave.amplitudeMeters * directionScale * visualScale)]);
  });
  const waveOmegaPhase = Array.from({ length: 24 }, (_waveSlot, index): LakeWaveUpload => {
    const wave = waves[index];
    return Object.freeze(wave === undefined ? [0, 0, 0, 0]
      : [Math.fround(wave.angularFrequencyRadiansPerSecond), Math.fround(wave.phaseRadians), 0, 0]);
  });
  const spacing = lakeMeshSpacingDomain(lakeWaterQualityProfile(quality).meshSegments);
  const horizontalDerivativeBound = waveKAmplitude.slice(0, waves.length)
    .reduce((sum, wave) => sum + lakeWaveDisplacementDerivativeBound(wave, spacing), 0) * (1 + 2 ** -20);
  const choppiness = horizontalDerivativeBound === 0 ? 4.5
    : Math.fround(Math.min(4.5, 0.56 / horizontalDerivativeBound * (1 - FLOAT32_RESERVE)));
  return Object.freeze({ waveKAmplitude: Object.freeze(waveKAmplitude), waveOmegaPhase: Object.freeze(waveOmegaPhase),
    waveCount: waves.length, visualWaveHeight: spectrum.significantWaveHeightMeters * 8,
    spacing, horizontalDerivativeBound, choppiness });
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
      result.slopeEnergy += waveNumber * wave.w;
    }
  }
  return result;
}
`;
