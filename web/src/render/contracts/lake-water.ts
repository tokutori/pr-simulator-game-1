/** Engine-neutral, bounded wave spectrum for render-only lake motion. */
export interface LakeWaveComponent {
  readonly directionNorth: number;
  readonly directionEast: number;
  readonly waveNumberRadiansPerMeter: number;
  readonly amplitudeMeters: number;
  readonly angularFrequencyRadiansPerSecond: number;
  readonly phaseRadians: number;
}

export interface LakeWaveSpectrum {
  readonly significantWaveHeightMeters: number;
  readonly components: readonly LakeWaveComponent[];
}

/** Stable, render-only sea state for one selected synthetic weather scenario. */
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

export type LakeWaterQuality = "low" | "medium" | "high";

export interface LakeWaterQualityProfile {
  readonly componentCount: number;
  readonly meshSegments: number;
}

const lowLakeWaveIndices = Array.from({ length: 6 }, (_, bandIndex) => bandIndex * 3 + bandIndex % 3);
const mediumLakeWaveIndices = [
  ...lowLakeWaveIndices,
  ...Array.from({ length: 4 }, (_, index) => {
    const bandIndex = index + 1;
    return bandIndex * 3 + (bandIndex + 1) % 3;
  })
].sort((a, b) => a - b);
const lakeWaveQualityIndices: Readonly<Record<LakeWaterQuality, readonly number[]>> = Object.freeze({
  low: Object.freeze(lowLakeWaveIndices),
  medium: Object.freeze(mediumLakeWaveIndices),
  high: Object.freeze(Array.from({ length: 18 }, (_, index) => index))
});

export function lakeWaterQualityProfile(quality: LakeWaterQuality): LakeWaterQualityProfile {
  switch (quality) {
    case "low": return Object.freeze({ componentCount: 6, meshSegments: 96 });
    case "medium": return Object.freeze({ componentCount: 10, meshSegments: 144 });
    case "high": return Object.freeze({ componentCount: 18, meshSegments: 224 });
  }
}

export function selectLakeWaveComponentsForQuality(
  spectrum: LakeWaveSpectrum,
  quality: LakeWaterQuality
): readonly LakeWaveComponent[] {
  const componentCount = lakeWaterQualityProfile(quality).componentCount;
  if (spectrum.components.length !== 18) {
    return Object.freeze(spectrum.components.slice(0, componentCount));
  }
  const indices = lakeWaveQualityIndices[quality].slice(0, componentCount);
  return Object.freeze(indices.flatMap((index) => {
    const component = spectrum.components[index];
    return component === undefined ? [] : [component];
  }));
}

/**
 * Creates a deterministic, finite-fetch wind-wave approximation from a stable
 * render-only condition. The caller provides fetch independently of mesh size;
 * scenario/environment data should eventually replace the synthetic values.
 * It is deliberately not a physical forecast or a Lake Biwa observation.
 */
export function createLakeWaveSpectrum(
  windNorthMetersPerSecond: number,
  windEastMetersPerSecond: number,
  fetchMeters = 600,
  componentCount = 12
): LakeWaveSpectrum {
  for (const value of [windNorthMetersPerSecond, windEastMetersPerSecond, fetchMeters]) {
    if (!Number.isFinite(value)) throw new RangeError("Lake wave inputs must be finite");
  }
  if (fetchMeters <= 0 || fetchMeters > 50_000 || !Number.isInteger(componentCount) || componentCount < 4 || componentCount > 24) {
    throw new RangeError("Lake fetch and component count are outside the supported range");
  }

  const windSpeed = Math.hypot(windNorthMetersPerSecond, windEastMetersPerSecond);
  if (!Number.isFinite(windSpeed) || windSpeed > 60) throw new RangeError("Lake wave wind speed must not exceed 60 m/s");
  const significantWaveHeightMeters = finiteFetchWaveHeight(windSpeed, fetchMeters);
  if (windSpeed < 0.05 || significantWaveHeightMeters < 0.001) {
    return Object.freeze({ significantWaveHeightMeters, components: Object.freeze([]) });
  }

  const meanNorth = windNorthMetersPerSecond / windSpeed;
  const meanEast = windEastMetersPerSecond / windSpeed;
  const peakPeriod = Math.max(0.8, 7.54 * Math.tanh(0.077 * Math.pow(9.80665 * fetchMeters / (windSpeed * windSpeed), 0.25)) * windSpeed / 9.80665);
  const peakOmega = 2 * Math.PI / peakPeriod;
  const components: LakeWaveComponent[] = [];
  const directionsPerBand = componentCount >= 15 ? 3 : 2;
  const bandCount = Math.ceil(componentCount / directionsPerBand);

  for (let index = 0; index < componentCount; index++) {
    const bandIndex = Math.floor(index / directionsPerBand);
    const directionIndex = index % directionsPerBand;
    const directionsInBand = Math.min(directionsPerBand, componentCount - bandIndex * directionsPerBand);
    const band = bandIndex / Math.max(1, bandCount - 1);
    const frequencyJitter = (directionIndex - (directionsInBand - 1) / 2) * 0.035;
    const omega = peakOmega * Math.exp((band - 0.42) * 1.5 + frequencyJitter);
    const ratio = peakOmega / omega;
    const sigma = omega <= peakOmega ? 0.07 : 0.09;
    const peakEnhancement = Math.exp(-Math.pow(omega - peakOmega, 2) / (2 * sigma * sigma * peakOmega * peakOmega));
    const jonswapShape = Math.pow(omega, -5) * Math.exp(-1.25 * Math.pow(ratio, 4)) * Math.pow(3.3, peakEnhancement);
    // Sample each frequency band across several distinct directions. The
    // small deterministic jitter avoids exact angular stacks between bands.
    const directionalSpread = 0.16 + 0.12 * band;
    const directionalOffset = (directionIndex - (directionsInBand - 1) / 2) * directionalSpread
      + (hashUnit(index + 701) - 0.5) * 0.09;
    const directionNorth = meanNorth * Math.cos(directionalOffset) - meanEast * Math.sin(directionalOffset);
    const directionEast = meanEast * Math.cos(directionalOffset) + meanNorth * Math.sin(directionalOffset);
    const logarithmicBandWidth = omega * (Math.exp(1.5 / Math.max(1, bandCount - 1)) - 1);
    const weight = Math.sqrt(jonswapShape * logarithmicBandWidth / directionsInBand);
    components.push({
      directionNorth,
      directionEast,
      waveNumberRadiansPerMeter: omega * omega / 9.80665,
      amplitudeMeters: weight,
      angularFrequencyRadiansPerSecond: omega,
      phaseRadians: hashUnit(index + 101) * Math.PI * 2
    });
  }

  // Normalize component variance to the finite-fetch significant height and
  // cap summed Gerstner steepness to keep the surface single-valued.
  const provisionalScale = significantWaveHeightMeters / (4 * Math.sqrt(components.reduce((sum, wave) => sum + wave.amplitudeMeters ** 2, 0)));
  const steepness = components.reduce((sum, wave) => sum + wave.waveNumberRadiansPerMeter * wave.amplitudeMeters * provisionalScale, 0);
  const steepnessScale = steepness > 0.52 ? 0.52 / steepness : 1;
  const scale = provisionalScale * steepnessScale;
  const normalized = components.map((wave) => Object.freeze({
    ...wave,
    amplitudeMeters: wave.amplitudeMeters * scale
  }));
  return Object.freeze({ significantWaveHeightMeters, components: Object.freeze(normalized) });
}

function finiteFetchWaveHeight(windSpeed: number, fetchMeters: number): number {
  if (windSpeed < 0.05) return 0;
  const dimensionlessFetch = 9.80665 * fetchMeters / (windSpeed * windSpeed);
  return 0.283 * windSpeed * windSpeed / 9.80665 * Math.tanh(0.0125 * Math.pow(dimensionlessFetch, 0.42));
}

function hashUnit(value: number): number {
  const sine = Math.sin(value * 127.1 + 311.7) * 43758.5453123;
  return sine - Math.floor(sine);
}
