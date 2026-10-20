import "./lake-model-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { createHash } from "node:crypto";
import { ClampToEdgeWrapping, EquirectangularReflectionMapping, LinearFilter,
  LinearMipmapLinearFilter, RepeatWrapping, SRGBColorSpace } from "three";
import { LakeSkyImageBridge } from "../../web/pkg/birdman_game_wasm.js";
import { createLakeSkyTexture, lakeSkyHaze } from "../../web/src/render/engines/three/lake-sky-texture.js";

const condition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, cloudBaseMeters: 1500, visibilityMeters: 25_000 };

function fingerprint(texture: ReturnType<typeof createLakeSkyTexture>): string {
  if (!(texture.image.data instanceof Uint8Array)) throw new Error("Sky texture has no byte data");
  return createHash("sha256").update(texture.image.data).digest("hex");
}

describe("shared lake sky", () => {
  it("transfers shared haze bounds without replacing absent visibility with weather metadata", () => {
    expect(lakeSkyHaze(undefined)).toEqual([0x9aafb1, 14_000, 80_000]);
    expect(lakeSkyHaze(condition.visibilityMeters)).toEqual([0x9aafb1, 4_375, 25_000]);
    for (const visibility of [0, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_VALUE, Number.MIN_VALUE, 1e-40, 3e38]) {
      expect(() => lakeSkyHaze(visibility)).toThrow();
    }
  });

  it.each([{ sky: null }, { sky: condition }])("frees the owned bridge after copying the panorama", ({ sky }) => {
    const release = vi.spyOn(LakeSkyImageBridge.prototype, "free");
    try {
      const texture = createLakeSkyTexture(sky);
      try {
        expect(release).toHaveBeenCalledTimes(1);
        expect(texture.image.data).toBeInstanceOf(Uint8Array);
        expect(texture.image.data?.length).toBe(1024 * 512 * 4);
        expect(texture.image.data?.slice(0, 4)).toEqual(new Uint8Array([180, 205, 218, 255]));
      } finally { texture.dispose(); }
    } finally { release.mockRestore(); }
  });

  it("frees the bridge when copying the image fails", () => {
    const cause = new Error("sky image copy failed");
    const pixels = vi.spyOn(LakeSkyImageBridge.prototype, "rgba").mockImplementationOnce(() => { throw cause; });
    const release = vi.spyOn(LakeSkyImageBridge.prototype, "free");
    try {
      expect(() => createLakeSkyTexture(condition)).toThrow(cause);
      expect(release).toHaveBeenCalledTimes(1);
    } finally { pixels.mockRestore(); release.mockRestore(); }
  });

  it("rejects invalid render parameters at the generated WASM boundary", () => {
    for (const [azimuth, elevation, fraction, visibility] of [
      [Number.NaN, 35, 0.35, 28_000], [360, 35, 0.35, 28_000],
      [240, Number.POSITIVE_INFINITY, 0.35, 28_000], [240, 91, 0.35, 28_000],
      [240, 35, -0.1, 28_000], [240, 35, 1.1, 28_000],
      [240, 35, 0.35, 0], [240, 35, 0.35, Number.NaN]
    ] as const) {
      expect(() => new LakeSkyImageBridge(azimuth, elevation, fraction, visibility)).toThrow();
    }
  });

  it("keeps the reflected horizon and equirectangular seam continuous", () => {
    const texture = createLakeSkyTexture({ ...condition, cloudFraction: 0 });
    try {
      expect(texture.mapping).toBe(EquirectangularReflectionMapping);
      expect(texture.colorSpace).toBe(SRGBColorSpace);
      expect(texture.wrapS).toBe(RepeatWrapping);
      expect(texture.wrapT).toBe(ClampToEdgeWrapping);
      expect(texture.magFilter).toBe(LinearFilter);
      expect(texture.minFilter).toBe(LinearMipmapLinearFilter);
      expect(texture.generateMipmaps).toBe(true);
      const { data, width, height } = texture.image;
      if (data === null) throw new Error("Sky texture has no pixel data");
      expect(width).toBe(1024);
      expect(height).toBe(512);
      const pixel = (x: number, y: number, channel: number): number =>
        data[(y * width + x) * 4 + channel] ?? 0;
      for (let y = height / 2; y < height; y++) {
        for (let channel = 0; channel < 3; channel++) {
          expect(Math.abs(pixel(0, y, channel) - pixel(width - 1, y, channel))).toBeLessThan(4);
        }
      }
      let horizonDelta = 0;
      for (let x = 0; x < width; x++) {
        for (let channel = 0; channel < 3; channel++) {
          horizonDelta = Math.max(horizonDelta,
            Math.abs(pixel(x, height / 2 - 1, channel) - pixel(x, height / 2, channel)));
        }
      }
      expect(horizonDelta).toBeLessThan(2);
    } finally {
      texture.dispose();
    }
  });

  it("projects cloud fraction into the same deterministic sky panorama", () => {
    const clear = createLakeSkyTexture({ ...condition, cloudFraction: 0 });
    const repeated = createLakeSkyTexture({ ...condition, cloudFraction: 0 });
    const cloudy = createLakeSkyTexture({ ...condition, cloudFraction: 0.5 });
    try {
      expect(fingerprint(clear)).toBe(fingerprint(repeated));
      expect(fingerprint(cloudy)).not.toBe(fingerprint(clear));
    } finally { clear.dispose(); repeated.dispose(); cloudy.dispose(); }
  });

  it.each([[0, 0.25], [90, 0.5], [180, 0.75], [270, 0]])("places sun azimuth %s at its independent equirectangular longitude", (azimuth, fraction) => {
    const texture = createLakeSkyTexture({ ...condition, sunAzimuthDegrees: azimuth, sunElevationDegrees: 35, cloudFraction: 0 });
    try {
      const { data, width, height } = texture.image;
      if (data === null) throw new Error("Sky texture has no pixel data");
      const row = Math.floor(height * (0.5 + 35 / 180));
      const sunColumn = Math.floor(width * fraction);
      const oppositeColumn = (sunColumn + width / 2) % width;
      const channel = (column: number): number => data[(row * width + column) * 4] ?? 0;
      expect(channel(sunColumn) - channel(oppositeColumn)).toBeGreaterThan(12);
    } finally { texture.dispose(); }
  });

  it("keeps unrecorded sky free of invented directional sun and cloud metadata", () => {
    const unknown = createLakeSkyTexture(null);
    const night = createLakeSkyTexture({ ...condition, cloudFraction: 0, sunElevationDegrees: -35 });
    try {
      expect(fingerprint(unknown)).toBe(fingerprint(night));
      const { data, width, height } = unknown.image;
      if (data === null) throw new Error("Sky texture has no pixel data");
      for (const row of [height / 2, Math.floor(height * 0.65), height - 1]) {
        for (const column of [0, width / 4, width / 2, width - 1]) {
          const offset = (row * width + column) * 4;
          const first = row * width * 4;
          expect(data.slice(offset, offset + 4)).toEqual(data.slice(first, first + 4));
        }
      }
    } finally { unknown.dispose(); night.dispose(); }
  });
});
