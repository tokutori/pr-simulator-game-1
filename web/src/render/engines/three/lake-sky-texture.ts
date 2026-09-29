import {
  ClampToEdgeWrapping,
  DataTexture,
  EquirectangularReflectionMapping,
  LinearFilter,
  LinearMipmapLinearFilter,
  RepeatWrapping,
  RGBAFormat,
  SRGBColorSpace,
  UnsignedByteType
} from "three";

function smoothstep(value: number): number {
  const t = Math.max(0, Math.min(1, value));
  return t * t * (3 - 2 * t);
}

function lattice(x: number, y: number, period: number): number {
  const wrappedX = ((x % period) + period) % period;
  let bits = Math.imul(wrappedX, 1597334677) ^ Math.imul(y, 3812015801);
  bits = Math.imul(bits ^ (bits >>> 16), 2246822507);
  return ((bits ^ (bits >>> 13)) >>> 0) / 4294967295;
}

function cloudNoise(u: number, v: number, columns: number, rows: number): number {
  const x = u * columns;
  const y = v * rows;
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const tx = smoothstep(x - x0);
  const ty = smoothstep(y - y0);
  const low = lattice(x0, y0, columns) * (1 - tx) + lattice(x0 + 1, y0, columns) * tx;
  const high = lattice(x0, y0 + 1, columns) * (1 - tx) + lattice(x0 + 1, y0 + 1, columns) * tx;
  return low * (1 - ty) + high * ty;
}

/** Shared, subdued sky radiance for the background and the lake reflection. */
export function createLakeSkyTexture(): DataTexture {
  const width = 512;
  const height = 256;
  const pixels = new Uint8Array(width * height * 4);
  const sun = [0.42, 0.82, 0.38] as const;
  const sunLength = Math.hypot(...sun);
  for (let row = 0; row < height; row++) {
    const v = (row + 0.5) / height;
    const latitude = (v - 0.5) * Math.PI;
    const elevation = Math.max(0, Math.sin(latitude));
    const gradient = Math.pow(elevation, 0.72);
    const horizonHaze = (1 - smoothstep(elevation / 0.12)) * 3;
    for (let column = 0; column < width; column++) {
      const u = (column + 0.5) / width;
      const longitude = (u - 0.5) * Math.PI * 2;
      let cloud = 0;
      if (elevation > 0) {
        const cloudDrift = cloudNoise(u, v, 4, 7) - 0.5;
        const cloudWarp = cloudDrift * 0.012;
        const lowCloudWeight = smoothstep(elevation / 0.025) *
          (1 - smoothstep((elevation - 0.22 - cloudDrift * 0.07) / 0.16));
        const cloudField = lowCloudWeight * (
          cloudNoise(u, v + cloudWarp, 10, 24) * 0.65 +
          cloudNoise(u, v + cloudWarp, 25, 52) * 0.35
        );
        // Weighted coverage over the visible sky is about 5% at this threshold.
        cloud = smoothstep((cloudField - 0.62) / 0.04) * 18;
      }
      const rayX = Math.cos(latitude) * Math.cos(longitude);
      const rayY = Math.sin(latitude);
      const rayZ = Math.cos(latitude) * Math.sin(longitude);
      const sunAlignment = Math.max(0, (rayX * sun[0] + rayY * sun[1] + rayZ * sun[2]) / sunLength);
      const sunGlow = Math.pow(sunAlignment, 120) * 14;
      const offset = (row * width + column) * 4;
      pixels[offset] = Math.round(Math.max(0, Math.min(255, 168 - 42 * gradient + cloud + horizonHaze + sunGlow)));
      pixels[offset + 1] = Math.round(Math.max(0, Math.min(255, 191 - 20 * gradient + cloud + horizonHaze + sunGlow * 0.8)));
      pixels[offset + 2] = Math.round(Math.max(0, Math.min(255, 197 - 7 * gradient + cloud + horizonHaze + sunGlow * 0.55)));
      pixels[offset + 3] = 255;
    }
  }

  const texture = new DataTexture(pixels, width, height, RGBAFormat, UnsignedByteType);
  texture.mapping = EquirectangularReflectionMapping;
  texture.colorSpace = SRGBColorSpace;
  texture.wrapS = RepeatWrapping;
  texture.wrapT = ClampToEdgeWrapping;
  texture.magFilter = LinearFilter;
  texture.minFilter = LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.needsUpdate = true;
  return texture;
}
