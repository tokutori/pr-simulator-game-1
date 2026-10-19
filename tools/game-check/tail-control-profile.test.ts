import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import type { TailControlProfileDocument } from "../../web/src/generated/tail-control-profile.js";
import type { TailSnapshotDocument } from "../../web/src/generated/tail-snapshot.js";
import { tailInputFromControlProfile } from "../../web/src/game/tail-device-input.js";
import { readContract } from "../game-ui-build/validate.js";
import { tailGamepadDemand, tailKeyboardDemand } from "../../web/src/game/tail-device-input.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-input.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function preparedSession(mode = 0, seedLow = 11): HybridGameSessionBridge {
  const session = new HybridGameSessionBridge(mode, seedLow, 12);
  session.open_setup();
  session.prepare();
  return session;
}

describe("Rust-owned two-tail controller metadata", () => {
  it("uses sealed named limits for keyboard intent in every authority mode", () => {
    for (const mode of [0, 1, 2]) {
      const session = preparedSession(mode);
      try {
        const profile = readContract<TailControlProfileDocument>("tail_profile", session.control_profile_json());
        expect(profile.schema_version).toBe(2);
        session.mark_briefing_ready();
        session.start_countdown(1);
        session.observe_countdown(session.countdown_token(), 1, true);
        const demand = tailKeyboardDemand(new Set(["ArrowUp", "ArrowRight", "KeyL"])).demand;
        const input = tailInputFromControlProfile(demand, profile);
        expect(input.desiredPitchRateRadiansPerSecond).toBe(profile.desired_body_rate_limit_rad_s.pitch);
        expect(input.desiredYawRateRadiansPerSecond).toBe(profile.desired_body_rate_limit_rad_s.yaw);
        expect(input.pilotPositionCommand).toEqual(demand.pilotPositionCommand);
        const next = readContract<TailSnapshotDocument>("tail_snapshot", session.advance_tick_json(encodeTailLogicalInput(input)));
        if (next.frame.kind !== "flight") throw new Error("Expected flight frame");
        expect(next.frame.state.tick).toBe(1);
        expect(next.frame.state.physical_incidence.horizontal_tail_rad).toBeLessThan(0);
        expect(next.frame.state.physical_incidence.vertical_tail_rad).toBeLessThan(0);
      } finally {
        session.free();
      }
    }
  });

  it("maps fractional normalized demand from metadata and never uses feedback gain or slew as a rate limit", () => {
    const session = preparedSession();
    try {
      const original = readContract<TailControlProfileDocument>("tail_profile", session.control_profile_json());
      const profile = { ...original, desired_body_rate_limit_rad_s: { pitch: 0.071, yaw: 0.043 },
        feedback_gain_seconds: { pitch: 0.31, yaw: 0.27 }, maximum_slew_rad_s: 0.19 };
      const demand = tailGamepadDemand([{ connected: true, axes: [-0.54, -0.54, 0, 0] }]);
      if (demand === null) throw new Error("Expected device demand");
      const input = tailInputFromControlProfile(demand, profile);
      expect(input.desiredPitchRateRadiansPerSecond).toBe(0.5 * profile.desired_body_rate_limit_rad_s.pitch);
      expect(input.desiredYawRateRadiansPerSecond).toBe(-0.5 * profile.desired_body_rate_limit_rad_s.yaw);
      expect(input.pilotPositionCommand).toEqual({ kind: "set", normalized: 0 });
    } finally {
      session.free();
    }
  });

  it("keeps preparation availability in the Rust owner and bounds normalized device demand", () => {
    const session = preparedSession();
    try {
      const profile = readContract<TailControlProfileDocument>("tail_profile", session.control_profile_json());
      const demand = tailKeyboardDemand(new Set()).demand;
      expect(() => tailInputFromControlProfile({ ...demand, noseUp: 1.01 }, profile)).toThrow(RangeError);
      session.cancel_briefing();
      expect(() => session.control_profile_json()).toThrow();
      session.return_to_title();
      expect(() => session.control_profile_json()).toThrow();
    } finally { session.free(); }
  });
});
