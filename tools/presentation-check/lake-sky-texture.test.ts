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
      expect(width).toBe(512);
      expect(height).toBe(256);
      const pixel = (x: number, y: number, channel: number): number =>
        data[(y * width + x) * 4 + channel] ?? 0;
      for (const y of [height / 2, height * 3 / 4]) {
        for (let channel = 0; channel < 3; channel++) {
          expect(Math.abs(pixel(0, y, channel) - pixel(width - 1, y, channel))).toBeLessThan(4);
        }
      }
      for (let channel = 0; channel < 3; channel++) {
        expect(Math.abs(pixel(0, height / 2 - 1, channel) - pixel(0, height / 2, channel))).toBeLessThan(2);
      }
    } finally {
      texture.dispose();
    }
  });
});
