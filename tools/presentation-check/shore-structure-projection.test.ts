import { describe, expect, it, vi } from "vitest";
import venueFeatures from "../../assets/biwa-venue-features.json";
import { BufferAttribute, DoubleSide } from "three";
import "./lake-model-fixture.js";
import { ShoreStructureGeometryBridge } from "../../web/pkg/birdman_game_wasm.js";
import { shoreStructureMesh } from "../../web/src/render/engines/three/lake-venue-mesh.js";

describe("OSM shoreline structure projection", () => {
  it("retains attributed OSM source features", () => {
    expect(venueFeatures.license).toBe("ODbL-1.0");
    expect(venueFeatures.attribution).toBe("© OpenStreetMap contributors");
    expect(venueFeatures.sourceSnapshotSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(venueFeatures.features).toHaveLength(50);
    expect(venueFeatures.features.map((feature) => feature.kind)).toEqual(expect.arrayContaining([
      "beach", "woodland", "tree-row", "pier", "quay", "breakwater"
    ]));
  });

  it.each(["pier", "quay", "breakwater"] as const)("projects the shared raised %s mesh and material", (kind) => {
    const mesh = shoreStructureMesh(kind);
    const geometry = mesh.geometry;
    const shared = new ShoreStructureGeometryBridge(kind);
    try {
      geometry.computeBoundingBox();
      const position = geometry.getAttribute("position");
      expect(position.count).toBeGreaterThan(0);
      expect(geometry.index?.count).toBeGreaterThan(0);
      expect(geometry.boundingBox).not.toBeNull();
      expect(geometry.boundingBox?.max.y).toBeGreaterThan(0.5);
      const ned = new BufferAttribute(shared.positions_ned(), 3);
      expect(position.count).toBe(ned.count);
      expect(Array.from(geometry.index?.array ?? [])).toEqual(Array.from(shared.indices()));
      for (let index = 0; index < position.count; index++) {
        expect(Number.isFinite(position.getX(index))).toBe(true);
        expect(Number.isFinite(position.getY(index))).toBe(true);
        expect(Number.isFinite(position.getZ(index))).toBe(true);
        expect(position.getX(index)).toBeCloseTo(ned.getY(index), 5);
        expect(position.getY(index)).toBeCloseTo(-ned.getZ(index), 5);
        expect(position.getZ(index)).toBeCloseTo(-ned.getX(index), 5);
      }
      expect(mesh.material.color.getHex()).toBe(shared.color_srgb());
      expect(mesh.material.side).toBe(DoubleSide);
      expect(mesh.castShadow).toBe(true);
      expect(mesh.receiveShadow).toBe(true);
    } finally {
      shared.free();
      geometry.dispose();
      mesh.material.dispose();
    }
  });

  it("releases the shared bridge after projection and preserves getter failures", () => {
    const free = vi.spyOn(ShoreStructureGeometryBridge.prototype, "free");
    const positions = vi.spyOn(ShoreStructureGeometryBridge.prototype, "positions_ned");
    try {
      const mesh = shoreStructureMesh("pier");
      expect(free).toHaveBeenCalledTimes(1);
      mesh.geometry.dispose();
      mesh.material.dispose();
      const failure = new Error("Original shared structure failure");
      positions.mockImplementationOnce(() => { throw failure; });
      expect(() => shoreStructureMesh("quay")).toThrow(failure);
      expect(free).toHaveBeenCalledTimes(2);
    } finally {
      free.mockRestore();
      positions.mockRestore();
    }
  });
});
