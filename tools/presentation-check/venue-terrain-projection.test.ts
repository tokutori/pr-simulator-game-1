import { setImmediate } from "node:timers/promises";
import { BufferAttribute, BufferGeometry, DoubleSide, MeshLambertMaterial } from "three";
import { describe, expect, it, vi } from "vitest";
import "./lake-model-fixture.js";
import { VenueTerrainGeometryBridge } from "../../web/pkg/birdman_game_wasm.js";
import { createLakeVenue, venueTerrainMeshes } from "../../web/src/render/engines/three/lake-venue-mesh.js";

function isNedProjection(attribute: ReturnType<BufferGeometry["getAttribute"]>, values: Float32Array): boolean {
  const source = new BufferAttribute(values, 3);
  if (attribute.count !== source.count) return false;
  for (let vertex = 0; vertex < source.count; vertex++) {
    if (attribute.getX(vertex) !== source.getY(vertex) || attribute.getY(vertex) !== -source.getZ(vertex)
        || attribute.getZ(vertex) !== -source.getX(vertex)) return false;
  }
  return true;
}

describe("registered terrain projection", () => {
  it("projects every shared position and completed normal without changing linear colors or indices", () => {
    const shared = new VenueTerrainGeometryBridge();
    const meshes = venueTerrainMeshes();
    try {
      expect(meshes).toHaveLength(14);
      expect(meshes).toHaveLength(shared.mesh_count());
      for (const [index, mesh] of meshes.entries()) {
        expect(mesh.name).toBe(shared.mesh_name(index));
        const position = mesh.geometry.getAttribute("position");
        const normal = mesh.geometry.getAttribute("normal");
        const color = mesh.geometry.getAttribute("color").array;
        const expectedColor = shared.colors_linear(index);
        expect(isNedProjection(position, shared.positions_ned(index)), mesh.name).toBe(true);
        expect(isNedProjection(normal, shared.normals_ned(index)), mesh.name).toBe(true);
        expect(Buffer.from(color.buffer, color.byteOffset, color.byteLength)
          .equals(Buffer.from(expectedColor.buffer, expectedColor.byteOffset, expectedColor.byteLength)), mesh.name).toBe(true);
        const indices = mesh.geometry.index?.array;
        if (indices === undefined) throw new Error(`Missing shared terrain indices: ${mesh.name}`);
        const expectedIndices = shared.indices(index);
        expect(Buffer.from(indices.buffer, indices.byteOffset, indices.byteLength)
          .equals(Buffer.from(expectedIndices.buffer, expectedIndices.byteOffset, expectedIndices.byteLength)), mesh.name).toBe(true);
        expect(mesh.material.vertexColors).toBe(true);
        expect(mesh.material.side).toBe(DoubleSide);
        expect(mesh.castShadow).toBe(false);
        expect(mesh.receiveShadow).toBe(false);
      }
    } finally {
      shared.free();
      for (const mesh of meshes) mesh.geometry.dispose();
      for (const material of new Set(meshes.map((mesh) => mesh.material))) material.dispose();
    }
  });

  it("releases all prepared geometry and materials and retains the original bulk failure", () => {
    const failure = new Error("Original terrain normal failure");
    const original = new VenueTerrainGeometryBridge();
    const firstNormals = original.normals_ned(0);
    original.free();
    const normals = vi.spyOn(VenueTerrainGeometryBridge.prototype, "normals_ned")
      .mockImplementation((index) => {
        if (index === 1) throw failure;
        return firstNormals;
      });
    const free = vi.spyOn(VenueTerrainGeometryBridge.prototype, "free");
    const geometryDispose = vi.spyOn(BufferGeometry.prototype, "dispose");
    const materialDispose = vi.spyOn(MeshLambertMaterial.prototype, "dispose");
    try {
      expect(() => venueTerrainMeshes()).toThrow(failure);
      expect(geometryDispose).toHaveBeenCalledTimes(2);
      expect(materialDispose).toHaveBeenCalledTimes(3);
      expect(free).toHaveBeenCalledTimes(1);
    } finally {
      normals.mockRestore();
      free.mockRestore();
      geometryDispose.mockRestore();
      materialDispose.mockRestore();
    }
  });

  it("retains the launch geometry and warning when terrain preparation fails", async () => {
    const failure = new Error("Original registered terrain failure");
    const count = vi.spyOn(VenueTerrainGeometryBridge.prototype, "mesh_count")
      .mockImplementation(() => { throw failure; });
    const warning = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const venue = createLakeVenue();
    try {
      await setImmediate();
      expect(venue.group.children).toHaveLength(5);
      expect(venue.group.getObjectByName("sloped-launch-platform")).toBeDefined();
      expect(warning).toHaveBeenCalledExactlyOnceWith("Lake Biwa environment could not be loaded", failure);
    } finally {
      venue.dispose();
      count.mockRestore();
      warning.mockRestore();
    }
  });

  it("does not prepare a terrain packet after the venue was disposed", async () => {
    const count = vi.spyOn(VenueTerrainGeometryBridge.prototype, "mesh_count");
    const venue = createLakeVenue();
    venue.dispose();
    try {
      await setImmediate();
      expect(count).not.toHaveBeenCalled();
    } finally { count.mockRestore(); }
  });
});
