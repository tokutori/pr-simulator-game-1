import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
afterEach(() => { vi.restoreAllMocks(); });
const neutral = ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" as const } });

function complete(session: AppSessionFacade): void {
  session.executeSharedSessionCommand({ kind: "open_setup" });
  session.executeSharedSessionCommand({ kind: "prepare" });
  session.executeSharedSessionCommand({ kind: "start_countdown" });
  session.observeCountdown(session.captureCountdownToken(), 3, true);
  session.flightPort.advanceTick(neutral);
  session.executeSharedSessionCommand({ kind: "abort" });
}

describe("layout-owned flight log query boundary with actual WASM", () => {
  it("reads the same current Result and Replay record without mutating its owner token", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      expect(() => session.readFlightLog("csv")).toThrow("InvalidTransition");
      complete(session);
      const token = session.captureQueryToken();
      const original = session.exportRecordJson();
      const csv = session.readFlightLog("csv");
      const columns = csv.slice(0, csv.indexOf("\n")).split(",");
      expect(columns).toEqual(expect.arrayContaining([
        "log_export_version", "record_schema_version", "control_layout",
        "physical_horizontal_tail_incidence_rad", "physical_vertical_tail_incidence_rad",
        "pilot_intent_nose_up_normalized", "pilot_intent_turn_right_normalized",
        "target_angular_rate_body_q_rad_s", "target_angular_rate_body_r_rad_s",
        "resolved_pilot_position_target_body_forward_m", "terminal_failure_available", "terminal_failure_json"
      ]));
      expect(csv.slice(csv.indexOf("\n") + 1)).toMatch(/^3,6,tail_incidence,/);
      expect(session.readFlightLog("json")).toBe(original);
      expect(session.acceptQuery(token, csv).kind).toBe("accepted");
      expect(session.readLifecycle().phase).toBe("result");
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      expect(session.readFlightLog("csv")).toBe(csv);
      expect(session.readFlightLog("json")).toBe(original);
      expect(session.readLifecycle().phase).toBe("replay");
    } finally { session.dispose(); }
  });


  it("preserves the current archive through failed replacement and clock queries", () => {
    const source = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    const owner = createAppSession({ controlModeCode: 0, seedLow: 23, seedHigh: 24 });
    try {
      complete(source);
      const original = ` \n${source.exportRecordJson()}\n `;
      owner.openArchive(original);
      const clock = owner.readPlaybackClock();
      const csv = owner.readFlightLog("csv");
      expect(owner.readFlightLog("json")).toBe(original);
      expect(owner.readPlaybackClock()).toEqual(clock);
      expect(() => owner.exportRecordJson()).toThrow();
      expect(() => { owner.openArchive("{invalid archive}"); }).toThrow();
      expect(owner.readFlightLog("json")).toBe(original);
      expect(owner.readFlightLog("csv")).toBe(csv);
    } finally { source.dispose(); owner.dispose(); }
  });

  it("preserves Rust Attract rejection and checks disposed ownership before invoking a producer export", () => {
    const owner = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    const csv = vi.spyOn(HybridGameSessionBridge.prototype, "export_flight_log_csv");
    const json = vi.spyOn(HybridGameSessionBridge.prototype, "export_current_flight_record_json");
    owner.executeSharedSessionCommand({ kind: "enter_attract" });
    expect(() => owner.readFlightLog("csv")).toThrow("InvalidTransition");
    expect(() => owner.readFlightLog("json")).toThrow("InvalidTransition");
    expect(csv).toHaveBeenCalledTimes(1);
    expect(json).toHaveBeenCalledTimes(1);
    csv.mockClear();
    json.mockClear();
    owner.dispose();
    expect(() => owner.readFlightLog("json")).toThrow("disposed");
    expect(csv).not.toHaveBeenCalled();
    expect(json).not.toHaveBeenCalled();
  });
});
