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
  const width = 1024;
  const height = 512;
  const pixels = new Uint8Array(width * height * 4);
  const sun = [0.42, 0.82, 0.38] as const;
  const sunLength = Math.hypot(...sun);
  let randomState = 0x4f5a2c91;
  const random = (): number => {
    randomState = (Math.imul(randomState, 1664525) + 1013904223) >>> 0;
    return randomState / 4294967296;
  };
  // Finite patches avoid the panorama-wide smears produced by thresholding
  // large noise cells. Their occupied solid angle is approximately 5%.
  const clouds = Array.from({ length: 48 }, () => ({
    u: random(),
    v: 0.5 + (0.06 + random() * 0.28) / Math.PI,
    radiusU: 0.009 + random() * 0.009,
    radiusV: 0.007 + random() * 0.007
  }));
  for (let row = 0; row < height; row++) {
    const v = (row + 0.5) / height;
    const latitude = (v - 0.5) * Math.PI;
    const elevation = Math.max(0, Math.sin(latitude));
    const gradient = Math.pow(elevation, 0.72);
    const horizonHaze = (1 - smoothstep(elevation / 0.12)) * 3;
    for (let column = 0; column < width; column++) {
      const u = (column + 0.5) / width;
      const longitude = (u - 0.5) * Math.PI * 2;
      let cloudOpacity = 0;
      if (latitude > 0 && latitude < 0.4) {
        for (const patch of clouds) {
          const separation = Math.abs(u - patch.u);
          const dx = Math.min(separation, 1 - separation) / patch.radiusU;
          const dy = (v - patch.v) / patch.radiusV;
          const radiusSquared = dx * dx + dy * dy;
          if (radiusSquared > 1.4) continue;
          const billow = cloudNoise(u, v, 70, 85) * 0.65 + cloudNoise(u, v, 170, 230) * 0.35;
          const opacity = smoothstep((1 - radiusSquared + (billow - 0.5) * 0.7) / 0.35) *
            (0.55 + 0.45 * billow);
          cloudOpacity = Math.max(cloudOpacity, opacity);
        }
      }
      const cloud = cloudOpacity * 18;
      const rayX = Math.cos(latitude) * Math.cos(longitude);
      const rayY = Math.sin(latitude);
      const rayZ = Math.cos(latitude) * Math.sin(longitude);
      const sunAlignment = Math.max(0, (rayX * sun[0] + rayY * sun[1] + rayZ * sun[2]) / sunLength);
      const sunGlow = Math.pow(sunAlignment, 120) * 14;
      const offset = (row * width + column) * 4;
      pixels[offset] = Math.round(Math.max(0, Math.min(255, 177 - 70 * gradient + cloud * 1.6 + horizonHaze + sunGlow)));
      pixels[offset + 1] = Math.round(Math.max(0, Math.min(255, 202 - 38 * gradient + cloud + horizonHaze + sunGlow * 0.8)));
      pixels[offset + 2] = Math.round(Math.max(0, Math.min(255, 215 - 10 * gradient + cloud * 0.5 + horizonHaze + sunGlow * 0.55)));
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
