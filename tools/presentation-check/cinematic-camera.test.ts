import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { cinematicCameraView, worldLookAtPose } from "../../web/src/render/camera/cinematic-camera.js";
import { rotateVec3, vec3 } from "../../web/src/render/contracts/math.js";
import type { Quaternion } from "../../web/src/render/contracts/math.js";
import { syntheticVenueMapForScenario } from "../../web/src/game/synthetic-venue-map.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";

const flight: FlightRenderPose = Object.freeze({
  datumPositionNed: Object.freeze({ north: 0, east: 0, down: -10 }),
  attitudeBodyToNed: Object.freeze({ w: 1, x: 0, y: 0, z: 0 }),
  pilotPositionMeters: 0,
  initialPilotPositionMeters: 0
});
const points = syntheticVenueMapForScenario(1)?.cameraPoints ?? [];

describe("cinematic camera rigs", () => {
  it("binds fixed camera points to the versioned synthetic venue", () => {
    const venue = syntheticVenueMapForScenario(1);
    expect(venue?.origin).toBe("scenario-launch-composite-cg");
    expect(venue?.version).toBe(1);
    expect(venue?.cameraPointsSha256)
      .toBe(createHash("sha256").update(JSON.stringify(venue?.cameraPoints)).digest("hex"));
  });

  it("aims a fixed platform camera at the recorded aircraft position", () => {
    const view = cinematicCameraView("platform", flight, 0, points);
    expect(view?.pose.position).toEqual(vec3(-18, 14, 16));
    expect(view?.verticalFieldOfViewDegrees).toBe(60);
    if (view === null) throw new Error("Platform camera point is missing");
    const forward = rotateVec3(view.pose.orientation, vec3(0, 0, -1));
    const targetDirection = vec3(18, -4, -16);
    const magnitude = Math.hypot(targetDirection.x, targetDirection.y, targetDirection.z);
    expect(forward.x).toBeCloseTo(targetDirection.x / magnitude, 10);
    expect(forward.y).toBeCloseTo(targetDirection.y / magnitude, 10);
    expect(forward.z).toBeCloseTo(targetDirection.z / magnitude, 10);
  });

  it("uses deterministic tracking and orbit poses at the same record time", () => {
    expect(cinematicCameraView("side", flight, 2, points))
      .toEqual(cinematicCameraView("side", flight, 2, points));
    expect(cinematicCameraView("orbit", flight, 4, points))
      .toEqual(cinematicCameraView("orbit", flight, 4, points));
    expect(cinematicCameraView("orbit", flight, 5, points))
      .not.toEqual(cinematicCameraView("orbit", flight, 4, points));
    expect(cinematicCameraView("telephoto", flight, 5, points)?.verticalFieldOfViewDegrees).toBe(24);
  });

  it("limits fixed-camera tracking rate independently of seek order", () => {
    const track = [
      { timeSeconds: 0, northMeters: 0, eastMeters: 0, altitudeMeters: 10 },
      { timeSeconds: 0.1, northMeters: 200, eastMeters: 200, altitudeMeters: 10 }
    ];
    const start = cinematicCameraView("platform", flight, 0, points, track);
    const middle = cinematicCameraView("platform", flight, 0.05, points, track);
    const end = cinematicCameraView("platform", flight, 0.1, points, track);
    if (start === null || middle === null || end === null) throw new Error("Fixed camera pose is unavailable");
    const angularLimit = (2 * Math.PI / 3) * 0.05;
    expect(quaternionAngle(start.pose.orientation, middle.pose.orientation)).toBeLessThanOrEqual(angularLimit + 1.0e-9);
    expect(quaternionAngle(middle.pose.orientation, end.pose.orientation)).toBeLessThanOrEqual(angularLimit + 1.0e-9);
    expect(cinematicCameraView("platform", flight, 0.05, points, track)).toEqual(middle);
  });

  it("handles a vertical look direction and rejects a coincident target", () => {
    const vertical = worldLookAtPose(vec3(0, 20, 0), vec3(0, 0, 0));
    expect(vertical).not.toBeNull();
    if (vertical === null) throw new Error("Vertical look direction is unavailable");
    const forward = rotateVec3(vertical.orientation, vec3(0, 0, -1));
    expect(forward.y).toBeCloseTo(-1, 10);
    expect(worldLookAtPose(vec3(0, 0, 0), vec3(0, 0, 0))).toBeNull();
    expect(cinematicCameraView("platform", flight, 0, [])).toBeNull();
  });
});

function quaternionAngle(left: Quaternion, right: Quaternion): number {
  const cosine = Math.abs(left.w * right.w + left.x * right.x + left.y * right.y + left.z * right.z);
  return 2 * Math.acos(Math.min(1, cosine));
}
