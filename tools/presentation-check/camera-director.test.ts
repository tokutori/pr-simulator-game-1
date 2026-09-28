import { describe, expect, it } from "vitest";
import { resolveReplayCameraMode } from "../../web/src/render/camera/camera-director.js";
import type { FlightAnalysisData } from "../../web/src/game/flight-record-query.js";

const analysis: FlightAnalysisData = Object.freeze({
  initialPilotPositionMeters: 0,
  samples: Object.freeze([
    sample(0, 10),
    sample(2, 9),
    sample(5, 6),
    sample(8, 2),
    sample(10, 0)
  ]),
  summary: Object.freeze({
    sampleCount: 5,
    durationSeconds: 10,
    maximumAltitudeMeters: 10,
    maximumAirspeedMetersPerSecond: 10,
    maximumGroundspeedMetersPerSecond: 10,
    maximumAngleOfAttackRadians: null,
    maximumAbsoluteRollRadians: 0,
    score: null,
    terminal: Object.freeze({ reason: "water-contact", disposition: "complete", timeSeconds: 10 })
  })
});

describe("replay camera director", () => {
  it("selects event-bound shots deterministically from record time", () => {
    expect(resolveReplayCameraMode("auto", analysis, 0.5, "screen")).toBe("chase");
    expect(resolveReplayCameraMode("auto", analysis, 4, "screen")).toBe("pilot");
    expect(resolveReplayCameraMode("auto", analysis, 7, "screen")).toBe("chase");
    expect(resolveReplayCameraMode("auto", analysis, 10, "screen")).toBe("chase");
  });

  it("produces the same camera at shared record times across frame rates", () => {
    const thirtyFpsTime = 120 / 30;
    const oneHundredTwentyFpsTime = 480 / 120;
    expect(thirtyFpsTime).toBe(oneHundredTwentyFpsTime);
    expect(resolveReplayCameraMode("auto", analysis, thirtyFpsTime, "screen"))
      .toBe(resolveReplayCameraMode("auto", analysis, oneHundredTwentyFpsTime, "screen"));
  });

  it("preserves manual choice and keeps automatic external views out of VR", () => {
    expect(resolveReplayCameraMode("chase", analysis, 4, "screen")).toBe("chase");
    expect(resolveReplayCameraMode("auto", analysis, 0, "webxr")).toBe("pilot");
    expect(resolveReplayCameraMode("auto", analysis, 0, "phone-vr")).toBe("pilot");
  });

  it("handles short records without indexing outside the sample range", () => {
    const short = Object.freeze({
      ...analysis,
      samples: Object.freeze([sample(0, 1), sample(0.4, 0)]),
      summary: Object.freeze({ ...analysis.summary, sampleCount: 2, durationSeconds: 0.4,
        terminal: Object.freeze({ reason: "water-contact" as const, disposition: "complete" as const, timeSeconds: 0.4 }) })
    });
    expect(resolveReplayCameraMode("auto", short, 0.2, "screen")).toBe("chase");
    expect(resolveReplayCameraMode("auto", short, 5, "screen")).toBe("chase");
  });
});

function sample(timeSeconds: number, altitudeMeters: number): FlightAnalysisData["samples"][number] {
  return Object.freeze({
    timeSeconds,
    northMeters: timeSeconds * 9,
    eastMeters: 0,
    altitudeMeters,
    airspeedMetersPerSecond: 9,
    groundspeedMetersPerSecond: 9,
    windNorthMetersPerSecond: 0,
    windEastMetersPerSecond: 0,
    windDownMetersPerSecond: 0,
    angleOfAttackRadians: null,
    sideslipRadians: null,
    rollRadians: 0,
    pitchRadians: 0,
    headingRadians: 0
  });
}
