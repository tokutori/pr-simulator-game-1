import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { decodeHudProfile, decodePreparedUiConfiguration, decodeSessionDifficulty } from "../../web/src/app/session-selection.js";
import type { SessionSelectionPort } from "../../web/src/app/session-selection.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

describe("Rust-owned session selection and prepared UI metadata", () => {
  it("reads current difficulty getters and distinguishes unprepared/record contexts", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      expect(facade.readPreparedConfiguration()).toEqual({ kind: "unavailable", reason: "unprepared_phase" });
      facade.executeOperation("open-setup");
      facade.executeOperation({ kind: "set-difficulty-option", axis: "information", code: 1 });
      facade.executeOperation({ kind: "set-difficulty-option", axis: "weather", code: 3 });
      expect(facade.readDifficulty()).toEqual(decodeSessionDifficulty(bridge));
      facade.executeOperation("prepare");
      const prepared = facade.readPreparedConfiguration();
      expect(prepared.kind).toBe("available");
      if (prepared.kind !== "available") throw new Error("Expected sealed Rust configuration");
      const metadata = bridge.configuration_metadata();
      expect(prepared.value).toMatchObject({ informationCode: metadata[1], weatherCode: metadata[3],
        catalogVersion: metadata[4], scenarioId: metadata[5], aircraftModelVersion: metadata[7], controllerProfileVersion: metadata[9] });
      const projection = facade.readGameSessionProjection();
      expect(projection).toMatchObject({ phaseCode: 3, difficulty: facade.readDifficulty(), configurationMetadata: prepared.value });
      facade.executeOperation("cancel-briefing");
      facade.executeOperation("return-to-title");
      facade.executeOperation("enter-attract");
      expect(facade.readPreparedConfiguration()).toEqual({ kind: "unavailable", reason: "playback_context" });
      expect(() => facade.readGameSessionProjection()).toThrow();
    } finally {
      facade.dispose();
    }
    expect(() => facade.readDifficulty()).toThrow("disposed");
  });

  it("never queries player configuration for menu or playback and rejects malformed wire values", () => {
    const read = vi.fn(() => new Array<number>(18).fill(0));
    for (const phase of [0, 1, 9, 10]) expect(decodePreparedUiConfiguration(phase, read).kind).toBe("unavailable");
    expect(read).not.toHaveBeenCalled();
    expect(() => decodePreparedUiConfiguration(3, () => new Array<number>(17).fill(0))).toThrow(RangeError);
    for (const invalid of [NaN, -1, 0x1_0000_0000, 0.5]) {
      const values = new Array<number>(18).fill(0);
      values[5] = invalid;
      expect(() => decodePreparedUiConfiguration(3, () => values)).toThrow(RangeError);
    }
    expect(() => decodeHudProfile([0, 1, 0, 1, 0])).toThrow(RangeError);
    expect(() => decodeHudProfile([0, 1, 0, 1, 0, 2])).toThrow(RangeError);
    const port: SessionSelectionPort = { difficulty_preset_code: () => 5, information_level_code: () => 0,
      assistance_level_code: () => 0, weather_class_code: () => 0, information_profile_codes: () => [0, 0, 0, 0, 0, 0],
      configuration_metadata: read };
    expect(() => decodeSessionDifficulty(port)).toThrow(RangeError);
  });
});
