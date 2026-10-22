import { setImmediate } from "node:timers/promises";
import shoreline from "../../assets/biwa-shoreline.json";
import terrain from "../../assets/biwa-terrain.json";
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

function pointInRing(north: number, east: number, ring: readonly (readonly number[])[]): boolean {
  let inside = false;
  for (let index = 0, previous = ring.length - 1; index < ring.length; previous = index, index++) {
    const currentPoint = ring[index];
    const previousPoint = ring[previous];
    if (currentPoint === undefined || previousPoint === undefined) continue;
    const currentNorth = currentPoint[0];
    const currentEast = currentPoint[1];
    const previousNorth = previousPoint[0];
    const previousEast = previousPoint[1];
    if (currentNorth === undefined || currentEast === undefined || previousNorth === undefined || previousEast === undefined) continue;
    const crossesEast = (currentEast > east) !== (previousEast > east);
    if (crossesEast && north < (previousNorth - currentNorth) * (east - currentEast) /
      (previousEast - currentEast) + currentNorth) inside = !inside;
  }
  return inside;
}

describe("registered terrain projection", () => {
  it("uses detailed terrain near the launch site and coarse distant ridges", () => {
    expect(terrain.finePatches).toHaveLength(10);
    for (const patch of terrain.finePatches) {
      expect(patch.elevationMetersAboveWater).toHaveLength(patch.columns * patch.rows);
    }
    expect(terrain.finePatches.map((patch) => patch.id)).toEqual(expect.arrayContaining([
      "terrain-patch-takeshima", "terrain-patch-okishima", "terrain-patch-okonozu",
      "terrain-patch-chikubushima", "terrain-patch-regional-shore",
      "terrain-patch-wooded-islet-1156534005",
      "terrain-patch-ridge-northwest", "terrain-patch-ridge-north",
      "terrain-patch-ridge-northwest-far", "terrain-patch-ridge-west"
    ]));
    const regionalShore = terrain.finePatches.find((patch) => patch.id === "terrain-patch-regional-shore");
    expect(regionalShore).toMatchObject({ northMinMeters: -6_030, eastMinMeters: -6_030, columns: 401, rows: 401, terrainStepMeters: 30 });
    expect(regionalShore?.elevationMetersAboveWater.some((height) => height !== null)).toBe(true);
  });

  it("retains measured opposite-shore summits in the source-resolved terrain patches", () => {
    const peaks = [
      { id: "terrain-patch-ridge-northwest", north: 20_894, east: -27_271, minimumHeight: 880 },
      { id: "terrain-patch-ridge-north", north: 28_457, east: 7_508, minimumHeight: 1_220 },
      { id: "terrain-patch-ridge-northwest-far", north: 32_840, east: -20_885, minimumHeight: 820 },
      { id: "terrain-patch-ridge-west", north: -3_277, east: -32_495, minimumHeight: 1_105 }
    ];
    for (const peak of peaks) {
      const patch = terrain.finePatches.find((entry) => entry.id === peak.id);
      expect(patch, peak.id).toMatchObject({ columns: 28, rows: 28, terrainStepMeters: 90, blendWidthMeters: 600 });
      if (patch === undefined) throw new TypeError(`Missing summit patch ${peak.id}`);
      const row = patch.rows - 1 - Math.round((peak.north - patch.northMinMeters) / patch.terrainStepMeters);
      const column = Math.round((peak.east - patch.eastMinMeters) / patch.terrainStepMeters);
      expect(patch.elevationMetersAboveWater[row * patch.columns + column], peak.id).toBeGreaterThan(peak.minimumHeight);
    }
  });

  it("places detailed terrain over the mapped islands", () => {
    const islands = new Map(shoreline.islands.filter((island) => island.name !== null)
      .map((island) => [island.name, island.ringNorthEastMeters]));
    expect([...islands.keys()]).toEqual(expect.arrayContaining(["多景島", "沖島", "オコノ洲"]));
    for (const islandName of ["多景島", "沖島", "オコノ洲"]) {
      const ring = islands.get(islandName);
      expect(ring).toBeDefined();
      if (ring === undefined) throw new TypeError(`Missing mapped island ${islandName}`);
      let northTotal = 0;
      let eastTotal = 0;
      for (const point of ring) {
        const north = point[0];
        const east = point[1];
        if (north === undefined || east === undefined) throw new TypeError(`Invalid island ring ${islandName}`);
        northTotal += north;
        eastTotal += east;
      }
      const northMeters = northTotal / ring.length;
      const eastMeters = eastTotal / ring.length;
      const covered = terrain.finePatches.some((patch) => {
        const northMax = patch.northMinMeters + (patch.rows - 1) * patch.terrainStepMeters;
        const eastMax = patch.eastMinMeters + (patch.columns - 1) * patch.terrainStepMeters;
        return northMeters >= patch.northMinMeters && northMeters <= northMax &&
          eastMeters >= patch.eastMinMeters && eastMeters <= eastMax;
      });
      expect(covered, `${islandName} detail patch`).toBe(true);
    }
  });

  it("keeps valid 30 m AW3D30 samples across each named island interior", () => {
    const islands = [
      { name: "多景島", patchId: "terrain-patch-takeshima" },
      { name: "竹生島", patchId: "terrain-patch-chikubushima" },
      { name: "沖島", patchId: "terrain-patch-okishima" },
      { name: "オコノ洲", patchId: "terrain-patch-okonozu" }
    ];
    for (const { name, patchId } of islands) {
      const island = shoreline.islands.find((entry) => entry.name === name);
      const patch = terrain.finePatches.find((entry) => entry.id === patchId);
      expect(island, `${name} outline`).toBeDefined();
      expect(patch, `${name} detail grid`).toBeDefined();
      if (island === undefined || patch === undefined) throw new TypeError(`Missing ${name} terrain inputs`);

      const northMax = patch.northMinMeters + (patch.rows - 1) * patch.terrainStepMeters;
      const eastMax = patch.eastMinMeters + (patch.columns - 1) * patch.terrainStepMeters;
      for (const [north, east] of island.ringNorthEastMeters) {
        expect(north, `${name} north coverage`).toBeGreaterThanOrEqual(patch.northMinMeters);
        expect(north, `${name} north coverage`).toBeLessThanOrEqual(northMax);
        expect(east, `${name} east coverage`).toBeGreaterThanOrEqual(patch.eastMinMeters);
        expect(east, `${name} east coverage`).toBeLessThanOrEqual(eastMax);
      }

      let interiorCells = 0;
      let validInteriorCells = 0;
      for (let row = 0; row < patch.rows; row++) {
        const north = patch.northMinMeters + (patch.rows - row - 1) * patch.terrainStepMeters;
        for (let column = 0; column < patch.columns; column++) {
          const east = patch.eastMinMeters + column * patch.terrainStepMeters;
          if (!pointInRing(north, east, island.ringNorthEastMeters)) continue;
          interiorCells++;
          if (patch.elevationMetersAboveWater[row * patch.columns + column] !== null) validInteriorCells++;
        }
      }
      expect(interiorCells, `${name} interior samples`).toBeGreaterThan(0);
      expect(validInteriorCells / interiorCells, `${name} valid interior coverage`).toBeGreaterThanOrEqual(0.95);
    }
  });


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
