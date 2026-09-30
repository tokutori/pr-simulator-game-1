import { describe, expect, it } from "vitest";
import shoreline from "../../../../../assets/biwa-shoreline.json";
import terrainSource from "../../../../../assets/biwa-terrain.json";
import landMask from "../../../../../assets/biwa-land-mask.json";
import { applyLandMask, fillIslandTerrainPatch, okinoshiraishiRockGeometry, shoreStructureGeometry, shorelineTransitionGeometry, takeshimaCanopyGeometry, terrainGeometry } from "./lake-venue-mesh.js";

const sourceTerrain = applyLandMask(terrainSource as unknown as Parameters<typeof applyLandMask>[0],
  landMask as unknown as Parameters<typeof applyLandMask>[1]);
const islandFillSpecs = [
  { name: "多景島", patchId: "terrain-patch-takeshima" },
  { name: "オコノ洲", patchId: "terrain-patch-okonozu" }
];
const terrain = {
  ...sourceTerrain,
  finePatches: sourceTerrain.finePatches.map((patch) => {
    const fill = islandFillSpecs.find((entry) => entry.patchId === patch.id);
    if (fill === undefined) return patch;
    const island = shoreline.islands.find((entry) => entry.name === fill.name);
    if (island === undefined) throw new TypeError(`Missing ${fill.name} source outline`);
    return fillIslandTerrainPatch(island.ringNorthEastMeters as unknown as Parameters<typeof fillIslandTerrainPatch>[0], patch);
  })
};

function renderedTerrainHeightAt(geometry: ReturnType<typeof terrainGeometry>, x: number, z: number): number | null {
  const positions = geometry.getAttribute("position");
  const indices = geometry.index;
  if (indices === null) return null;
  for (let offset = 0; offset < indices.count; offset += 3) {
    const a = indices.getX(offset);
    const b = indices.getX(offset + 1);
    const c = indices.getX(offset + 2);
    const ax = positions.getX(a); const az = positions.getZ(a);
    const bx = positions.getX(b); const bz = positions.getZ(b);
    const cx = positions.getX(c); const cz = positions.getZ(c);
    const denominator = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
    if (Math.abs(denominator) < 1e-10) continue;
    const weightA = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / denominator;
    const weightB = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / denominator;
    const weightC = 1 - weightA - weightB;
    if (weightA >= -1e-6 && weightB >= -1e-6 && weightC >= -1e-6) {
      return weightA * positions.getY(a) + weightB * positions.getY(b) + weightC * positions.getY(c);
    }
  }
  return null;
}

function geometryCoversXZ(geometry: ReturnType<typeof shorelineTransitionGeometry>, x: number, z: number): boolean {
  const positions = geometry.getAttribute("position");
  const indices = geometry.index;
  if (indices === null) return false;
  for (let offset = 0; offset < indices.count; offset += 3) {
    const a = indices.getX(offset);
    const b = indices.getX(offset + 1);
    const c = indices.getX(offset + 2);
    const ax = positions.getX(a); const az = positions.getZ(a);
    const bx = positions.getX(b); const bz = positions.getZ(b);
    const cx = positions.getX(c); const cz = positions.getZ(c);
    const denominator = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
    if (Math.abs(denominator) < 1e-10) continue;
    const weightA = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / denominator;
    const weightB = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / denominator;
    const weightC = 1 - weightA - weightB;
    if (weightA >= -1e-5 && weightB >= -1e-5 && weightC >= -1e-5) return true;
  }
  return false;
}

function expectedShoreSamples(points: readonly (readonly number[])[], closed: boolean): number {
  if (points.length < 2) return 0;
  let count = 0;
  const segments = closed ? points.length : points.length - 1;
  for (let index = 0; index < segments; index++) {
    const start = points[index];
    const end = points[(index + 1) % points.length];
    if (start === undefined || end === undefined) continue;
    const northDelta = (end[0] ?? 0) - (start[0] ?? 0);
    const eastDelta = (end[1] ?? 0) - (start[1] ?? 0);
    count += Math.max(1, Math.ceil(Math.hypot(northDelta, eastDelta) / 30));
  }
  return count + (closed ? 0 : 1);
}

describe("shoreline terrain transition", () => {
  it("connects every open-shore sample and mapped island samples to rendered terrain", () => {
    const shorelineAsset = shoreline as unknown as Parameters<typeof shorelineTransitionGeometry>[0];
    const requiredShorelineSamples = shoreline.shorelinesNorthEastMeters.reduce((sum, line) => sum + expectedShoreSamples(line, false), 0);
    const rockWayIds = new Set([41039534, 41039535, 41039553, 41039509]);
    const terrainBackedIslandSamples = shoreline.islands
      .filter((island) => !island.wayIds.some((id) => rockWayIds.has(id)))
      .reduce((sum, island) => sum + expectedShoreSamples(island.ringNorthEastMeters, true), 0);
    const required = requiredShorelineSamples + terrainBackedIslandSamples;
    const geometry = shorelineTransitionGeometry(shorelineAsset, [terrain, ...terrain.finePatches]);
    const position = geometry.getAttribute("position");
    const indices = geometry.index;
    expect(indices).not.toBeNull();
    if (indices === null) throw new TypeError("Shoreline transition indices are missing");
    const referencedVertices = new Uint8Array(position.count);
    for (let offset = 0; offset < indices.count; offset++) referencedVertices[indices.getX(offset)] = 1;
    let represented = 0;
    for (let start = 0; start + 3 < position.count; start += 4) {
      if (referencedVertices[start] === 1 && referencedVertices[start + 1] === 1 &&
          referencedVertices[start + 2] === 1 && referencedVertices[start + 3] === 1) represented++;
    }
    expect(required).toBeGreaterThan(0);
    expect(represented).toBe(required);
    const openCoast = shorelineTransitionGeometry({
      schemaVersion: 2,
      shorelinesNorthEastMeters: shoreline.shorelinesNorthEastMeters as unknown as readonly (readonly (readonly [number, number])[])[],
      islands: []
    }, [terrain, ...terrain.finePatches]);
    const coastPositions = openCoast.getAttribute("position");
    const coastIndices = openCoast.index;
    expect(coastPositions.count / 4).toBe(requiredShorelineSamples);
    expect(coastIndices).not.toBeNull();
    if (coastIndices !== null) {
      const coastReferencedVertices = new Uint8Array(coastPositions.count);
      for (let offset = 0; offset < coastIndices.count; offset++) coastReferencedVertices[coastIndices.getX(offset)] = 1;
      expect(coastReferencedVertices.every((value) => value === 1)).toBe(true);
    }
    openCoast.dispose();
    geometry.dispose();
  }, 20_000);

  it("connects every sampled point along the launch shoreline", () => {
    const launchShoreline = shoreline.shorelinesNorthEastMeters[33];
    if (launchShoreline === undefined) throw new TypeError("Missing launch shoreline source line");
    const geometry = shorelineTransitionGeometry({
      schemaVersion: 2,
      shorelinesNorthEastMeters: [launchShoreline as unknown as readonly (readonly [number, number])[]],
      islands: []
    }, [terrain, ...terrain.finePatches]);
    expect(geometry.getAttribute("position").count / 4).toBe(expectedShoreSamples(launchShoreline, false));
    geometry.dispose();
  }, 20_000);

  it("joins the Chikubushima outline to its detailed DSM surface", () => {
    const island = shoreline.islands.find((entry) => entry.name === "竹生島");
    const patch = terrain.finePatches.find((entry) => entry.id === "terrain-patch-chikubushima");
    expect(island).toBeDefined();
    expect(patch?.terrainStepMeters).toBe(30);
    if (island === undefined || patch === undefined) throw new TypeError("Missing Chikubushima terrain inputs");
    const islandOutline = island as unknown as Parameters<typeof shorelineTransitionGeometry>[0]["islands"][number];

    const elevations = patch.elevationMetersAboveWater.reduce<number[]>((values, height) => {
      if (typeof height === "number") values.push(height);
      return values;
    }, []);
    expect(elevations.length).toBeGreaterThan(100);
    let minimumElevation = Number.POSITIVE_INFINITY;
    let maximumElevation = Number.NEGATIVE_INFINITY;
    for (const elevation of elevations) {
      minimumElevation = Math.min(minimumElevation, elevation);
      maximumElevation = Math.max(maximumElevation, elevation);
    }
    expect(maximumElevation - minimumElevation).toBeGreaterThan(50);

    let expectedSamples = 0;
    for (let index = 0; index < islandOutline.ringNorthEastMeters.length; index++) {
      const start = islandOutline.ringNorthEastMeters[index];
      const end = islandOutline.ringNorthEastMeters[(index + 1) % islandOutline.ringNorthEastMeters.length];
      if (start === undefined || end === undefined) continue;
      expectedSamples += Math.max(1, Math.ceil(Math.hypot(end[0] - start[0], end[1] - start[1]) / 30));
    }
    const transition = shorelineTransitionGeometry({
      schemaVersion: 2,
      shorelinesNorthEastMeters: [],
      islands: [islandOutline]
    }, [terrain, ...terrain.finePatches]);
    expect(transition.getAttribute("position").count / 4).toBe(expectedSamples);
    transition.dispose();
  });

  it("keeps a continuous transition around every mapped major island", () => {
    const majorIslandNames = new Set(["多景島", "竹生島", "沖島", "オコノ洲"]);
    const majorIslands = shoreline.islands.filter((island) => island.name !== null && majorIslandNames.has(island.name));
    expect(majorIslands).toHaveLength(majorIslandNames.size);
    for (const island of majorIslands) {
      const islandName = island.name ?? "unnamed island";
      let expectedSamples = 0;
      for (let index = 0; index < island.ringNorthEastMeters.length; index++) {
        const start = island.ringNorthEastMeters[index];
        const end = island.ringNorthEastMeters[(index + 1) % island.ringNorthEastMeters.length];
        if (start === undefined || end === undefined) continue;
        expectedSamples += Math.max(1, Math.ceil(Math.hypot((end[0] ?? 0) - (start[0] ?? 0), (end[1] ?? 0) - (start[1] ?? 0)) / 30));
      }
      const geometry = shorelineTransitionGeometry({
        schemaVersion: 2,
        shorelinesNorthEastMeters: [],
        islands: [island as unknown as Parameters<typeof shorelineTransitionGeometry>[0]["islands"][number]]
      }, [terrain, ...terrain.finePatches]);
      expect(geometry.getAttribute("position").count / 4, `${islandName} shoreline samples`).toBe(expectedSamples);
      const referenced = new Uint8Array(geometry.getAttribute("position").count);
      const indices = geometry.index;
      if (indices === null) throw new TypeError(`${islandName} shoreline transition indices are missing`);
      for (let offset = 0; offset < indices.count; offset++) referenced[indices.getX(offset)] = 1;
      expect(referenced.every((value) => value === 1), `${islandName} transition vertices are connected`).toBe(true);
      geometry.dispose();
    }
  }, 20_000);

  it("uses source DSM samples to close the small wooded islet locally", () => {
    const island = shoreline.islands.find((entry) => entry.wayIds.includes(1156534005));
    const patch = terrain.finePatches.find((entry) => entry.id === "terrain-patch-wooded-islet-1156534005");
    expect(island?.name).toBeNull();
    expect(patch).toMatchObject({ terrainStepMeters: 10, rows: 11, columns: 14 });
    if (island === undefined || patch === undefined) throw new TypeError("Missing OSM wooded-islet source patch");
    const landSamples = patch.elevationMetersAboveWater.filter((height, index) => patch.landMask?.[index] && height !== null);
    expect(landSamples.length).toBeGreaterThanOrEqual(6);
    expect(landSamples.every((height) => (height ?? -1) >= 0)).toBe(true);
    const terrainMesh = terrainGeometry(patch, [], [terrain, ...terrain.finePatches]);
    expect(terrainMesh.index?.count).toBeGreaterThan(0);
    const transition = shorelineTransitionGeometry({
      schemaVersion: 2,
      shorelinesNorthEastMeters: [],
      islands: [island as unknown as Parameters<typeof shorelineTransitionGeometry>[0]["islands"][number]]
    }, [terrain, ...terrain.finePatches]);
    const positions = transition.getAttribute("position");
    let maximumWidth = 0;
    for (let index = 0; index < positions.count; index += 4) {
      maximumWidth = Math.max(maximumWidth,
        2 * Math.hypot(positions.getX(index + 3) - positions.getX(index + 2),
          positions.getZ(index + 3) - positions.getZ(index + 2)));
    }
    expect(maximumWidth).toBeLessThan(120);
    terrainMesh.dispose();
    transition.dispose();
  });

  it("fills the four Okinoshiraishi OSM outlines with a submerged, raised render-only rock mesh", () => {
    const geometry = okinoshiraishiRockGeometry(shoreline.islands as unknown as Parameters<typeof okinoshiraishiRockGeometry>[0]);
    geometry.computeBoundingBox();
    const positions = geometry.getAttribute("position");
    const colors = geometry.getAttribute("color");
    const indices = geometry.index;
    const rockWayIds = new Set([41039534, 41039535, 41039553, 41039509]);
    const sourceFootprint = new Set(shoreline.islands
      .filter((island) => island.wayIds.some((id) => rockWayIds.has(id)))
      .flatMap((island) => island.ringNorthEastMeters.flatMap((point) => {
        const north = point[0];
        const east = point[1];
        return typeof north === "number" && typeof east === "number"
          ? [`${String(Math.fround(east))},${String(Math.fround(-north))}`]
          : [];
      })));
    const renderedFootprint = new Set(Array.from({ length: positions.count }, (_, index) => index)
      .filter((index) => positions.getY(index) === -1.5)
      .map((index) => `${String(positions.getX(index))},${String(positions.getZ(index))}`));
    expect(positions.count).toBeGreaterThan(40);
    expect(renderedFootprint).toEqual(sourceFootprint);
    expect(colors.count).toBe(positions.count);
    expect(indices?.count).toBeGreaterThan(100);
    expect(geometry.boundingBox?.min.y).toBeCloseTo(-1.5, 5);
    expect(geometry.boundingBox?.max.y).toBeCloseTo(14, 5);
    for (let index = 0; index < positions.count; index++) {
      expect(Number.isFinite(positions.getX(index))).toBe(true);
      expect(Number.isFinite(positions.getY(index))).toBe(true);
      expect(Number.isFinite(positions.getZ(index))).toBe(true);
    }
    if (indices !== null) {
      for (let index = 0; index < indices.count; index++) {
        expect(indices.getX(index)).toBeLessThan(positions.count);
      }
    }
    geometry.dispose();
  });

  it("always grows into island land, independent of OSM ring winding", () => {
    const ring = [[-60, -60], [-60, 60], [60, 60], [60, -60]] as const;
    const islandGrid = {
      terrainStepMeters: 30,
      northMinMeters: -120,
      eastMinMeters: -120,
      columns: 9,
      rows: 9,
      elevationMetersAboveWater: Array.from({ length: 81 }, (_, index) => {
        const row = Math.floor(index / 9);
        const column = index % 9;
        const north = -120 + (8 - row) * 30;
        const east = -120 + column * 30;
        return Math.abs(north) <= 30 && Math.abs(east) <= 30 ? 3 : null;
      })
    };
    for (const islandRing of [ring, [...ring].reverse()]) {
      const geometry = shorelineTransitionGeometry({
        schemaVersion: 2,
        shorelinesNorthEastMeters: [],
        islands: [{ name: "test island", ringNorthEastMeters: islandRing }]
      }, [islandGrid]);
      const positions = geometry.getAttribute("position");
      expect(positions.count).toBeGreaterThan(12);
      for (let index = 3; index < positions.count; index += 4) {
        expect(Math.abs(positions.getX(index))).toBeLessThanOrEqual(30);
        expect(Math.abs(positions.getZ(index))).toBeLessThanOrEqual(30);
        expect(positions.getY(index)).toBeCloseTo(2.85, 3);
      }
      expect(Math.min(...Array.from({ length: positions.count / 4 }, (_, sample) => positions.getY(sample * 4)))).toBe(-8);
      geometry.dispose();
    }
  });

  it("uses OSM outer-water geometry to select the land side of open shores", () => {
    const line = [[0, -60], [0, 60]] as const;
    const southLand = {
      terrainStepMeters: 30,
      northMinMeters: -120,
      eastMinMeters: -120,
      columns: 9,
      rows: 9,
      elevationMetersAboveWater: Array.from({ length: 81 }, (_, index) => {
        const row = Math.floor(index / 9);
        const north = 120 - row * 30;
        return north <= 0 ? 2 : null;
      })
    };
    const southSide = shorelineTransitionGeometry({
      schemaVersion: 3,
      shorelinesNorthEastMeters: [line],
      shorelineLandSideSigns: [-1],
      islands: []
    }, [southLand]);
    const oppositeSide = shorelineTransitionGeometry({
      schemaVersion: 3,
      shorelinesNorthEastMeters: [line],
      shorelineLandSideSigns: [1],
      islands: []
    }, [southLand]);
    expect(southSide.getAttribute("position").count).toBeGreaterThan(0);
    expect(oppositeSide.getAttribute("position").count).toBe(0);
    southSide.dispose();
    oppositeSide.dispose();
  });

  it("ramps from the lake datum across masked coastal cells to sampled land", () => {
    const shoreline = {
      schemaVersion: 2 as const,
      shorelinesNorthEastMeters: [[[0, -30], [0, 30]] as const],
      islands: []
    };
    const grid = {
      terrainStepMeters: 30,
      northMinMeters: 0,
      eastMinMeters: -30,
      columns: 3,
      rows: 8,
      elevationMetersAboveWater: Array.from({ length: 24 }, (_, index) => index < 15 ? 4 : null)
    };
    const geometry = shorelineTransitionGeometry(shoreline, [grid]);
    geometry.computeBoundingBox();
    const position = geometry.getAttribute("position");
    const renderedTerrain = terrainGeometry(grid);
    const indices = geometry.index;
    expect(indices).not.toBeNull();
    expect(position.count).toBeGreaterThan(8);
    expect(indices?.count).toBe(36);
    const referencedVertices = new Set<number>();
    if (indices !== null) {
      for (let offset = 0; offset < indices.count; offset++) referencedVertices.add(indices.getX(offset));
    }
    expect(referencedVertices.size).toBe(position.count);
    expect(geometry.boundingBox?.max.z).toBeGreaterThanOrEqual(240);
    expect(geometry.boundingBox?.min.z).toBeLessThanOrEqual(-80);
      expect(geometry.boundingBox?.min.y).toBeCloseTo(-8);
    expect(geometry.boundingBox?.max.y).toBeCloseTo(3.85);
    for (let index = 3; index < position.count; index += 4) {
      const terrainHeight = renderedTerrainHeightAt(renderedTerrain, position.getX(index), position.getZ(index));
      expect(terrainHeight, `outer shoreline vertex ${String(index)}`).not.toBeNull();
      if (terrainHeight !== null) expect(position.getY(index) - terrainHeight).toBeCloseTo(-0.15, 3);
    }
    for (let index = 0; index < position.count; index += 4) {
      expect(position.getY(index), `submerged overlap vertex ${String(index)}`).toBe(-8);
      expect(position.getY(index + 1), `submerged shoreline vertex ${String(index + 1)}`).toBe(-8);
    }
    if (indices !== null) {
      for (let offset = 0; offset < indices.count; offset += 3) {
        const a = indices.getX(offset);
        const b = indices.getX(offset + 1);
        const c = indices.getX(offset + 2);
        const normalY = (position.getZ(b) - position.getZ(a)) * (position.getX(c) - position.getX(a)) -
          (position.getX(b) - position.getX(a)) * (position.getZ(c) - position.getZ(a));
        expect(normalY).toBeGreaterThan(0);
      }
    }
    geometry.dispose();
    renderedTerrain.dispose();
  });

  it("covers the actual launch-shoreline nodata band with upward-facing terrain", () => {
    const launchShoreline = shoreline.shorelinesNorthEastMeters[33];
    if (launchShoreline === undefined) throw new TypeError("Missing launch shoreline source line");
    const geometry = shorelineTransitionGeometry({
      schemaVersion: 2,
      shorelinesNorthEastMeters: [launchShoreline as unknown as readonly (readonly [number, number])[]],
      islands: []
    }, [terrain, ...terrain.finePatches]);
    const position = geometry.getAttribute("position");
    const indices = geometry.index;
    const regionalShorePatch = terrain.finePatches.find((patch) => patch.id === "terrain-patch-regional-shore");
    expect(regionalShorePatch).toBeDefined();
    expect(position.count).toBeGreaterThan(100);
    expect(indices?.count).toBeGreaterThan(100);
    if (regionalShorePatch === undefined) throw new TypeError("Missing detailed local shoreline terrain");
    const regionalShoreTerrain = terrainGeometry(regionalShorePatch, [], [terrain, ...terrain.finePatches]);
    const patchNorthMax = regionalShorePatch.northMinMeters + (regionalShorePatch.rows - 1) * regionalShorePatch.terrainStepMeters;
    const patchEastMax = regionalShorePatch.eastMinMeters + (regionalShorePatch.columns - 1) * regionalShorePatch.terrainStepMeters;
    let checkedOuterVertices = 0;
    let unjoinedOuterVertices = 0;
    let submergedUnderlapVertices = 0;
    let maximumLocalTransitionWidth = 0;
    let widestLaunchShoreSample: readonly [number, number] | null = null;
    for (let index = 0; index < position.count; index += 4) {
      const transitionWidth = Math.hypot(position.getX(index + 3) - position.getX(index + 2),
        position.getZ(index + 3) - position.getZ(index + 2)) * 2;
      if (transitionWidth > maximumLocalTransitionWidth) {
        maximumLocalTransitionWidth = transitionWidth;
        widestLaunchShoreSample = [position.getX(index), -position.getZ(index)];
      }
      const east = position.getX(index);
      const north = -position.getZ(index);
      if (north >= regionalShorePatch.northMinMeters && north <= patchNorthMax &&
          east >= regionalShorePatch.eastMinMeters && east <= patchEastMax && position.getY(index) < 0) {
        submergedUnderlapVertices++;
      }
    }
    for (let index = 3; index < position.count; index += 4) {
      const east = position.getX(index);
      const north = -position.getZ(index);
      if (north < regionalShorePatch.northMinMeters || north > patchNorthMax ||
          east < regionalShorePatch.eastMinMeters || east > patchEastMax) continue;
      checkedOuterVertices++;
      if (position.getY(index) < 0) continue;
      const renderedHeight = renderedTerrainHeightAt(regionalShoreTerrain, east, position.getZ(index));
      if (renderedHeight === null || Math.abs(position.getY(index) - renderedHeight + 0.15) > 0.003) {
        unjoinedOuterVertices++;
      }
    }
    expect(checkedOuterVertices).toBeGreaterThan(100);
    expect(submergedUnderlapVertices).toBeGreaterThan(100);
    expect(unjoinedOuterVertices).toBeLessThan(checkedOuterVertices * 0.01);
    expect(maximumLocalTransitionWidth, JSON.stringify(widestLaunchShoreSample)).toBeLessThanOrEqual(600);
    expect(checkedOuterVertices).toBeGreaterThan(unjoinedOuterVertices);
    if (indices !== null) {
      for (let offset = 0; offset < indices.count; offset += 3) {
        const a = indices.getX(offset);
        const b = indices.getX(offset + 1);
        const c = indices.getX(offset + 2);
        const normalY = (position.getZ(b) - position.getZ(a)) * (position.getX(c) - position.getX(a)) -
          (position.getX(b) - position.getX(a)) * (position.getZ(c) - position.getZ(a));
        expect(normalY).toBeGreaterThan(0);
      }
    }
    let checkedTransitionPanels = 0;
    for (let sample = 0; sample + 1 < position.count / 4; sample++) {
      for (let ring = 0; ring < 3; ring++) {
        const previous = sample * 4;
        const current = (sample + 1) * 4;
        const corners = [previous + ring, previous + ring + 1, current + ring, current + ring + 1];
        const centerX = corners.reduce((sum, vertex) => sum + position.getX(vertex), 0) / corners.length;
        const centerZ = corners.reduce((sum, vertex) => sum + position.getZ(vertex), 0) / corners.length;
        expect(geometryCoversXZ(geometry, centerX, centerZ), `transition panel ${String(sample)}:${String(ring)}`).toBe(true);
        checkedTransitionPanels++;
      }
    }
    expect(checkedTransitionPanels).toBeGreaterThan(500);
    geometry.dispose();
    regionalShoreTerrain.dispose();
  }, 20_000);

  it("bridges the broad DSM no-data band along the northern shore", () => {
    const northernShore = shoreline.shorelinesNorthEastMeters.find((line) =>
      line.some((point) => Math.hypot((point[0] ?? 0) - 9_300, (point[1] ?? 0) - 400) < 500)
    );
    expect(northernShore).toBeDefined();
    if (northernShore === undefined) throw new TypeError("Missing northern-shoreline source line");

    let expectedSamples = 1;
    for (let index = 0; index < northernShore.length - 1; index++) {
      const start = northernShore[index];
      const end = northernShore[index + 1];
      if (start === undefined || end === undefined) continue;
      expectedSamples += Math.max(1, Math.ceil(Math.hypot((end[0] ?? 0) - (start[0] ?? 0), (end[1] ?? 0) - (start[1] ?? 0)) / 30));
    }
    const geometry = shorelineTransitionGeometry({
      schemaVersion: 2,
      shorelinesNorthEastMeters: [northernShore as unknown as readonly (readonly [number, number])[]],
      islands: []
    }, [terrain, ...terrain.finePatches]);

    // Every sampled OSM coastline point needs a four-ring transition profile.
    // Previously 18 of 102 samples in this broad masked band had no collar.
    expect(geometry.getAttribute("position").count / 4).toBe(expectedSamples);
    geometry.dispose();
  }, 20_000);

  it("closes short open shoreline fragments at terminal DSM gaps", () => {
    const testShorelineSegments = [
      {
        name: "west shore",
        near: (north: number, east: number): boolean => north >= 2_500 && north <= 2_600 && east >= -16_100 && east <= -15_900
      },
      {
        name: "west shore with locally inverted land-side metadata",
        near: (north: number, east: number): boolean => north >= 2_500 && north <= 2_600 && east >= -16_600 && east <= -16_400
      },
      {
        name: "northwest shore near Chikubushima",
        near: (north: number, east: number): boolean => north >= 16_800 && north <= 17_000 && east >= -10_400 && east <= -10_200
      }
    ];
    for (const segment of testShorelineSegments) {
      const source = shoreline.shorelinesNorthEastMeters.find((line) =>
        line.some(([north, east]) => segment.near(north ?? 0, east ?? 0))
      );
      expect(source, `${segment.name} source`).toBeDefined();
      if (source === undefined) throw new TypeError(`Missing ${segment.name} source shoreline`);
      const geometry = shorelineTransitionGeometry({
        schemaVersion: 2,
        shorelinesNorthEastMeters: [source as unknown as readonly (readonly [number, number])[]],
        islands: []
      }, [terrain, ...terrain.finePatches]);
      const positions = geometry.getAttribute("position");
      expect(positions.count / 4, `${segment.name} connected samples`)
        .toBe(expectedShoreSamples(source, false));
      const referenced = new Uint8Array(positions.count);
      const indices = geometry.index;
      expect(indices).not.toBeNull();
      if (indices !== null) {
        for (let offset = 0; offset < indices.count; offset++) referenced[indices.getX(offset)] = 1;
      }
      expect(referenced.every((value) => value === 1), `${segment.name} mesh vertices are joined`).toBe(true);
      geometry.dispose();
    }
  }, 20_000);
});

describe("AW3D30 terrain mesh orientation", () => {
  it("densifies the sparse Okonozu DSM inside the mapped island outline", () => {
    const patch = terrain.finePatches.find((entry) => entry.id === "terrain-patch-okonozu");
    if (patch === undefined) throw new TypeError("Missing filled Okonozu terrain");
    expect(patch.terrainStepMeters).toBe(15);
    expect(patch.columns).toBe(81);
    expect(patch.rows).toBe(81);
    expect(patch.elevationMetersAboveWater.filter((height) => height !== null).length).toBeGreaterThan(50);
    expect(patch.landMask?.some((isLand, index) => isLand && patch.elevationMetersAboveWater[index] === null)).toBe(false);
  });

  it("removes DSM elevations marked as water before emitting triangles", () => {
    const geometry = terrainGeometry({
      terrainStepMeters: 30,
      northMinMeters: 0,
      eastMinMeters: 0,
      columns: 3,
      rows: 3,
      elevationMetersAboveWater: Array.from({ length: 9 }, () => 5),
      landMask: [true, true, true, true, false, true, true, true, true]
    });
    const indices = geometry.index;
    expect(indices?.count).toBe(6);
    if (indices !== null) {
      for (let offset = 0; offset < indices.count; offset++) expect(indices.getX(offset)).not.toBe(4);
    }
    geometry.dispose();
  });

  it("fills only short DSM holes classified as land", () => {
    const common = {
      terrainStepMeters: 30,
      northMinMeters: 0,
      eastMinMeters: 0,
      columns: 3,
      rows: 3,
      elevationMetersAboveWater: [5, 5, 5, 5, null, 5, 5, 5, 5]
    } as const;
    const land = terrainGeometry({ ...common, landMask: [true, true, true, true, true, true, true, true, true] });
    const water = terrainGeometry({ ...common, landMask: [true, true, true, true, false, true, true, true, true] });
    expect(land.index?.count).toBeGreaterThan(water.index?.count ?? 0);
    land.dispose();
    water.dispose();
  });

  it("maps north-to-south rows into negative Z and emits upward-facing triangles", () => {
    const geometry = terrainGeometry({
      terrainStepMeters: 10,
      northMinMeters: 0,
      eastMinMeters: 0,
      columns: 2,
      rows: 2,
      elevationMetersAboveWater: [1, 2, 3, 4]
    });
    const position = geometry.getAttribute("position");
    const index = geometry.index;
    expect(index).not.toBeNull();
    expect([position.getX(0), position.getY(0), position.getZ(0)]).toEqual([0, 1, -10]);
    expect([position.getX(1), position.getY(1), position.getZ(1)]).toEqual([10, 2, -10]);
    expect([position.getX(2), position.getY(2), position.getZ(2)]).toEqual([0, 3, -0]);
    if (index === null) throw new TypeError("Terrain indices are missing");
    for (let offset = 0; offset < index.count; offset += 3) {
      const a = index.getX(offset);
      const b = index.getX(offset + 1);
      const c = index.getX(offset + 2);
      const first = [position.getX(b) - position.getX(a), position.getY(b) - position.getY(a), position.getZ(b) - position.getZ(a)];
      const second = [position.getX(c) - position.getX(a), position.getY(c) - position.getY(a), position.getZ(c) - position.getZ(a)];
      const normalY = (first[2] ?? 0) * (second[0] ?? 0) - (first[0] ?? 0) * (second[2] ?? 0);
      expect(normalY).toBeGreaterThan(0);
    }
    geometry.dispose();
  });

  it("retains upward winding when a coarse triangle is clipped around a detail patch", async () => {
    const { subtractTerrainPatch } = await import("./terrain-patch-clip.js");
    const triangle = [
      { x: -1, y: -3, z: -1 },
      { x: -1, y: 3, z: 2 },
      { x: 2, y: 0, z: -1 }
    ] as const;
    const pieces = subtractTerrainPatch(triangle, { minX: 0, maxX: 1, minZ: 0, maxZ: 1 });
    for (const polygon of pieces) {
      for (let offset = 1; offset < polygon.length - 1; offset++) {
        const a = polygon[0]; const b = polygon[offset]; const c = polygon[offset + 1];
        if (a === undefined || b === undefined || c === undefined) continue;
        const normalY = (b.z - a.z) * (c.x - a.x) - (b.x - a.x) * (c.z - a.z);
        expect(normalY).toBeGreaterThanOrEqual(0);
      }
    }
  });

  it("keeps every emitted face upward across the actual broad terrain and detail patches", () => {
    const grids = [terrain, ...terrain.finePatches];
    for (const [gridIndex, grid] of grids.entries()) {
      const geometry = gridIndex === 0
        ? terrainGeometry(grid, terrain.finePatches)
        : terrainGeometry(grid);
      geometry.computeBoundingBox();
      const position = geometry.getAttribute("position");
      const indices = geometry.index;
      expect(indices).not.toBeNull();
      expect(position.count).toBeGreaterThan(0);
      expect(geometry.boundingBox?.min.x).toBeCloseTo(grid.eastMinMeters, 4);
      expect(geometry.boundingBox?.max.x).toBeCloseTo(
        grid.eastMinMeters + (grid.columns - 1) * grid.terrainStepMeters, 4
      );
      expect(geometry.boundingBox?.min.z).toBeCloseTo(
        -(grid.northMinMeters + (grid.rows - 1) * grid.terrainStepMeters), 4
      );
      expect(geometry.boundingBox?.max.z).toBeCloseTo(-grid.northMinMeters, 4);
      if (indices === null) throw new TypeError("Terrain indices are missing");
      let nonUpwardFaces = 0;
      for (let offset = 0; offset < indices.count; offset += 3) {
        const a = indices.getX(offset);
        const b = indices.getX(offset + 1);
        const c = indices.getX(offset + 2);
        const normalY = (position.getZ(b) - position.getZ(a)) * (position.getX(c) - position.getX(a)) -
          (position.getX(b) - position.getX(a)) * (position.getZ(c) - position.getZ(a));
        if (normalY <= 0) nonUpwardFaces++;
      }
      expect(nonUpwardFaces, `grid ${String(gridIndex)} non-upward faces`).toBe(0);
      geometry.dispose();
    }
  });

  it("blends the regional 30 m terrain edge into the broad 300 m surface", () => {
    const regional = terrain.finePatches.find((entry) => entry.id === "terrain-patch-regional-shore");
    expect(regional).toMatchObject({ terrainStepMeters: 30, blendWidthMeters: 1_800, blendTo: "broad-terrain" });
    if (regional === undefined) throw new TypeError("Missing regional terrain shell");
    const broadGeometry = terrainGeometry(terrain, terrain.finePatches);
    const regionalGeometry = terrainGeometry(regional, [], [terrain, ...terrain.finePatches]);
    const positions = regionalGeometry.getAttribute("position");
    let comparedEdges = 0;
    let maximumDelta = 0;
    for (let row = 0; row < regional.rows; row++) {
      for (let column = 0; column < regional.columns; column++) {
        if (row !== 0 && column !== 0 && row !== regional.rows - 1 && column !== regional.columns - 1) continue;
        const index = row * regional.columns + column;
        const north = regional.northMinMeters + (regional.rows - row - 1) * regional.terrainStepMeters;
        const east = regional.eastMinMeters + column * regional.terrainStepMeters;
        const broadHeight = renderedTerrainHeightAt(broadGeometry, east, -north);
        if (broadHeight === null) continue;
        comparedEdges++;
        maximumDelta = Math.max(maximumDelta, Math.abs(positions.getY(index) - broadHeight));
      }
    }
    expect(comparedEdges).toBeGreaterThan(100);
    expect(maximumDelta).toBeLessThan(0.11);
    broadGeometry.dispose();
    regionalGeometry.dispose();
  }, 20_000);

  it("clips the broad mesh under the detailed regional shoreline patch", () => {
    const regional = terrain.finePatches.find((entry) => entry.id === "terrain-patch-regional-shore");
    if (regional === undefined) throw new TypeError("Missing regional terrain grid");
    const broadGeometry = terrainGeometry(terrain, terrain.finePatches, [terrain, ...terrain.finePatches]);
    const regionalGeometry = terrainGeometry(regional, [], [terrain, ...terrain.finePatches]);
    const positions = regionalGeometry.getAttribute("position");
    const indices = regionalGeometry.index;
    expect(indices).not.toBeNull();
    if (indices === null) throw new TypeError("Regional terrain mesh has no triangles");
    let checked = 0;
    for (let offset = 0; offset < indices.count; offset += 3) {
      const a = indices.getX(offset);
      const b = indices.getX(offset + 1);
      const c = indices.getX(offset + 2);
      const east = (positions.getX(a) + positions.getX(b) + positions.getX(c)) / 3;
      const north = -(positions.getZ(a) + positions.getZ(b) + positions.getZ(c)) / 3;
      const inside = east > regional.eastMinMeters && east < regional.eastMinMeters + (regional.columns - 1) * regional.terrainStepMeters &&
        north > regional.northMinMeters && north < regional.northMinMeters + (regional.rows - 1) * regional.terrainStepMeters;
      if (!inside) continue;
      checked++;
      expect(geometryCoversXZ(broadGeometry, east, -north)).toBe(false);
      expect(geometryCoversXZ(regionalGeometry, east, -north)).toBe(true);
      if (checked >= 100) break;
    }
    expect(checked).toBeGreaterThan(0);
    broadGeometry.dispose();
    regionalGeometry.dispose();
  }, 20_000);

  it("uses matching terrain normals along a shared broad and detailed mesh edge", () => {
    const heightAt = (north: number, east: number): number => 24 + north * 0.018 + east * 0.027 + north * east * 0.0002;
    const broad = {
      terrainStepMeters: 60,
      northMinMeters: -240,
      eastMinMeters: -240,
      columns: 9,
      rows: 9,
      elevationMetersAboveWater: Array.from({ length: 81 }, (_, index) => {
        const row = Math.floor(index / 9);
        const column = index % 9;
        return heightAt(-240 + (8 - row) * 60, -240 + column * 60);
      })
    };
    const fine = {
      terrainStepMeters: 30,
      northMinMeters: -60,
      eastMinMeters: -60,
      columns: 5,
      rows: 5,
      elevationMetersAboveWater: Array.from({ length: 25 }, (_, index) => {
        const row = Math.floor(index / 5);
        const column = index % 5;
        const north = -60 + (4 - row) * 30;
        const east = -60 + column * 30;
        const edgeDistance = Math.min(row, 4 - row, column, 4 - column) * 30;
        const detail = Math.sin(north * 0.1) * Math.cos(east * 0.1) * Math.min(1, edgeDistance / 30);
        return heightAt(north, east) + detail;
      })
    };
    const broadGeometry = terrainGeometry(broad, [fine], [broad, fine]);
    const fineGeometry = terrainGeometry(fine, [], [broad, fine]);
    const sharedPosition = { east: -60, north: 0, height: heightAt(0, -60) };
    const normalAt = (geometry: ReturnType<typeof terrainGeometry>): readonly [number, number, number] => {
      const position = geometry.getAttribute("position");
      const normal = geometry.getAttribute("normal");
      for (let index = 0; index < position.count; index++) {
        if (Math.abs(position.getX(index) - sharedPosition.east) > 0.01 ||
            Math.abs(position.getY(index) - sharedPosition.height) > 0.01 ||
            Math.abs(position.getZ(index) + sharedPosition.north) > 0.01) continue;
        return [normal.getX(index), normal.getY(index), normal.getZ(index)];
      }
      throw new Error("Missing shared terrain boundary vertex");
    };
    const broadNormal = normalAt(broadGeometry);
    const fineNormal = normalAt(fineGeometry);
    expect(fineNormal[0]).toBeCloseTo(broadNormal[0], 5);
    expect(fineNormal[1]).toBeCloseTo(broadNormal[1], 5);
    expect(fineNormal[2]).toBeCloseTo(broadNormal[2], 5);
    broadGeometry.dispose();
    fineGeometry.dispose();
  });
});

describe("OSM shoreline structure geometry", () => {
  it.each(["pier", "quay", "breakwater"] as const)("builds finite, raised %s meshes from the mapped ways", (kind) => {
    const geometry = shoreStructureGeometry(kind);
    geometry.computeBoundingBox();
    const position = geometry.getAttribute("position");
    expect(position.count).toBeGreaterThan(0);
    expect(geometry.index?.count).toBeGreaterThan(0);
    expect(geometry.boundingBox).not.toBeNull();
    expect(geometry.boundingBox?.max.y).toBeGreaterThan(0.5);
    for (let index = 0; index < position.count; index++) {
      expect(Number.isFinite(position.getX(index))).toBe(true);
      expect(Number.isFinite(position.getY(index))).toBe(true);
      expect(Number.isFinite(position.getZ(index))).toBe(true);
    }
    geometry.dispose();
  });
});

describe("Takeshima canopy geometry", () => {
  it("fills the OSM island outline using DSM heights and deep-green vertex colors", () => {
    const island = shoreline.islands.find((entry) => entry.name === "多景島");
    const patch = sourceTerrain.finePatches.find((entry) => entry.id === "terrain-patch-takeshima");
    if (island === undefined || patch === undefined) throw new TypeError("Missing Takeshima source assets");
    const geometry = takeshimaCanopyGeometry(
      island.ringNorthEastMeters as unknown as readonly (readonly [number, number])[],
      patch
    );
    geometry.computeBoundingBox();
    const positions = geometry.getAttribute("position");
    const colors = geometry.getAttribute("color");
    const north: number[] = [];
    const east: number[] = [];
    for (const point of island.ringNorthEastMeters) {
      const northCoordinate = point[0];
      const eastCoordinate = point[1];
      if (northCoordinate !== undefined && eastCoordinate !== undefined) {
        north.push(northCoordinate);
        east.push(eastCoordinate);
      }
    }
    expect(positions.count).toBeGreaterThan(island.ringNorthEastMeters.length);
    expect(geometry.index?.count).toBeGreaterThan(0);
    expect(geometry.boundingBox?.min.x).toBeCloseTo(Math.min(...east), 3);
    expect(geometry.boundingBox?.max.x).toBeCloseTo(Math.max(...east), 3);
    expect(geometry.boundingBox?.min.z).toBeCloseTo(-Math.max(...north), 3);
    expect(geometry.boundingBox?.max.z).toBeCloseTo(-Math.min(...north), 3);
    expect(geometry.boundingBox?.max.y).toBeLessThanOrEqual(14.2);
    expect(colors.getX(0)).toBeLessThan(0.04);
    expect(colors.getY(0)).toBeGreaterThan(0.13);
    expect(colors.getY(0)).toBeLessThan(0.18);
    const indices = geometry.index;
    expect(indices).not.toBeNull();
    if (indices !== null) {
      for (let offset = 0; offset < indices.count; offset += 3) {
        const a = indices.getX(offset);
        const b = indices.getX(offset + 1);
        const c = indices.getX(offset + 2);
        const normalY = (positions.getZ(b) - positions.getZ(a)) * (positions.getX(c) - positions.getX(a)) -
          (positions.getX(b) - positions.getX(a)) * (positions.getZ(c) - positions.getZ(a));
        expect(normalY).toBeGreaterThan(0);
      }
    }
    for (let index = 0; index < positions.count; index++) {
      expect(Number.isFinite(positions.getX(index))).toBe(true);
      expect(Number.isFinite(positions.getY(index))).toBe(true);
      expect(Number.isFinite(positions.getZ(index))).toBe(true);
    }
    geometry.dispose();
  });
});
