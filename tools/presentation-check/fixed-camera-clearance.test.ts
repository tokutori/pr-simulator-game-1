import { DoubleSide, Mesh, MeshBasicMaterial, Raycaster, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import analysisMap from "../../assets/biwa-analysis-map.json";
import landMask from "../../assets/biwa-land-mask.json";
import terrainAsset from "../../assets/biwa-terrain.json";
import { HybridGameSessionBridge, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";
import { applyLandMask, terrainGeometry } from "../../web/src/render/engines/three/lake-venue-mesh.js";
import "./lake-model-fixture.js";

function regionalTerrain(): Mesh {
  const terrain = applyLandMask(terrainAsset, landMask);
  const regional = terrain.finePatches.find((patch) => patch.id === "terrain-patch-regional-shore");
  if (regional === undefined) throw new Error("Missing regional terrain");
  return new Mesh(terrainGeometry(regional, [], [terrain, ...terrain.finePatches]),
    new MeshBasicMaterial({ side: DoubleSide }));
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
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const session = new TailAppSessionFacade(bridge, physics_hz());
    const mesh = regionalTerrain();
    try {
      bridge.open_setup();
      bridge.prepare();
      bridge.mark_briefing_ready();
      bridge.start_countdown(1);
      bridge.observe_countdown(bridge.countdown_token(), 1, true);
      const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
        desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });
      for (let tick = 0; tick < physics_hz() * 60 && bridge.phase_code() === 5; tick++) bridge.advance_tick_json(neutral);
      expect(bridge.phase_code()).toBe(7);
      session.executeOperation("enter-replay");
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(dataset.summary.durationSeconds).toBeGreaterThan(10);
      for (const time of [0, dataset.summary.durationSeconds / 2, dataset.summary.durationSeconds]) {
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
