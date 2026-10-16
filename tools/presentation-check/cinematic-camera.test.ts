import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { rotateVec3, vec3 } from "../../web/src/render/contracts/math.js";
import { currentVenueMapFixture } from "./current-venue-fixture.js";
import { recordedCameraFixture } from "./recorded-camera-fixture.js";

describe("shared recorded cinematic camera projection", () => {
  it("binds fixed camera points to the versioned registered Lake Biwa venue", () => {
    const venue = currentVenueMapFixture();
    if (venue.kind !== "available") throw new Error("Missing registered Lake Biwa venue");
    expect(venue.value.cameraPointsSha256).toBe(createHash("sha256").update(JSON.stringify(venue.value.cameraPoints)).digest("hex"));
  });

  it("projects complete recorded aircraft controls and absolute camera axes from one Rust query", () => {
    const session = recordedCameraFixture();
    try {
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      const frame = session.queryReplayCamera(0.015, "side", true, dataset);
      const sample = session.queryRecordSample(0.015);
      expect(frame.simulationTimeSeconds).toBe(sample.timeSeconds);
      expect(frame.controls).toEqual(sample.controls);
      expect(frame.airspeedMetersPerSecond).toBe(sample.state.telemetry.airspeedMetersPerSecond);
      expect(frame.windVelocityNedMetersPerSecond).toEqual({ north: sample.state.windAtCgNedMetersPerSecond[0],
        east: sample.state.windAtCgNedMetersPerSecond[1], down: sample.state.windAtCgNedMetersPerSecond[2] });
      expect(frame.tailGeometry?.kind).toBe("available");
      const camera = frame.replayCamera.cinematic;
      if (camera === null) throw new Error("Missing shared Side camera");
      expect(camera.pose.position).toEqual(vec3(frame.datumPositionNed.east + 30, -frame.datumPositionNed.down + 9, -frame.datumPositionNed.north - 4));
      expect(camera.verticalFieldOfViewDegrees).toBe(50);
      const forward = rotateVec3(camera.pose.orientation, vec3(0, 0, -1));
      const length = Math.hypot(30, 9, 4);
      expect(forward.x).toBeCloseTo(-30 / length, 10);
      expect(forward.y).toBeCloseTo(-9 / length, 10);
      expect(forward.z).toBeCloseTo(4 / length, 10);
    } finally { session.dispose(); }
  });
});
