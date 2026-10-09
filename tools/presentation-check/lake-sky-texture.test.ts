import { describe, expect, it } from "vitest";
import { createHash } from "node:crypto";
import { EquirectangularReflectionMapping, SRGBColorSpace } from "three";
import { createLakeSkyTexture } from "../../web/src/render/engines/three/lake-sky-texture.js";

const condition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, visibilityMeters: 25_000 };

function fingerprint(texture: ReturnType<typeof createLakeSkyTexture>): string {
  if (!(texture.image.data instanceof Uint8Array)) throw new Error("Sky texture has no byte data");
  return createHash("sha256").update(texture.image.data).digest("hex");
}

describe("shared lake sky", () => {
  it("keeps the reflected horizon and equirectangular seam continuous", () => {
    const texture = createLakeSkyTexture(condition);
    try {
      expect(texture.mapping).toBe(EquirectangularReflectionMapping);
      expect(texture.colorSpace).toBe(SRGBColorSpace);
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

  it("is deterministic and responds to the recorded cloud fraction", () => {
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
