import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { initSync, physics_hz, SyntheticFlightSession } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));

describe("generated WebAssembly browser binding", () => {
  it("runs a fixed-rate flight through fractional water contact", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    expect(physics_hz()).toBe(100);

    const session = new SyntheticFlightSession(0);
    try {
      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let index = 0; index < 3_000 && snapshot.terminal === "airborne"; index += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(Number.isInteger(snapshot.tick)).toBe(true);
      expect(snapshot.contactFraction).toBeGreaterThanOrEqual(0);
      expect(snapshot.contactFraction).toBeLessThanOrEqual(1);
      expect(session.advance_tick(0, 0, 0, 0)).toEqual(session.snapshot());
    } finally {
      session.free();
    }
  });
});
