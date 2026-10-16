import { describe, expect, it } from "vitest";
import { recordedCameraFixture } from "./recorded-camera-fixture.js";

describe("shared camera director through the Web owner", () => {
  it("keeps automatic VR Pilot, explicit external choice, and short-record terminal fallback", () => {
    const session = recordedCameraFixture();
    try {
      const dataset = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
      expect(session.queryReplayCamera(0, "auto", true, dataset).replayCamera.mode).toBe("platform");
      expect(session.queryReplayCamera(1, "auto", true, dataset).replayCamera.mode).toBe("chase");
      expect(session.queryReplayCamera(0, "auto", false, dataset).replayCamera.mode).toBe("pilot");
      expect(session.queryReplayCamera(0, "shore", false, dataset).replayCamera.mode).toBe("shore");
      const middle = session.queryReplayCamera(0.015, "platform", true, dataset);
      session.queryReplayCamera(0.03, "orbit", true, dataset);
      expect(session.queryReplayCamera(0.015, "platform", true, dataset)).toEqual(middle);
    } finally { session.dispose(); }
  });
});
