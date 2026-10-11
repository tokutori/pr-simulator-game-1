import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createLakeWaveSpectrum } from "../../web/src/render/contracts/lake-water.js";
import type { LakeVisualCondition, LakeWaterQuality } from "../../web/src/render/contracts/lake-water.js";
import { createLakeWaveProjection } from "../../web/src/render/engines/three/lake-wave-projection.js";
import { createLakeDetailLayer } from "../../web/src/render/engines/three/lake-detail-texture.js";

const sourcePaths = [
  "web/src/render/contracts/lake-water.ts",
  "web/src/render/engines/three/lake-wave-projection.ts",
  "web/src/render/engines/three/lake-detail-texture.ts"
] as const;
const conditions: readonly LakeVisualCondition[] = [
  [0, 0, 600, 0], [0.54, 1.07, 600, 0], [-0.35, 1.42, 600, 2],
  [-1.767766953, 1.767766953, 600, 5], [4.2, -1.5, 12000, 37], [60, 0, 50000, 0]
].map(([north = 0, east = 0, fetch = 600, seed = 0]) => ({
  windNorthMetersPerSecond: north, windEastMetersPerSecond: east,
  fetchMeters: fetch, detailAmplitudeScale: 1, patternSeed: seed
}));
const qualityLevels: readonly LakeWaterQuality[] = ["low", "medium", "high"];
const spectra = conditions.map((condition) => ({
  condition,
  spectrum: createLakeWaveSpectrum(condition.windNorthMetersPerSecond, condition.windEastMetersPerSecond, condition.fetchMeters, 18),
  projections: qualityLevels.map((quality) => ({ quality, projection: createLakeWaveProjection(condition, quality) }))
}));
const detailInputs = [
  { extentMeters: 64, waveletCount: 48, seed: 123, directionX: 1, directionZ: 0 },
  { extentMeters: 64, waveletCount: 48, seed: 124, directionX: 1, directionZ: 0 },
  { extentMeters: 64, waveletCount: 3150, seed: 1717, directionX: 1.07 / Math.hypot(0.54, 1.07), directionZ: -0.54 / Math.hypot(0.54, 1.07) },
  { extentMeters: 288, waveletCount: 5400, seed: 2917, directionX: 1.07 / Math.hypot(0.54, 1.07), directionZ: -0.54 / Math.hypot(0.54, 1.07),
    broaderWavelets: { count: 900, featureScaleMeters: 4.5, heightScale: 0.5 } }
];
const rawOutput = process.argv[2];
if (rawOutput !== undefined) mkdirSync(rawOutput, { recursive: true });
const details = detailInputs.map((input, index) => {
  const layer = createLakeDetailLayer(input.extentMeters, input.waveletCount, input.seed, input.directionX, input.directionZ, "broaderWavelets" in input ? input.broaderWavelets : undefined);
  try {
    const data = layer.texture.image.data;
    if (!(data instanceof Uint8Array)) throw new Error("Web reference detail data must be Uint8Array");
    if (rawOutput !== undefined) writeFileSync(join(rawOutput, `detail-${index}.rgba`), data);
    const sums = [0, 0, 0, 0];
    const squaredSums = [0, 0, 0, 0];
    let activeSlopePixels = 0;
    let clippedSlopePixels = 0;
    for (let offset = 0; offset < data.length; offset += 4) {
      const slopeX = data[offset] ?? 0;
      const slopeZ = data[offset + 1] ?? 0;
      if ((slopeX - 128) ** 2 + (slopeZ - 128) ** 2 > 127 ** 2 * 0.01) activeSlopePixels++;
      if (slopeX <= 1 || slopeX >= 255 || slopeZ <= 1 || slopeZ >= 255) clippedSlopePixels++;
      for (const channel of [0, 1, 2, 3] as const) {
        const value = data[offset + channel] ?? 0;
        sums[channel] = (sums[channel] ?? 0) + value;
        squaredSums[channel] = (squaredSums[channel] ?? 0) + value * value;
      }
    }
    const indices = Array.from({ length: 48 }, (_, index) => Math.floor(index * (512 * 512 - 1) / 47));
    return {
      configuration: input,
      width: layer.texture.image.width,
      height: layer.texture.image.height,
      sha256: createHash("sha256").update(data).digest("hex"),
      sums, squaredSums, activeSlopePixels, clippedSlopePixels,
      samples: indices.map((index) => ({ index, rgba: Array.from(data.subarray(index * 4, index * 4 + 4)) }))
    };
  } finally { layer.texture.dispose(); }
});
const reference = {
  webRevision: "51b8678100da1520773d2247db52e0d830c34124",
  sources: sourcePaths.map((path) => ({ path, sha256: createHash("sha256").update(readFileSync(new URL(`../../${path}`, import.meta.url))).digest("hex") })),
  spectra, details
};
writeFileSync(new URL("../../crates/birdman-game-session/src/lake_water/web-reference.json", import.meta.url), `${JSON.stringify(reference, null, 2)}\n`, "utf8");
