import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { TailPreparedLaunchDocument } from "../../web/src/generated/tail-prepared.js";
import { readContract } from "../game-ui-build/validate.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutral = ({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" as const } });

describe("sealed initial launch presentation", () => {
  for (const mode of [0, 1, 2]) {
    it.each(["calm", "mild", "typical", "challenging", "near_limit"] as const)(`preserves mode ${String(mode)} and weather %s from preparation through launch and Retry`, (weather) => {
      const bridge = new HybridGameSessionBridge(mode, 21, 22);
      const session = new TailAppSessionFacade(bridge, physics_hz());
      try {
        expect(session.readPreparedLaunchPose()).toBeNull();
        session.executeSharedSessionCommand({ kind: "open_setup" });
        expect(session.readPreparedLaunchPose()).toBeNull();
        session.executeSharedSessionCommand({ kind: "select_weather", weather });
        session.executeSharedSessionCommand({ kind: "prepare" });
        const initial = session.readPreparedLaunchPose();
        expect(initial).not.toBeNull();
        expect(initial?.controls?.layout).toBe("tail_incidence");
        const before = bridge.snapshot_json();
        const prepared = readContract<TailPreparedLaunchDocument>("tail_prepared", bridge.prepared_launch_snapshot_json());
        expect(prepared.preview.kind).toBe("prepared");
        expect(bridge.snapshot_json()).toBe(before);
        expect(session.readLifecycle().phase).toBe("briefing_ready");
        session.executeSharedSessionCommand({ kind: "start_countdown" });
        for (let step = 0; step < 3; step++) {
          expect(session.readPreparedLaunchPose()).toEqual(initial);
          session.observeCountdown(session.captureCountdownToken(), 1, true);
        }
        expect(session.readPreparedLaunchPose()).toBeNull();
        const live = session.readSnapshot();
        if (initial === null || live.frame.kind !== "flight" || prepared.preview.kind !== "prepared") {
          throw new Error("Expected sealed preparation and its first live frame");
        }
        expect(live.frame.state).toEqual(prepared.preview.state);
        expect(live.render_pose).toEqual(initial);
        session.flightPort.advanceTick(neutral);
        session.executeSharedSessionCommand({ kind: "abort" });
        expect(session.readPreparedLaunchPose()).toBeNull();
        session.executeSharedSessionCommand({ kind: "retry" });
        expect(session.readPreparedLaunchPose()).toEqual(initial);
        session.executeSharedSessionCommand({ kind: "cancel_briefing" });
        expect(session.readPreparedLaunchPose()).toBeNull();
      } finally { session.dispose(); }
    });
  }

});
