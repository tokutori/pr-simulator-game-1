import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));

describe("generated WebAssembly browser binding", () => {
  it("runs a fixed-rate flight through fractional water contact", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    expect(physics_hz()).toBe(100);

    const session = new GameSessionBridge(0);
    try {
      expect(session.phase_code()).toBe(0);
      session.open_setup();
      expect(session.phase_code()).toBe(1);
      session.prepare();
      expect(session.phase_code()).toBe(2);
      session.fail_briefing(0);
      expect(session.phase_code()).toBe(8);
      session.retry_briefing();
      expect(session.phase_code()).toBe(2);
      session.cancel_briefing();
      expect(session.phase_code()).toBe(1);
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      expect(session.advance_countdown()).toBe(0);
      const initial = parseFlightSnapshot(session.launch());
      expect(initial.tick).toBe(0);
      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let index = 0; index < 3_000 && snapshot.terminal === "airborne"; index += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(Number.isInteger(snapshot.tick)).toBe(true);
      expect(snapshot.contactFraction).toBeGreaterThanOrEqual(0);
      expect(snapshot.contactFraction).toBeLessThanOrEqual(1);
      expect(snapshot.scoreCourseMeters).toBeGreaterThanOrEqual(200);
      expect(session.advance_tick(0, 0, 0, 0)).toEqual(session.snapshot());
      session.retry();
      expect(session.phase_code()).toBe(3);
      session.open_setup();
      session.return_to_title();
      expect(session.phase_code()).toBe(0);
    } finally {
      session.free();
    }
  });
});
