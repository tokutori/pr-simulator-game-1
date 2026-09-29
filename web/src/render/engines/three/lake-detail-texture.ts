import { DataTexture, LinearFilter, LinearMipmapLinearFilter, RepeatWrapping } from "three";

export interface LakeDetailLayer {
  readonly texture: DataTexture;
  readonly extentMeters: number;
}

/** Builds a periodic normal field from finite-length height wavelets once. */
export function createLakeDetailLayer(
  extentMeters: number,
  waveletCount: number,
  seed: number,
  directionX: number,
  directionZ: number
): LakeDetailLayer {
  const size = 512;
  const texelMeters = extentMeters / size;
  const featureScale = extentMeters / 64;
  const heightField = new Float32Array(size * size);
  const slopeX = new Float32Array(size * size);
  const slopeZ = new Float32Array(size * size);
  let randomState = seed >>> 0;
  const random = (): number => {
    randomState = (Math.imul(randomState, 1664525) + 1013904223) >>> 0;
    return randomState / 4294967296;
  };

  for (let wavelet = 0; wavelet < waveletCount; wavelet++) {
    const centerX = random() * size;
    const centerZ = random() * size;
    const angle = (random() - 0.5) * 1.8;
    const travelX = directionX * Math.cos(angle) - directionZ * Math.sin(angle);
    const travelZ = directionZ * Math.cos(angle) + directionX * Math.sin(angle);
    const crestX = -travelZ;
    const crestZ = travelX;
    const crestLength = (0.3 + random() * 0.35) * featureScale;
    const envelopeWidth = (0.4 + random() * 0.5) * featureScale;
    const height = (0.045 + random() * 0.085) * Math.min(1.35, Math.sqrt(featureScale));
    const radius = Math.ceil(4 * Math.max(crestLength, envelopeWidth) / texelMeters);

    for (let offsetZ = -radius; offsetZ <= radius; offsetZ++) {
      for (let offsetX = -radius; offsetX <= radius; offsetX++) {
        const deltaX = offsetX * texelMeters;
        const deltaZ = offsetZ * texelMeters;
        const alongCrest = deltaX * crestX + deltaZ * crestZ;
        const alongTravel = deltaX * travelX + deltaZ * travelZ;
        const normalizedRadius = (alongCrest / crestLength) ** 2 + (alongTravel / envelopeWidth) ** 2;
        if (normalizedRadius > 16) continue;
        const envelope = height * Math.exp(-0.5 * normalizedRadius);
        // A compact mound with its own surrounding trough replaces the long
        // sinusoidal carrier. Its height integral vanishes over the plane.
        const derivative = envelope * (0.5 * normalizedRadius - 2);
        const crestDerivative = derivative * alongCrest / (crestLength * crestLength);
        const travelDerivative = derivative * alongTravel / (envelopeWidth * envelopeWidth);
        const texelX = ((Math.floor(centerX) + offsetX) % size + size) % size;
        const texelZ = ((Math.floor(centerZ) + offsetZ) % size + size) % size;
        const index = texelZ * size + texelX;
        heightField[index] = (heightField[index] ?? 0) + envelope * (1 - 0.5 * normalizedRadius);
        slopeX[index] = (slopeX[index] ?? 0) + crestDerivative * crestX + travelDerivative * travelX;
        slopeZ[index] = (slopeZ[index] ?? 0) + crestDerivative * crestZ + travelDerivative * travelZ;
      }
    }
  }

  const pixels = new Uint8Array(size * size * 4);
  for (let index = 0; index < slopeX.length; index++) {
    const x = Math.max(-1, Math.min(1, slopeX[index] ?? 0));
    const z = Math.max(-1, Math.min(1, slopeZ[index] ?? 0));
    pixels[index * 4] = Math.round(128 + x * 127);
    pixels[index * 4 + 1] = Math.round(128 + z * 127);
    pixels[index * 4 + 2] = Math.round(Math.min(1, (x * x + z * z) * 2.5) * 255);
    pixels[index * 4 + 3] = Math.max(0, Math.min(255,
      Math.round(128 + Math.max(-0.5, Math.min(0.5, heightField[index] ?? 0)) * 255)
    ));
  }

  const texture = new DataTexture(pixels, size, size);
  texture.wrapS = RepeatWrapping;
  texture.wrapT = RepeatWrapping;
  texture.magFilter = LinearFilter;
  texture.minFilter = LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.anisotropy = 8;
  texture.needsUpdate = true;
  return { texture, extentMeters };
}
