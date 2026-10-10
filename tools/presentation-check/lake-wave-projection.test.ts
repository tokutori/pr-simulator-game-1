import { describe, expect, it } from "vitest";
import { lakeWaterQualityProfile } from "../../web/src/render/contracts/lake-water.js";
import type { LakeVisualCondition } from "../../web/src/render/contracts/lake-water.js";
import { createLakeGeometry } from "../../web/src/render/engines/three/lake-water-geometry.js";
import { createLakeWaveProjection, lakeGridSpacingGradient, lakeMeshSpacingDomain, lakeWaveDisplacementDerivativeBound,
  lakeWaveVisibility } from "../../web/src/render/engines/three/lake-wave-projection.js";
import type { LakeMeshSpacingDomain, LakeWaveProjectionInputs, LakeWaveUpload } from "../../web/src/render/engines/three/lake-wave-projection.js";

const representativeWeather: readonly LakeVisualCondition[] = [
  [0.54, 1.07], [0, 1.32], [-0.35, 1.42], [-0.65, 1.54], [-0.93, 1.67], [-1.767766953, 1.767766953]
].map(([north = 0, east = 0], index) => ({ windNorthMetersPerSecond: north, windEastMetersPerSecond: east,
  fetchMeters: 600, detailAmplitudeScale: 1, patternSeed: index }));

describe("lake wave displacement derivative bound", () => {
  it.each(["low", "medium", "high"] as const)("bounds canonical Float32 inputs and compensated waves in %s", (quality) => {
    for (const condition of representativeWeather) {
      const inputs = createLakeWaveProjection(condition, quality);
      expect(inputs.waveCount).toBe(lakeWaterQualityProfile(quality).componentCount);
      expect(inputs.waveKAmplitude).toHaveLength(24);
      expect(inputs.waveOmegaPhase).toHaveLength(24);
      expect([...inputs.waveKAmplitude.flat(), ...inputs.waveOmegaPhase.flat(), inputs.choppiness,
        inputs.spacing.minimum, inputs.spacing.maximum, inputs.spacing.gradientMagnitude]
        .every((value) => Number.isFinite(value) && value === Math.fround(value))).toBe(true);
      expect(inputs.choppiness).toBeGreaterThan(0);
      expect(inputs.choppiness).toBeLessThanOrEqual(4.5);
      expect(inputs.choppiness * inputs.horizontalDerivativeBound).toBeLessThan(0.56);
      for (const wave of inputs.waveKAmplitude.slice(0, inputs.waveCount)) {
        const bound = lakeWaveDisplacementDerivativeBound(wave, inputs.spacing);
        let sampled = 0;
        for (let index = 0; index <= 64; index++) {
          const spacing = inputs.spacing.minimum + (inputs.spacing.maximum - inputs.spacing.minimum) * index / 64;
          sampled = Math.max(sampled, referenceDerivativeBound(wave, inputs.spacing, spacing));
        }
        expect(sampled).toBeLessThanOrEqual(bound);
        if (wave[2] * inputs.spacing.minimum >= 2.5) expect(bound).toBe(0);
      }
    }
  });

  it("evaluates the transition stationary point and endpoints on the actual visibility interval", () => {
    const wave: LakeWaveUpload = [Math.fround(0.8), Math.fround(0.60000004), Math.fround(3), Math.fround(0.1)];
    for (const domain of [
      { minimum: 0.1, maximum: 0.82, gradientMagnitude: 0.15 },
      { minimum: 0.45, maximum: 0.62, gradientMagnitude: 0.15 },
      { minimum: 0.65, maximum: 0.81, gradientMagnitude: 0.15 },
      { minimum: 0, maximum: 0.2, gradientMagnitude: 0.15 }
    ]) {
      const bound = lakeWaveDisplacementDerivativeBound(wave, domain);
      let sampled = 0;
      for (let index = 0; index <= 4096; index++) {
        sampled = Math.max(sampled, referenceDerivativeBound(wave, domain,
          domain.minimum + (domain.maximum - domain.minimum) * index / 4096));
      }
      expect(bound).toBeGreaterThanOrEqual(sampled);
      expect(bound / sampled).toBeLessThan(1.00001);
    }
  });

  it("includes the uploaded direction norm in both modulation and outer horizontal displacement", () => {
    const domain = { minimum: 0, maximum: 0, gradientMagnitude: 0 };
    const unit = lakeWaveDisplacementDerivativeBound([1, 0, 2, 0.1], domain);
    expect(lakeWaveDisplacementDerivativeBound([2, 0, 2, 0.1], domain)).toBeCloseTo(4 * unit, 12);
    expect(lakeWaveDisplacementDerivativeBound([0, 0, 2, 0.1], domain)).toBe(0);
    expect(lakeWaveDisplacementDerivativeBound([1, 0, 10, 1], { ...domain, minimum: 0.25, maximum: 2 })).toBe(0);
    expect(() => lakeWaveDisplacementDerivativeBound([1, 0, Number.NaN, 1], domain)).toThrow(RangeError);
    expect(() => lakeMeshSpacingDomain(0)).toThrow(RangeError);
  });

  it("handles an empty spectrum without a division or non-finite projection input", () => {
    const inputs = createLakeWaveProjection({ windNorthMetersPerSecond: 0, windEastMetersPerSecond: 0,
      fetchMeters: 600, detailAmplitudeScale: 1, patternSeed: 0 }, "low");
    expect(inputs.waveCount).toBe(0);
    expect(inputs.horizontalDerivativeBound).toBe(0);
    expect(inputs.choppiness).toBe(4.5);
  });

  it("keeps max-axis spacing gradients bounded at center and ties", () => {
    const magnitude = lakeMeshSpacingDomain(96).gradientMagnitude;
    expect(lakeGridSpacingGradient(0, 0, magnitude)).toEqual([0, 0]);
    expect(lakeGridSpacingGradient(-2, 1, magnitude)).toEqual([-magnitude, 0]);
    expect(lakeGridSpacingGradient(1, -2, magnitude)).toEqual([0, -magnitude]);
    expect(lakeGridSpacingGradient(-2, 2, magnitude)).toEqual([-magnitude / 2, magnitude / 2]);
    for (const point of [[0, 0], [2, 1], [1, 2], [2, -2]] as const) {
      expect(Math.hypot(...lakeGridSpacingGradient(point[0], point[1], magnitude))).toBeLessThanOrEqual(magnitude);
    }
    expect(lakeWaveVisibility(2, 1.3 / 2)).toEqual({ value: 1, derivative: -0 });
    expect(lakeWaveVisibility(2, 2.5 / 2)).toEqual({ value: 0, derivative: -0 });
    expect(lakeWaveVisibility(2, 1.9 / 2).derivative).toBeCloseTo(-2.5, 14);
  });

  it.each(["low", "medium", "high"] as const)("keeps actual %s mesh triangles finite and non-inverted in the CPU reference", (quality) => {
    const condition = representativeWeather[5];
    if (condition === undefined) throw new Error("Missing registered weather fixture");
    const inputs = createLakeWaveProjection(condition, quality);
    const geometry = createLakeGeometry(lakeWaterQualityProfile(quality).meshSegments);
    try {
      const positions = geometry.getAttribute("position");
      const grid = geometry.getAttribute("aGridSpacing");
      const indices = geometry.index;
      if (indices === null) throw new Error("Missing actual lake triangle indices");
      const projected = new Float32Array(positions.count * 3);
      let minimumSpacing = Number.POSITIVE_INFINITY;
      let maximumSpacing = 0;
      for (let index = 0; index < positions.count; index++) {
        const spacing = grid.getX(index);
        minimumSpacing = Math.min(minimumSpacing, spacing);
        maximumSpacing = Math.max(maximumSpacing, spacing);
        projected.set(referenceProjection(inputs, positions.getX(index), -positions.getY(index), spacing, 12.345).position, index * 3);
      }
      expect(minimumSpacing).toBe(inputs.spacing.minimum);
      expect(maximumSpacing).toBe(inputs.spacing.maximum);
      expect(projected.every(Number.isFinite)).toBe(true);
      let minimumAreaRatio = Number.POSITIVE_INFINITY;
      for (let index = 0; index < indices.count; index += 3) {
        const first = indices.getX(index);
        const second = indices.getX(index + 1);
        const third = indices.getX(index + 2);
        const baseArea = (positions.getX(second) - positions.getX(first)) * (-positions.getY(third) + positions.getY(first))
          - (-positions.getY(second) + positions.getY(first)) * (positions.getX(third) - positions.getX(first));
        const coordinate = (vertex: number, axis: number): number => {
          const value = projected[vertex * 3 + axis];
          if (value === undefined) throw new Error("Missing projected mesh vertex");
          return value;
        };
        const area = (coordinate(second, 0) - coordinate(first, 0)) * (coordinate(third, 2) - coordinate(first, 2))
          - (coordinate(second, 2) - coordinate(first, 2)) * (coordinate(third, 0) - coordinate(first, 0));
        minimumAreaRatio = Math.min(minimumAreaRatio, area / baseArea);
      }
      expect(Number.isFinite(minimumAreaRatio)).toBe(true);
      expect(minimumAreaRatio).toBeGreaterThan(0);
    } finally { geometry.dispose(); }
  });

  it("includes the LOD amplitude derivative in normals at transitions and preserves translation-only coordinates", () => {
    const condition = representativeWeather[5];
    if (condition === undefined) throw new Error("Missing registered weather fixture");
    const inputs = createLakeWaveProjection(condition, "high");
    const wave = inputs.waveKAmplitude.find((candidate) => candidate[2] > 0 && candidate[2] * inputs.spacing.minimum < 1.3);
    if (wave === undefined) throw new Error("Missing visible geometric wave");
    const transitionX = (1.9 / wave[2] - inputs.spacing.minimum) / inputs.spacing.gradientMagnitude;
    for (const point of [[transitionX, 0.37], [0, -transitionX], [3, 3], [0, 0]] as const) {
      const spacingAt = (localX: number, localZ: number): number => inputs.spacing.minimum
        + inputs.spacing.gradientMagnitude * Math.max(Math.abs(localX), Math.abs(localZ));
      const project = (localX: number, localZ: number) => referenceProjection(inputs, localX, localZ, spacingAt(localX, localZ), 9.5, [125, -70]);
      const sample = project(point[0], point[1]);
      const step = 1e-5;
      const lowX = project(point[0] - step, point[1]);
      const highX = project(point[0] + step, point[1]);
      const lowZ = project(point[0], point[1] - step);
      const highZ = project(point[0], point[1] + step);
      const expectedX = [sample.jacobian[0], sample.heightGradient[0], sample.jacobian[2]] as const;
      const expectedZ = [sample.jacobian[1], sample.heightGradient[1], sample.jacobian[3]] as const;
      for (const axis of [0, 1, 2] as const) {
        expect(Math.abs((highX.position[axis] - lowX.position[axis]) / (2 * step) - expectedX[axis])).toBeLessThan(2e-5);
        expect(Math.abs((highZ.position[axis] - lowZ.position[axis]) / (2 * step) - expectedZ[axis])).toBeLessThan(2e-5);
      }
      const determinant = sample.jacobian[0] * sample.jacobian[3] - sample.jacobian[1] * sample.jacobian[2];
      expect(determinant).toBeGreaterThan((1 - inputs.choppiness * inputs.horizontalDerivativeBound) ** 2);
      expect(sample.normal.every(Number.isFinite)).toBe(true);
      expect(sample.normal[1]).toBeGreaterThan(0);
    }
  });
});

function referenceDerivativeBound(wave: LakeWaveUpload, domain: LakeMeshSpacingDomain, spacing: number): number {
  const directionNorm = Math.hypot(wave[0], wave[1]);
  const visibility = lakeWaveVisibility(wave[2], spacing);
  return directionNorm * Math.abs(wave[3]) * (visibility.value * directionNorm
    * (1.5 * wave[2] + 48.86163958897689 * Math.max(0.11, 0.055 * wave[2]))
    + 1.5 * Math.abs(visibility.derivative) * domain.gradientMagnitude);
}

function referenceNoise(pointX: number, pointZ: number): readonly [number, number, number] {
  const cellX = Math.floor(pointX);
  const cellZ = Math.floor(pointZ);
  const fractionX = pointX - cellX;
  const fractionZ = pointZ - cellZ;
  const blendX = fractionX * fractionX * (3 - 2 * fractionX);
  const blendZ = fractionZ * fractionZ * (3 - 2 * fractionZ);
  const hash = (offsetX: number, offsetZ: number): number => {
    const value = Math.sin((cellX + offsetX) * 127.1 + (cellZ + offsetZ) * 311.7) * 43758.5453;
    return value - Math.floor(value);
  };
  const lowerLeft = hash(0, 0);
  const lowerRight = hash(1, 0);
  const upperLeft = hash(0, 1);
  const upperRight = hash(1, 1);
  const lower = lowerLeft + (lowerRight - lowerLeft) * blendX;
  const upper = upperLeft + (upperRight - upperLeft) * blendX;
  return [2 * (lower + (upper - lower) * blendZ) - 1,
    12 * fractionX * (1 - fractionX) * ((lowerRight - lowerLeft) * (1 - blendZ) + (upperRight - upperLeft) * blendZ),
    12 * fractionZ * (1 - fractionZ) * ((upperLeft - lowerLeft) * (1 - blendX) + (upperRight - lowerRight) * blendX)];
}

function referenceProjection(inputs: LakeWaveProjectionInputs, localX: number, localZ: number, spacing: number,
  seconds: number, translation: readonly [number, number] = [0, 0]): {
    readonly position: readonly [number, number, number]; readonly jacobian: readonly [number, number, number, number];
    readonly heightGradient: readonly [number, number]; readonly normal: readonly [number, number, number];
  } {
  const worldX = localX + translation[0];
  const worldZ = localZ + translation[1];
  const spacingGradient = lakeGridSpacingGradient(localX, localZ, inputs.spacing.gradientMagnitude);
  const position: [number, number, number] = [localX, 0, localZ];
  const jacobian: [number, number, number, number] = [1, 0, 0, 1];
  const heightGradient: [number, number] = [0, 0];
  for (let index = 0; index < inputs.waveCount; index++) {
    const wave = inputs.waveKAmplitude[index];
    const timing = inputs.waveOmegaPhase[index];
    if (wave === undefined || timing === undefined) throw new Error("Missing reference wave upload");
    if (wave[2] * spacing >= 2.5) continue;
    const [directionX, directionZ, waveNumber, baseAmplitude] = wave;
    const frequency = Math.max(0.11, waveNumber * 0.055);
    const along = (worldX * directionX + worldZ * directionZ) * frequency + timing[1] * 2.3;
    const across = (-worldX * directionZ + worldZ * directionX) * frequency + timing[1] * 1.7;
    const broad = referenceNoise(across, along);
    const fine = referenceNoise(across * 1.7 + along * 0.37 + 13.7, along * 1.9 - across * 0.24 - 8.2);
    const broadGradient = [frequency * (-directionZ * broad[1] + directionX * broad[2]),
      frequency * (directionX * broad[1] + directionZ * broad[2])] as const;
    const fineGradient = [frequency * (fine[1] * (-directionZ * 1.7 + directionX * 0.37)
      + fine[2] * (directionZ * 0.24 + directionX * 1.9)),
    frequency * (fine[1] * (directionX * 1.7 + directionZ * 0.37)
      + fine[2] * (-directionX * 0.24 + directionZ * 1.9))] as const;
    const packet = 1 + 0.38 * broad[0] + 0.12 * fine[0];
    const visibility = lakeWaveVisibility(waveNumber, spacing);
    const amplitude = baseAmplitude * visibility.value * packet;
    const amplitudeGradient = [0, 1].map((axis) => baseAmplitude * (visibility.value
      * (0.38 * (broadGradient[axis] ?? 0) + 0.12 * (fineGradient[axis] ?? 0))
      + packet * visibility.derivative * (spacingGradient[axis] ?? 0)));
    const phase = waveNumber * (worldX * directionX + worldZ * directionZ) + 4.6 * broad[0] + 1.4 * fine[0]
      - timing[0] * seconds + timing[1];
    const phaseGradient = [waveNumber * directionX + 4.6 * broadGradient[0] + 1.4 * fineGradient[0],
      waveNumber * directionZ + 4.6 * broadGradient[1] + 1.4 * fineGradient[1]] as const;
    const phaseSine = Math.sin(phase);
    const phaseCosine = Math.cos(phase);
    const crestBasis = 0.5 + 0.5 * phaseSine;
    const heightBasis = 2 * crestBasis ** 3 - 0.625;
    position[0] += inputs.choppiness * amplitude * directionX * phaseCosine;
    position[1] += amplitude * heightBasis;
    position[2] += inputs.choppiness * amplitude * directionZ * phaseCosine;
    const horizontalGradient = [0, 1].map((axis) => inputs.choppiness * (phaseCosine * (amplitudeGradient[axis] ?? 0)
      - amplitude * phaseSine * (phaseGradient[axis] ?? 0)));
    jacobian[0] += directionX * (horizontalGradient[0] ?? 0);
    jacobian[1] += directionX * (horizontalGradient[1] ?? 0);
    jacobian[2] += directionZ * (horizontalGradient[0] ?? 0);
    jacobian[3] += directionZ * (horizontalGradient[1] ?? 0);
    heightGradient[0] += amplitude * 3 * crestBasis ** 2 * phaseCosine * phaseGradient[0] + heightBasis * (amplitudeGradient[0] ?? 0);
    heightGradient[1] += amplitude * 3 * crestBasis ** 2 * phaseCosine * phaseGradient[1] + heightBasis * (amplitudeGradient[1] ?? 0);
  }
  const normal: [number, number, number] = [heightGradient[1] * jacobian[2] - jacobian[3] * heightGradient[0],
    jacobian[3] * jacobian[0] - jacobian[1] * jacobian[2], jacobian[1] * heightGradient[0] - heightGradient[1] * jacobian[0]];
  const length = Math.hypot(...normal);
  return { position, jacobian, heightGradient, normal: [normal[0] / length, normal[1] / length, normal[2] / length] };
}
