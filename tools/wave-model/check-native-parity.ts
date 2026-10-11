import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createLakeDetailLayer } from "../../web/src/render/engines/three/lake-detail-texture.js";

interface DetailConfiguration {
  readonly extentMeters: number;
  readonly waveletCount: number;
  readonly seed: number;
  readonly directionX: number;
  readonly directionZ: number;
  readonly broaderWavelets?: Readonly<{ count: number; featureScaleMeters: number; heightScale: number }>;
}

const output = process.argv[2];
if (output === undefined) throw new Error("Provide the native parity-output directory");
const reference: unknown = JSON.parse(readFileSync(new URL("../../crates/birdman-game-session/src/lake_water/web-reference.json", import.meta.url), "utf8"));
if (typeof reference !== "object" || reference === null || !("details" in reference) || !Array.isArray(reference.details)) throw new Error("Invalid trusted Web parity fixture");
for (const [index, detail] of reference.details.entries()) {
  const configuration = (detail as { configuration: DetailConfiguration }).configuration;
  const layer = createLakeDetailLayer(configuration.extentMeters, configuration.waveletCount, configuration.seed,
    configuration.directionX, configuration.directionZ, configuration.broaderWavelets);
  try {
    const expected = layer.texture.image.data;
    if (!(expected instanceof Uint8Array)) throw new Error("Web detail texture must expose Uint8Array data");
    const actual = readFileSync(join(output, `detail-${index}.rgba`));
    if (actual.length !== expected.length) throw new Error(`Detail ${index} byte length differs`);
    let changedBytes = 0;
    let maximumByteDifference = 0;
    for (let offset = 0; offset < expected.length; offset++) {
      const difference = Math.abs((actual[offset] ?? -1) - (expected[offset] ?? -1));
      if (difference > 0) changedBytes++;
      maximumByteDifference = Math.max(maximumByteDifference, difference);
    }
    if (maximumByteDifference > 1) throw new Error(`Detail ${index} exceeds one-byte quantization parity: ${maximumByteDifference}`);
    console.log(JSON.stringify({ index, seed: configuration.seed, extentMeters: configuration.extentMeters,
      changedBytes, maximumByteDifference,
      nativeSha256: createHash("sha256").update(actual).digest("hex"), webSha256: createHash("sha256").update(expected).digest("hex") }));
  } finally { layer.texture.dispose(); }
}
