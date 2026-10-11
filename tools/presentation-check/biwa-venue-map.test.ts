import { describe, expect, it } from "vitest";
import shoreline from "../../assets/biwa-shoreline.json";
import terrain from "../../assets/biwa-terrain.json";
import venueFeatures from "../../assets/biwa-venue-features.json";
import { LAUNCH_PLATFORM } from "../../web/src/render/contracts/launch-venue.js";
import { currentVenueEnvironmentFixture, currentVenueMapFixture } from "./current-venue-fixture.js";
import { venueMapForEnvironment } from "../../web/src/game/biwa-venue-map.js";

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

describe("Hikone venue map", () => {
  it("requires the recorded registered origin instead of a scenario-number fallback", () => {
    const environment = currentVenueEnvironmentFixture();
    if (environment.kind !== "available") throw new Error("Missing current environment fixture");
    expect(venueMapForEnvironment({ kind: "unavailable", reason: "unregistered_environment_identity" }))
      .toEqual({ kind: "unavailable", reason: "environment_unavailable" });
    expect(venueMapForEnvironment({ kind: "available", value: { ...environment.value,
      localFrame: { kind: "unavailable", reason: "origin_not_recorded" } } }))
      .toEqual({ kind: "unavailable", reason: "origin_not_recorded" });
    const frame = environment.value.localFrame;
    if (frame.kind !== "available") throw new Error("Missing registered origin");
    expect(venueMapForEnvironment({ kind: "available", value: { ...environment.value,
      localFrame: { kind: "available", value: { ...frame.value, longitudeDegrees: frame.value.longitudeDegrees + 0.1 } } } }))
      .toEqual({ kind: "unavailable", reason: "unregistered_origin" });
  });
  it("uses the launch platform's full dimensions and northwest orientation", () => {
    const projection = currentVenueMapFixture();
    if (projection.kind !== "available") throw new Error("Missing registered Lake Biwa venue");
    const venue = projection.value;
    const platform = venue.lines.find((line) => line.id === "launch-platform");
    expect(platform).toBeDefined();
    const points = platform?.points ?? [];
    expect(points).toHaveLength(5);
    const pointAt = (index: number): (typeof points)[number] => {
      const point = points[index];
      if (point === undefined) throw new TypeError(`Missing launch-platform corner ${String(index)}`);
      return point;
    };
    const frontLeft = pointAt(0);
    const frontRight = pointAt(1);
    const rearRight = pointAt(2);
    const rearLeft = pointAt(3);
    const closed = pointAt(4);
    expect(frontLeft).toEqual(closed);

    const distance = (a: (typeof points)[number], b: (typeof points)[number]): number =>
      Math.hypot(b.northMeters - a.northMeters, b.eastMeters - a.eastMeters);
    expect(distance(frontLeft, frontRight)).toBeCloseTo(LAUNCH_PLATFORM.widthMeters, 8);
    expect(distance(frontRight, rearRight)).toBeCloseTo(LAUNCH_PLATFORM.lengthMeters, 8);
    expect(distance(rearRight, rearLeft)).toBeCloseTo(LAUNCH_PLATFORM.widthMeters, 8);
    expect(distance(rearLeft, closed)).toBeCloseTo(LAUNCH_PLATFORM.lengthMeters, 8);

    const frontNorth = (frontLeft.northMeters + frontRight.northMeters) / 2;
    const frontEast = (frontLeft.eastMeters + frontRight.eastMeters) / 2;
    const rearNorth = (rearRight.northMeters + rearLeft.northMeters) / 2;
    const rearEast = (rearRight.eastMeters + rearLeft.eastMeters) / 2;
    expect(frontNorth - rearNorth).toBeGreaterThan(0);
    expect(frontEast - rearEast).toBeLessThan(0);
  });

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

  it("keeps the OSM venue features attributed and exposes Matsubara shore features on the map", () => {
    expect(venueFeatures.license).toBe("ODbL-1.0");
    expect(venueFeatures.attribution).toBe("© OpenStreetMap contributors");
    expect(venueFeatures.sourceSnapshotSha256).toMatch(/^[a-f0-9]{64}$/);
    expect(venueFeatures.features).toHaveLength(50);
    expect(venueFeatures.features.map((feature) => feature.kind)).toEqual(expect.arrayContaining([
      "beach", "woodland", "tree-row", "pier", "quay", "breakwater"
    ]));
    const projection = currentVenueMapFixture();
    if (projection.kind !== "available") throw new Error("Missing registered Lake Biwa venue");
    const venue = projection.value;
    expect(venue.sourceNote).toContain("ODbL-1.0");
    expect(venue.lines.some((line) => line.label === "松原水泳場")).toBe(true);
    expect(venue.lines.some((line) => line.label.startsWith("桟橋"))).toBe(true);
    expect(venue.lines.some((line) => line.label.startsWith("防波堤"))).toBe(true);
    expect(venue.landmarks.some((landmark) => landmark.label === "松原水泳場")).toBe(true);
  });
});
