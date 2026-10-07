import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { parseTailControlProfile, tailInputFromControlProfile } from "../../web/src/game/tail-control-profile.js";
import { tailGamepadDemand, tailKeyboardDemand } from "../../web/src/game/tail-device-input.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function preparedSession(mode = 0, seedLow = 11): HybridGameSessionBridge {
  const session = new HybridGameSessionBridge(mode, seedLow, 12);
  session.open_setup();
  session.prepare();
  return session;
}

function nested(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return parent[key] as Record<string, unknown>;
}

describe("Rust-owned two-tail controller metadata", () => {
  it("uses sealed named limits for keyboard intent in every authority mode", () => {
    for (const mode of [0, 1, 2]) {
      const session = preparedSession(mode);
      try {
        const preparing = parseTailSessionSnapshot(session.snapshot_json());
        const profile = parseTailControlProfile(session.control_profile_json(), preparing);
        expect(profile.identity).toBe(preparing.identity);
        expect(Object.isFrozen(profile.desiredBodyRateLimitRadiansPerSecond)).toBe(true);
        session.mark_briefing_ready();
        session.start_countdown(1);
        session.advance_countdown();
        const initial = parseTailSessionSnapshot(session.launch());
        const demand = tailKeyboardDemand(new Set(["ArrowUp", "ArrowRight", "KeyL"])).demand;
        const input = tailInputFromControlProfile(demand, profile, initial);
        expect(input.desiredPitchRateRadiansPerSecond).toBe(profile.desiredBodyRateLimitRadiansPerSecond.pitch);
        expect(input.desiredYawRateRadiansPerSecond).toBe(profile.desiredBodyRateLimitRadiansPerSecond.yaw);
        expect(input.pilotPositionCommand).toEqual(demand.pilotPositionCommand);
        const next = parseTailSessionSnapshot(session.advance_tick_json(encodeTailLogicalInput(input)));
        if (next.frame.kind !== "flight") throw new Error("Expected flight frame");
        expect(next.frame.state.tick).toBe(1);
        expect(next.frame.state.physicalIncidence.horizontalTailRadians).toBeLessThan(0);
        expect(next.frame.state.physicalIncidence.verticalTailRadians).toBeLessThan(0);
      } finally {
        session.free();
      }
    }
  });

  it("maps fractional normalized demand from metadata and never uses feedback gain or slew as a rate limit", () => {
    const session = preparedSession();
    try {
      const snapshot = parseTailSessionSnapshot(session.snapshot_json());
      const parsed: unknown = JSON.parse(session.control_profile_json());
      const document = parsed as Record<string, unknown>;
      document.desired_body_rate_limit_rad_s = { pitch: 0.071, yaw: 0.043 };
      document.feedback_gain_seconds = { pitch: 0.31, yaw: 0.27 };
      document.maximum_slew_rad_s = 0.19;
      const profile = parseTailControlProfile(JSON.stringify(document), snapshot);
      const demand = tailGamepadDemand([{ connected: true, axes: [-0.54, -0.54, 0, 0] }]);
      if (demand === null) throw new Error("Expected device demand");
      const input = tailInputFromControlProfile(demand, profile, snapshot);
      expect(input.desiredPitchRateRadiansPerSecond).toBe(0.5 * profile.desiredBodyRateLimitRadiansPerSecond.pitch);
      expect(input.desiredYawRateRadiansPerSecond).toBe(-0.5 * profile.desiredBodyRateLimitRadiansPerSecond.yaw);
      expect(input.pilotPositionCommand).toEqual({ kind: "set", normalized: 0 });
    } finally {
      session.free();
    }
  });

  it("rejects stale identity and unavailable preparation metadata without inheriting another session", () => {
    const session = preparedSession();
    const other = preparedSession(0, 99);
    try {
      const snapshot = parseTailSessionSnapshot(session.snapshot_json());
      const profile = parseTailControlProfile(session.control_profile_json(), snapshot);
      const demand = tailKeyboardDemand(new Set()).demand;
      expect(() => tailInputFromControlProfile(demand, profile, parseTailSessionSnapshot(other.snapshot_json()))).toThrow(RangeError);
      session.cancel_briefing();
      const setup = parseTailSessionSnapshot(session.snapshot_json());
      expect(() => session.control_profile_json()).toThrow();
      expect(() => parseTailControlProfile("{}", setup)).toThrow(RangeError);
      expect(() => tailInputFromControlProfile(demand, profile, setup)).toThrow(RangeError);
      session.return_to_title();
      expect(() => session.control_profile_json()).toThrow();
      expect(() => tailInputFromControlProfile(demand, profile, parseTailSessionSnapshot(session.snapshot_json()))).toThrow(RangeError);
    } finally {
      session.free();
      other.free();
    }
  });

  it("rejects wrong schema/layout, unknown axes, incompatible controller identity and invalid numeric metadata", () => {
    const session = preparedSession();
    try {
      const snapshot = parseTailSessionSnapshot(session.snapshot_json());
      const mutations: readonly ((document: Record<string, unknown>) => void)[] = [
        (document) => { document.schema_version = 1; }, (document) => { document.control_layout = "legacy_three_axis"; },
        (document) => { document.controller_profile_id = "another-controller"; },
        (document) => { document.controller_profile_version = 0; }, (document) => { document.controller_profile_version = 99; },
        (document) => { nested(document, "desired_body_rate_limit_rad_s").roll = 1; },
        (document) => { nested(document, "desired_body_rate_limit_rad_s").pitch = 0; },
        (document) => { nested(document, "desired_body_rate_limit_rad_s").yaw = -1; },
        (document) => { nested(document, "feedback_gain_seconds").pitch = -1; },
        (document) => { nested(document, "feedback_gain_seconds").yaw = null; },
        (document) => { document.maximum_slew_rad_s = 0; }, (document) => { document.maximum_slew_rad_s = Infinity; },
        (document) => { document.extra = true; }
      ];
      for (const mutate of mutations) {
        const parsed: unknown = JSON.parse(session.control_profile_json());
        const document = parsed as Record<string, unknown>;
        mutate(document);
        expect(() => parseTailControlProfile(JSON.stringify(document), snapshot)).toThrow(RangeError);
      }
      const profile = parseTailControlProfile(session.control_profile_json(), snapshot);
      expect(() => tailInputFromControlProfile({ ...tailKeyboardDemand(new Set()).demand, noseUp: 1.01 }, profile, snapshot)).toThrow(RangeError);
      expect(() => parseTailControlProfile(" ".repeat(2_049), snapshot)).toThrow(RangeError);
    } finally {
      session.free();
    }
  });
});
