import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
afterEach(() => { vi.restoreAllMocks(); });
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function complete(session: AppSessionFacade): void {
  session.executeOperation("open-setup");
  session.executeOperation("prepare");
  session.executeOperation("start-flight");
  while (session.advanceCountdown() > 0) continue;
  session.launch();
  session.flightPort.advance_tick_json(neutral);
  session.executeOperation("abort");
}

describe("layout-owned flight log query boundary with actual WASM", () => {
  it("reads the same current Result and Replay record without mutating its owner token", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      expect(() => session.readFlightLog("csv")).toThrow("Result or Replay");
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
      expect(csv.slice(csv.indexOf("\n") + 1)).toMatch(/^2,6,tail_incidence,/);
      expect(session.readFlightLog("json")).toBe(original);
      expect(session.acceptQuery(token, csv).kind).toBe("accepted");
      expect(session.readLifecycle().phaseCode).toBe(7);
      session.executeOperation("enter-replay");
      expect(session.readFlightLog("csv")).toBe(csv);
      expect(session.readFlightLog("json")).toBe(original);
      expect(session.readLifecycle().phaseCode).toBe(9);
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

  it.each(["retry", "replace", "dispose"] as const)("rejects a query whose record owner changes during export (%s)", (operation) => {
    const owner = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      complete(owner);
      const original = owner.exportRecordJson();
      if (operation === "replace") owner.openArchive(original);
      vi.spyOn(HybridGameSessionBridge.prototype, "export_flight_log_csv").mockImplementationOnce(() => {
        if (operation === "retry") owner.executeOperation("retry");
        else if (operation === "replace") {
          owner.executeOperation("leave-replay");
          owner.openArchive(original);
        }
        else owner.dispose();
        return "stale payload";
      });
      expect(() => owner.readFlightLog("csv")).toThrow(operation === "dispose" ? "disposed" : "source changed");
    } finally { owner.dispose(); }
  });

  it("rejects disposed and Attract reads before invoking a producer export", () => {
    const owner = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    const csv = vi.spyOn(HybridGameSessionBridge.prototype, "export_flight_log_csv");
    const json = vi.spyOn(HybridGameSessionBridge.prototype, "export_current_flight_record_json");
    owner.executeOperation("enter-attract");
    expect(() => owner.readFlightLog("csv")).toThrow("Result or Replay");
    expect(() => owner.readFlightLog("json")).toThrow("Result or Replay");
    owner.dispose();
    expect(() => owner.readFlightLog("json")).toThrow("disposed");
    expect(csv).not.toHaveBeenCalled();
    expect(json).not.toHaveBeenCalled();
  });
});
