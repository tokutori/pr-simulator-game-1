import { describe, expect, it } from "vitest";
import { recordedCameraFixture } from "./recorded-camera-fixture.js";

describe("shared camera director through the Web owner", () => {
  it("keeps automatic VR Pilot, explicit external choice, and short-record terminal fallback", () => {
    const session = recordedCameraFixture();
    try {
      expect(session.queryReplayCamera(0, "auto", true).replayCamera.mode).toBe("platform");
      expect(session.queryReplayCamera(1, "auto", true).replayCamera.mode).toBe("chase");
      expect(session.queryReplayCamera(0, "auto", false).replayCamera.mode).toBe("pilot");
      expect(session.queryReplayCamera(0, "shore", false).replayCamera.mode).toBe("shore");
      const middle = session.queryReplayCamera(0.015, "platform", true);
      session.queryReplayCamera(0.03, "orbit", true);
      expect(session.queryReplayCamera(0.015, "platform", true)).toEqual(middle);
    } finally { session.dispose(); }
  });
});
