import { Mesh, Raycaster, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import analysisMap from "../../assets/biwa-analysis-map.json";
import { physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { launchCurrentSession, neutralTailInput } from "../game-check/current-session-fixture.js";
import { venueTerrainMeshes } from "../../web/src/render/engines/three/lake-venue-mesh.js";
import "./lake-model-fixture.js";

function regionalTerrain(): Mesh {
  const meshes = venueTerrainMeshes();
  const regional = meshes.find((mesh) => mesh.name === "lake-biwa-terrain-patch-regional-shore");
  for (const mesh of meshes) if (mesh !== regional) mesh.geometry.dispose();
  for (const material of new Set(meshes.map((mesh) => mesh.material))) {
    if (material !== regional?.material) material.dispose();
  }
  if (regional === undefined) throw new Error("Missing regional terrain");
  return regional;
}

function assertClearSight(mesh: Mesh, position: Vector3, target: Vector3, label: string): void {
  const direction = target.clone().sub(position);
  const distance = direction.length();
  const ray = new Raycaster(position, direction.normalize(), 0, distance);
  expect(ray.intersectObject(mesh), label).toHaveLength(0);
}

function disposeTerrain(mesh: Mesh): void {
  mesh.geometry.dispose();
  const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
  for (const material of materials) material.dispose();
}

describe("registered fixed camera terrain clearance", () => {
  it("clears the rendered regional terrain along the nominal 0/100/200 m launch course", () => {
    const mesh = regionalTerrain();
    try {
      for (const point of analysisMap.cameraPoints) {
        if (point.id === "platform") continue;
        const position = new Vector3(point.eastMeters, point.altitudeMeters, -point.northMeters);
        const down = new Raycaster(position.clone().setY(1_000), new Vector3(0, -1, 0));
        const ground = down.intersectObject(mesh)[0];
        expect(ground, `${point.id} terrain under camera`).toBeDefined();
        if (ground === undefined) throw new Error("Missing camera ground");
        expect(position.y - ground.point.y, `${point.id} ground clearance`).toBeGreaterThan(2);
        for (const [distance, altitude] of [[0, 10.5], [100, 6], [200, 0.35]] as const) {
          const horizontal = distance / Math.sqrt(2);
          const target = new Vector3(-horizontal, altitude - 2, -horizontal);
          assertClearSight(mesh, position.clone().setY(position.y - 2), target,
            `${point.id} nominal ${String(distance)} m course with 2 m vertical margin`);
        }
      }
    } finally { disposeTerrain(mesh); }
  }, 20_000);

  it("keeps shared fixed views clear at the start, middle, and terminal of a real default flight", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    const mesh = regionalTerrain();
    try {
      launchCurrentSession(session);
      for (let tick = 0; tick < physics_hz() * 60 && session.readSnapshot().phase === "flight_running"; tick++) session.flightPort.advanceTick(neutralTailInput);
      expect(session.readSnapshot().phase).toBe("result");
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(dataset.summary.duration_seconds).toBeGreaterThan(10);
      for (const time of [0, dataset.summary.duration_seconds / 2, dataset.summary.duration_seconds]) {
        for (const camera of ["shore", "telephoto"] as const) {
          const pose = session.queryReplayCamera(time, camera, true, dataset);
          const cinematic = pose.replayCamera.cinematic;
          if (cinematic === null) throw new Error(`Missing ${camera} shared camera`);
          const position = cinematic.pose.position;
          assertClearSight(mesh, new Vector3(position.x, position.y, position.z),
            new Vector3(pose.datumPositionNed.east, -pose.datumPositionNed.down, -pose.datumPositionNed.north),
            `${camera} actual record at ${String(time)} s`);
        }
      }
    } finally {
      disposeTerrain(mesh);
      session.dispose();
    }
  }, 20_000);
});
