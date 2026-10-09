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
import { createLakeSkyCondition, lakeSkySunDirectionNed } from "../../contracts/lake-sky.js";
import type { LakeSkyCondition } from "../../contracts/lake-sky.js";

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

interface CumulusPatch {
  readonly u: number;
  readonly baseV: number;
  readonly halfWidthU: number;
  readonly heightV: number;
  readonly lobeHeights: readonly number[];
  readonly lobeOffsets: readonly number[];
  readonly lobeWidths: readonly number[];
}

/** Shared, subdued sky radiance for the background and the lake reflection. */
export function createLakeSkyTexture(condition: LakeSkyCondition | null = null): DataTexture {
  const sky = condition === null ? null : createLakeSkyCondition(condition);
  const width = 1024;
  const height = 512;
  const pixels = new Uint8Array(width * height * 4);
  const direction = sky === null ? null : lakeSkySunDirectionNed(sky);
  const sun = direction === null ? [0, 1, 0] as const : [direction.east, -direction.down, -direction.north] as const;
  const daylight = sky !== null && sky.sunElevationDegrees >= 0 ? 1 : 0;
  const cloudCount = sky === null ? 0 : Math.round(36 * sky.cloudFraction / 0.35);
  let randomState = 0x4f5a2c91;
  const random = (): number => {
    randomState = (Math.imul(randomState, 1664525) + 1013904223) >>> 0;
    return randomState / 4294967296;
  };
  // Fair-weather cumulus have a common flat condensation base and several
  // rounded updraft lobes. Keep individual heaps small and separated.
  const clouds: CumulusPatch[] = [];
  for (let attempt = 0; attempt < 160 && clouds.length < cloudCount; attempt++) {
    const elevation = 0.04 + Math.pow(random(), 1.6) * 0.32;
    const scale = 0.85 + elevation / 0.36;
    const halfWidthU = (0.007 + random() * 0.008) * scale;
    const seamMargin = halfWidthU + 0.002;
    const patch: CumulusPatch = {
      u: seamMargin + random() * (1 - 2 * seamMargin),
      baseV: 0.5 + elevation / Math.PI,
      halfWidthU,
      heightV: (0.04 + random() * 0.05) * scale / Math.PI,
      lobeHeights: [0.4 + random() * 0.5, 0.48 + random() * 0.52,
        0.48 + random() * 0.52, 0.38 + random() * 0.55],
      lobeOffsets: [-0.72 + random() * 0.3, -0.42 + random() * 0.36,
        0.06 + random() * 0.38, 0.4 + random() * 0.34],
      lobeWidths: [0.35 + random() * 0.26, 0.38 + random() * 0.3,
        0.36 + random() * 0.3, 0.35 + random() * 0.26]
    };
    const overlaps = clouds.some((other) => {
      const separation = Math.abs(patch.u - other.u);
      const horizontal = Math.min(separation, 1 - separation);
      return horizontal < (patch.halfWidthU + other.halfWidthU) * 0.82 &&
        Math.abs(patch.baseV - other.baseV) < (patch.heightV + other.heightV) * 0.45;
    });
    if (!overlaps) clouds.push(patch);
  }
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
      let cloudBrightness = 0;
      if (latitude > 0 && latitude < 0.5) {
        for (const patch of clouds) {
          const separation = Math.abs(u - patch.u);
          const x = (separation <= 0.5 ? u - patch.u : u - patch.u - Math.sign(u - patch.u))
            / patch.halfWidthU;
          const y = (v - patch.baseV) / patch.heightV;
          if (Math.abs(x) > 1.08 || y < -0.07 || y > 1.08) continue;
          let top = 0.18 * Math.sqrt(Math.max(0, 1 - x * x));
          for (let lobe = 0; lobe < 4; lobe++) {
            const localX = (x - (patch.lobeOffsets[lobe] ?? 0)) / (patch.lobeWidths[lobe] ?? 0.5);
            top = Math.max(top, (patch.lobeHeights[lobe] ?? 0) *
              Math.sqrt(Math.max(0, 1 - localX * localX)));
          }
          // The panorama spans 360 degrees horizontally and 180 vertically;
          // twice as many columns as rows keeps the billows angularly round.
          const billow = cloudNoise(u, v, 150, 75) * 0.55 + cloudNoise(u, v, 400, 200) * 0.45;
          const edge = smoothstep((top - y + (billow - 0.5) * 0.16) / 0.11);
          const base = smoothstep((y + 0.02) / 0.055);
          const opacity = edge * base * (0.55 + 0.6 * billow);
          if (opacity > cloudOpacity) {
            cloudOpacity = opacity;
            cloudBrightness = Math.max(0, Math.min(1,
              0.23 + 0.58 * y / Math.max(top, 0.01) + (billow - 0.5) * 0.65));
          }
        }
      }
      const rayX = Math.cos(latitude) * Math.cos(longitude);
      const rayY = Math.sin(latitude);
      const rayZ = Math.cos(latitude) * Math.sin(longitude);
      const sunAlignment = Math.max(0, rayX * sun[0] + rayY * sun[1] + rayZ * sun[2]);
      const sunGlow = Math.pow(sunAlignment, 120) * 14 * daylight;
      const offset = (row * width + column) * 4;
      const skyR = 177 - 70 * gradient + horizonHaze + sunGlow;
      const skyG = 202 - 38 * gradient + horizonHaze + sunGlow * 0.8;
      const skyB = 215 - 10 * gradient + horizonHaze + sunGlow * 0.55;
      const cloudR = 134 + 95 * cloudBrightness;
      const cloudG = 148 + 90 * cloudBrightness;
      const cloudB = 157 + 80 * cloudBrightness;
      pixels[offset] = Math.round(Math.max(0, Math.min(255, skyR * (1 - cloudOpacity) + cloudR * cloudOpacity)));
      pixels[offset + 1] = Math.round(Math.max(0, Math.min(255, skyG * (1 - cloudOpacity) + cloudG * cloudOpacity)));
      pixels[offset + 2] = Math.round(Math.max(0, Math.min(255, skyB * (1 - cloudOpacity) + cloudB * cloudOpacity)));
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
