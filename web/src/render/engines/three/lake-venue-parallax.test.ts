import { PerspectiveCamera, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import shoreline from "../../../../../assets/biwa-shoreline.json";
import terrain from "../../../../../assets/biwa-terrain.json";
import { cinematicCameraView } from "../../camera/cinematic-camera.js";
import { pilotEyePoseThree, SYNTHETIC_PILOT_EYE_POINT } from "../../camera/pilot-eye-point.js";
import { IDENTITY_POSE, composePose, multiplyQuaternion, quaternion } from "../../contracts/math.js";
import type { FlightRenderPose } from "../../contracts/runtime.js";
import { venueMapForScenario } from "../../../game/biwa-venue-map.js";
import { flightRelativePose } from "./flight-pose.js";
import type { FlightCameraMode } from "../../contracts/runtime.js";

interface Grid {
  readonly columns: number;
  readonly rows: number;
  readonly northMinMeters: number;
  readonly eastMinMeters: number;
  readonly terrainStepMeters: number;
  readonly elevationMetersAboveWater: readonly (number | null)[];
}

interface WorldPoint {
  readonly north: number;
  readonly east: number;
  readonly height: number;
}

function peakInGrid(grid: Grid, include: (point: WorldPoint) => boolean): WorldPoint {
  let peak: WorldPoint | null = null;
  for (let row = 0; row < grid.rows; row++) {
    for (let column = 0; column < grid.columns; column++) {
      const height = grid.elevationMetersAboveWater[row * grid.columns + column];
      if (height === null || height === undefined) continue;
      const point = {
        north: grid.northMinMeters + (grid.rows - row - 1) * grid.terrainStepMeters,
        east: grid.eastMinMeters + column * grid.terrainStepMeters,
        height
      };
      if (include(point) && (peak === null || point.height > peak.height)) peak = point;
    }
  }
  if (peak === null) throw new TypeError("No terrain sample in the requested view sector");
  return peak;
}

function cameraForBearing(bearingDegrees: number): PerspectiveCamera {
  return pilotCameraForFlight(0, 0, 12, bearingDegrees, 0, 0);
}

function projectedHorizontal(point: WorldPoint, camera: PerspectiveCamera): number {
  return new Vector3(point.east, point.height, -point.north).project(camera).x;
}

function cameraForVenueView(mode: "platform" | "shore" | "telephoto"): PerspectiveCamera {
  const venue = venueMapForScenario(1);
  if (venue === null) throw new TypeError("Missing Lake Biwa venue map");
  const flight = {
    datumPositionNed: { north: 0, east: 0, down: -10 },
    attitudeBodyToNed: { w: 1, x: 0, y: 0, z: 0 },
    pilotPositionMeters: 0,
    initialPilotPositionMeters: 0
  } as const;
  const view = cinematicCameraView(mode satisfies FlightCameraMode, flight, 0, venue.cameraPoints);
  if (view === null) throw new TypeError(`Missing ${mode} venue camera`);
  const camera = new PerspectiveCamera(view.verticalFieldOfViewDegrees, 16 / 9, 0.05, 100_000);
  camera.position.set(view.pose.position.x, view.pose.position.y, view.pose.position.z);
  camera.quaternion.set(view.pose.orientation.x, view.pose.orientation.y, view.pose.orientation.z, view.pose.orientation.w);
  camera.updateProjectionMatrix();
  camera.updateMatrixWorld();
  return camera;
}

function pilotCameraForFlight(
  north: number,
  east: number,
  height: number,
  headingDegrees: number,
  pitchDegrees: number,
  rollDegrees: number
): PerspectiveCamera {
  const heading = headingDegrees * Math.PI / 180;
  const pitch = pitchDegrees * Math.PI / 180;
  const roll = rollDegrees * Math.PI / 180;
  // NED heading rotates around down; positive pitch and roll follow the flight contract.
  const bodyToNed = multiplyQuaternion(
    multiplyQuaternion(
      quaternion(Math.cos(heading / 2), 0, 0, Math.sin(heading / 2)),
      quaternion(Math.cos(pitch / 2), 0, Math.sin(pitch / 2), 0)
    ),
    quaternion(Math.cos(roll / 2), Math.sin(roll / 2), 0, 0)
  );
  const flight: FlightRenderPose = {
    datumPositionNed: { north, east, down: -height },
    attitudeBodyToNed: bodyToNed,
    pilotPositionMeters: 0,
    initialPilotPositionMeters: 0
  };
  const aircraftPose = flightRelativePose(flight, IDENTITY_POSE);
  const eyePose = composePose(aircraftPose, pilotEyePoseThree(SYNTHETIC_PILOT_EYE_POINT, 0));
  const camera = new PerspectiveCamera(60, 16 / 9, 0.05, 100_000);
  camera.position.set(eyePose.position.x, eyePose.position.y, eyePose.position.z);
  camera.quaternion.set(eyePose.orientation.x, eyePose.orientation.y, eyePose.orientation.z, eyePose.orientation.w);
  camera.updateMatrixWorld();
  return camera;
}

function peakIsInView(point: WorldPoint, camera: PerspectiveCamera): boolean {
  const projected = new Vector3(point.east, point.height, -point.north).project(camera);
  return projected.z > -1 && projected.z < 1 && Math.abs(projected.x) < 1 && Math.abs(projected.y) < 1;
}

describe("Lake Biwa terrain parallax", () => {
  it("includes detailed DSM patches for Takeshima, Chikubushima, Okishima, and Okonozu", () => {
    const islands = [
      { name: "多景島", patchId: "terrain-patch-takeshima" },
      { name: "竹生島", patchId: "terrain-patch-chikubushima" },
      { name: "沖島", patchId: "terrain-patch-okishima" },
      { name: "オコノ洲", patchId: "terrain-patch-okonozu" }
    ];
    for (const island of islands) {
      const outline = shoreline.islands.find((entry) => entry.name === island.name);
      const patch = terrain.finePatches.find((entry) => entry.id === island.patchId);
      expect(outline?.ringNorthEastMeters.length, `${island.name} OSM island outline`).toBeGreaterThan(3);
      expect(patch?.terrainStepMeters, `${island.name} terrain resolution`).toBe(30);
      expect(patch?.elevationMetersAboveWater.filter((height) => height !== null).length,
        `${island.name} valid DSM samples`).toBeGreaterThan(0);
    }
  });

  it("keeps four source-resolved opposite-shore summits inside their detailed terrain patches", () => {
    const ridgePatchIds = [
      "terrain-patch-ridge-northwest",
      "terrain-patch-ridge-north",
      "terrain-patch-ridge-northwest-far",
      "terrain-patch-ridge-west"
    ];
    for (const patchId of ridgePatchIds) {
      const patch = terrain.finePatches.find((entry) => entry.id === patchId);
      expect(patch, `${patchId} source patch`).toBeDefined();
      if (patch === undefined) throw new TypeError(`Missing ${patchId}`);
      const summit = peakInGrid(patch, () => true);
      const patchCenterNorth = patch.northMinMeters + (patch.rows - 1) * patch.terrainStepMeters / 2;
      const patchCenterEast = patch.eastMinMeters + (patch.columns - 1) * patch.terrainStepMeters / 2;
      const summitOffset = Math.hypot(summit.north - patchCenterNorth, summit.east - patchCenterEast);
      const validSamples = patch.elevationMetersAboveWater.filter((height) => height !== null).length;

      expect(summit.height, `${patchId} summit elevation`).toBeGreaterThan(500);
      expect(summitOffset, `${patchId} summit alignment`).toBeLessThan(100);
      expect(validSamples, `${patchId} detailed DSM coverage`).toBe(patch.rows * patch.columns);
    }
  });

  it("retains distant terrain ridges in northwest, west, and northern view sectors", () => {
    const sectors = [
      { name: "northwest", center: 315, includes: (bearing: number): boolean => bearing >= 300 && bearing <= 330 },
      { name: "west", center: 270, includes: (bearing: number): boolean => bearing >= 250 && bearing <= 290 },
      { name: "north", center: 0, includes: (bearing: number): boolean => bearing >= 350 || bearing <= 40 }
    ];
    for (const sector of sectors) {
      const ridge = peakInGrid(terrain, (point) => {
        const distance = Math.hypot(point.north, point.east);
        const bearingDegrees = (Math.atan2(point.east, point.north) * 180 / Math.PI + 360) % 360;
        return distance >= 12_000 && distance <= 42_000 && sector.includes(bearingDegrees);
      });
      expect(ridge.height, `${sector.name} ridge peak`).toBeGreaterThan(500);
      const projected = new Vector3(ridge.east, ridge.height, -ridge.north).project(cameraForBearing(sector.center));
      expect(projected.x, `${sector.name} ridge horizontal projection`).toBeGreaterThan(-1);
      expect(projected.x, `${sector.name} ridge horizontal projection`).toBeLessThan(1);
      expect(projected.y, `${sector.name} ridge vertical projection`).toBeGreaterThan(-1);
      expect(projected.y, `${sector.name} ridge vertical projection`).toBeLessThan(1);
      expect(projected.z, `${sector.name} ridge depth`).toBeGreaterThan(-1);
      expect(projected.z, `${sector.name} ridge depth`).toBeLessThan(1);
    }
  });

  it("frames real Lake Biwa islands and opposite ridges in the configured venue camera views", () => {
    const landmarkGridIds = {
      Takeshima: "terrain-patch-takeshima",
      Okishima: "terrain-patch-okishima",
      "West ridge": "terrain-patch-ridge-west",
      "North ridge": "terrain-patch-ridge-north"
    } as const;
    const expectedInView = [
      { camera: "platform", landmark: "North ridge" },
      { camera: "shore", landmark: "Takeshima" },
      { camera: "shore", landmark: "West ridge" },
      { camera: "telephoto", landmark: "Okishima" },
      { camera: "telephoto", landmark: "West ridge" }
    ] as const;
    const cameraCache = new Map<string, PerspectiveCamera>();
    const peakCache = new Map<string, WorldPoint>();
    for (const { camera: mode, landmark } of expectedInView) {
      const gridId = landmarkGridIds[landmark];
      const grid = terrain.finePatches.find((patch) => patch.id === gridId);
      expect(grid, `${landmark} terrain patch`).toBeDefined();
      if (grid === undefined) throw new TypeError(`Missing ${gridId}`);
      let point = peakCache.get(landmark);
      if (point === undefined) {
        point = peakInGrid(grid, () => true);
        peakCache.set(landmark, point);
      }
      let camera = cameraCache.get(mode);
      if (camera === undefined) {
        camera = cameraForVenueView(mode);
        cameraCache.set(mode, camera);
      }
      expect(peakIsInView(point, camera), `${mode} camera includes ${landmark}`).toBe(true);
    }
  });

  it("keeps the island and opposite ridge world-fixed with stronger near-field parallax over 500 m", () => {
    const islandPatch = terrain.finePatches.find((patch) => patch.id === "terrain-patch-takeshima");
    if (islandPatch === undefined) throw new TypeError("Missing Takeshima detail patch");
    const nearIsland = peakInGrid(islandPatch, () => true);
    const farRidge = peakInGrid(terrain, (point) => {
      const distance = Math.hypot(point.north, point.east);
      const bearingDegrees = (Math.atan2(point.east, point.north) * 180 / Math.PI + 360) % 360;
      return distance >= 12_000 && distance <= 42_000 && bearingDegrees >= 300 && bearingDegrees <= 330;
    });

    const initialCamera = pilotCameraForFlight(0, 0, 10, 315, 0, 0);
    const movedCamera = pilotCameraForFlight(500 / Math.sqrt(2), -500 / Math.sqrt(2), 10, 315, 0, 0);
    const nearShift = Math.abs(projectedHorizontal(nearIsland, movedCamera) - projectedHorizontal(nearIsland, initialCamera));
    const farShift = Math.abs(projectedHorizontal(farRidge, movedCamera) - projectedHorizontal(farRidge, initialCamera));

    expect(nearIsland.height).toBeGreaterThan(0);
    expect(farRidge.height).toBeGreaterThan(500);
    expect(nearShift).toBeGreaterThan(farShift * 3);
    expect(farShift).toBeGreaterThan(0);
  });

  it("keeps the northwest ridge visible through pilot heading, pitch, roll, and altitude changes", () => {
    const ridgePatch = terrain.finePatches.find((patch) => patch.id === "terrain-patch-ridge-northwest");
    if (ridgePatch === undefined) throw new TypeError("Missing northwest ridge detail patch");
    const ridge = peakInGrid(ridgePatch, () => true);
    const states = [
      { heading: 315, pitch: 0, roll: 0, height: 10 },
      { heading: 310, pitch: -5, roll: 5, height: 10 },
      { heading: 320, pitch: 5, roll: -5, height: 10 },
      { heading: 315, pitch: 0, roll: 0, height: 30 }
    ] as const;

    for (const state of states) {
      const projected = new Vector3(ridge.east, ridge.height, -ridge.north)
        .project(pilotCameraForFlight(0, 0, state.height, state.heading, state.pitch, state.roll));
      expect(projected.x, `${JSON.stringify(state)} ridge horizontal position`).toBeGreaterThan(-0.45);
      expect(projected.x, `${JSON.stringify(state)} ridge horizontal position`).toBeLessThan(0.45);
      expect(projected.y, `${JSON.stringify(state)} ridge vertical position`).toBeGreaterThan(-0.45);
      expect(projected.y, `${JSON.stringify(state)} ridge vertical position`).toBeLessThan(0.45);
      expect(projected.z, `${JSON.stringify(state)} ridge depth`).toBeGreaterThan(-1);
      expect(projected.z, `${JSON.stringify(state)} ridge depth`).toBeLessThan(1);
    }
  });
});
