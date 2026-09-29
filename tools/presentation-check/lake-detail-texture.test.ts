import { describe, expect, it } from "vitest";
import { createLakeDetailLayer } from "../../web/src/render/engines/three/lake-detail-texture.js";

function textureHash(data: ArrayLike<number>): number {
  let hash = 2166136261;
  for (let index = 0; index < data.length; index++) {
    hash = Math.imul(hash ^ (data[index] ?? 0), 16777619);
  }
  return hash >>> 0;
}

describe("lake detail texture", () => {
  it("is deterministic for a seed and changes when the seed changes", () => {
    const first = createLakeDetailLayer(64, 48, 123, 1, 0);
    const repeat = createLakeDetailLayer(64, 48, 123, 1, 0);
    const alternate = createLakeDetailLayer(64, 48, 124, 1, 0);

    try {
      const firstData = first.texture.image.data;
      const repeatData = repeat.texture.image.data;
      const alternateData = alternate.texture.image.data;
      if (firstData === null || repeatData === null || alternateData === null) {
        throw new Error("Lake detail texture has no pixel data");
      }
      expect(textureHash(firstData)).toBe(textureHash(repeatData));
      expect(textureHash(firstData)).not.toBe(textureHash(alternateData));
    } finally {
      first.texture.dispose();
      repeat.texture.dispose();
      alternate.texture.dispose();
    }
  });

  it("keeps the generated height field centered on the still-water level", () => {
    const layer = createLakeDetailLayer(64, 48, 123, 1, 0);
    try {
      const data = layer.texture.image.data;
      if (data === null) throw new Error("Lake detail texture has no pixel data");
      let heightOffset = 0;
      for (let index = 3; index < data.length; index += 4) {
        heightOffset += (data[index] ?? 0) - 128;
      }
      expect(Math.abs(heightOffset / (data.length / 4))).toBeLessThan(0.03);
    } finally {
      layer.texture.dispose();
    }
  });

  it("keeps production detail layers informative without clipping encoded slopes", () => {
    const windSpeed = Math.hypot(0.54, 1.07);
    const directionX = 1.07 / windSpeed;
    const directionZ = -0.54 / windSpeed;
    const layers = [
      createLakeDetailLayer(64, 1400, 1717, directionX, directionZ),
      createLakeDetailLayer(193, 2400, 2917, directionX, directionZ)
    ];

    try {
      for (const layer of layers) {
        const data = layer.texture.image.data;
        if (data === null) throw new Error("Lake detail texture has no pixel data");

        let clippedSlopePixels = 0;
        let activeSlopePixels = 0;
        let heightOffset = 0;
        let squaredHeightOffset = 0;
        const pixelCount = data.length / 4;
        for (let index = 0; index < data.length; index += 4) {
          const slopeX = data[index] ?? 128;
          const slopeZ = data[index + 1] ?? 128;
          const heightOffsetByte = (data[index + 3] ?? 128) - 128;
          if (slopeX <= 1 || slopeX >= 255 || slopeZ <= 1 || slopeZ >= 255) clippedSlopePixels += 1;
          if ((slopeX - 128) ** 2 + (slopeZ - 128) ** 2 > 127 ** 2 * 0.01) activeSlopePixels += 1;
          const heightMeters = heightOffsetByte / 255;
          heightOffset += heightMeters;
          squaredHeightOffset += heightMeters ** 2;
        }

        expect(clippedSlopePixels / pixelCount).toBeLessThan(0.01);
        expect(activeSlopePixels / pixelCount).toBeGreaterThan(0.1);
        expect(Math.abs(heightOffset / pixelCount)).toBeLessThan(0.005);
        expect(Math.sqrt(squaredHeightOffset / pixelCount)).toBeGreaterThan(0.02);
      }
    } finally {
      for (const layer of layers) layer.texture.dispose();
    }
  });
});
