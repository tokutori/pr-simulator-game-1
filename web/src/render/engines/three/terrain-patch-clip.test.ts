import { describe, expect, it } from "vitest";
import { subtractTerrainPatch, type TerrainVertex } from "./terrain-patch-clip.js";

function polygonArea(polygon: readonly TerrainVertex[]): number {
  let twiceArea = 0;
  for (let index = 0; index < polygon.length; index++) {
    const current = polygon[index];
    const next = polygon[(index + 1) % polygon.length];
    if (current === undefined || next === undefined) continue;
    twiceArea += current.x * next.z - next.x * current.z;
  }
  return Math.abs(twiceArea) / 2;
}

describe("terrain patch clipping", () => {
  it("removes the patch footprint from a coarse triangle and preserves interpolated height", () => {
    const triangle: readonly TerrainVertex[] = [
      { x: -1, y: -3, z: -1 },
      { x: 2, y: 0, z: -1 },
      { x: -1, y: 3, z: 2 }
    ];
    const pieces = subtractTerrainPatch(triangle, { minX: 0, maxX: 1, minZ: 0, maxZ: 1 });

    expect(pieces.reduce((area, piece) => area + polygonArea(piece), 0)).toBeCloseTo(4);
    for (const piece of pieces) {
      for (const vertex of piece) {
        expect(vertex.y).toBeCloseTo(vertex.x + 2 * vertex.z);
      }
      const center = piece.reduce((sum, vertex) => ({ x: sum.x + vertex.x, z: sum.z + vertex.z }), { x: 0, z: 0 });
      expect(center.x / piece.length < 1 && center.x / piece.length > 0 &&
        center.z / piece.length < 1 && center.z / piece.length > 0).toBe(false);
    }
  });
});
