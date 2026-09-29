import { describe, expect, it } from "vitest";
import { EquirectangularReflectionMapping, SRGBColorSpace } from "three";
import { createLakeSkyTexture } from "../../web/src/render/engines/three/lake-sky-texture.js";

describe("shared lake sky", () => {
  it("keeps the reflected horizon and equirectangular seam continuous", () => {
    const texture = createLakeSkyTexture();
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
});
