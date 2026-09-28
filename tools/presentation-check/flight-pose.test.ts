import { describe, expect, it } from "vitest";
import { quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import { flightRelativePose } from "../../web/src/render/engines/three/flight-pose.js";

describe("Three.js flight pose conversion", () => {
  it("preserves a local pose at identity aircraft attitude", () => {
    const converted = flightRelativePose({
      datumPositionNed: Object.freeze({ north: 0, east: 0, down: 0 }),
      attitudeBodyToNed: quaternion(1, 0, 0, 0),
      pilotPositionMeters: 0,
      initialPilotPositionMeters: 0
    }, {
      position: vec3(0, 0, -1),
      orientation: quaternion(1, 0, 0, 0)
    });

    expect(converted.position.x).toBeCloseTo(0);
    expect(converted.position.y).toBeCloseTo(0);
    expect(converted.position.z).toBeCloseTo(-1);
    expect(converted.orientation.w).toBeCloseTo(1);
    expect(converted.orientation.x).toBeCloseTo(0);
    expect(converted.orientation.y).toBeCloseTo(0);
    expect(converted.orientation.z).toBeCloseTo(0);
  });

  it("maps NED position and a body yaw into Three.js world axes", () => {
    const yawQuarterTurn = Math.SQRT1_2;
    const converted = flightRelativePose({
      datumPositionNed: Object.freeze({ north: 1, east: 2, down: 3 }),
      attitudeBodyToNed: quaternion(yawQuarterTurn, 0, 0, yawQuarterTurn),
      pilotPositionMeters: 0,
      initialPilotPositionMeters: 0
    }, {
      position: vec3(0, 0, -1),
      orientation: quaternion(1, 0, 0, 0)
    });

    expect(converted.position.x).toBeCloseTo(3);
    expect(converted.position.y).toBeCloseTo(-3);
    expect(converted.position.z).toBeCloseTo(-1);
  });
});
