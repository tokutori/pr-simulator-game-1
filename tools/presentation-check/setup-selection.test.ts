import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { describe, expect, it } from "vitest";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";

function setupModel(): AppModel {
  return { ...createInitialAppModel(), gameSession: { kind: "setup", phaseCode: 1, controlLayout: "tail_incidence" } };
}

describe("Direct flight setup selection", () => {
  it.each([
    ["preset", 2], ["information", 2], ["assistance", 2], ["weather", 3]
  ] as const)("requests the selected %s value without owning a domain draft", (axis, code) => {
    const model = setupModel();
    const next = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: `game-setup-select-${axis}-${String(code)}` } });
    expect(next.model.difficulty).toBe(model.difficulty);
    expect(next.effects).toEqual([{ type: "game-session-operation", operation: { kind: "set-difficulty-option", axis, code }, requestId: 1 }]);
    const repeated = updateApp(next.model, { type: "ui-action", action: { type: "activate", controlId: "game-setup-select-weather-4" } });
    expect(repeated.model).toBe(next.model);
    expect(repeated.effects).toEqual([]);
  });

  it.each(["game-setup-select-preset-4", "game-setup-select-assistance-4", "game-setup-select-weather-5", "game-setup-select-weather-NaN", "game-setup-select-roll-1"])("rejects invalid candidate %s", (controlId) => {
    const model = setupModel();
    const next = updateApp(model, { type: "ui-action", action: { type: "activate", controlId } });
    expect(next.model).toBe(model);
    expect(next.effects).toEqual([]);
  });

  it("rejects selection outside Setup and preserves the selected value", () => {
    for (const model of [createInitialAppModel(), setupModel()]) {
      const next = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-setup-select-weather-0" } });
      expect(next.model).toBe(model);
      expect(next.effects).toEqual([]);
    }
  });

  it.each(["preset", "information", "assistance", "weather"] as const)("sets the selected %s through the current Rust session", (axis) => {
    initSync({ module: new Uint8Array(readFileSync(fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url)))) });
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      session.executeOperation("open-setup");
      expect(session.executeOperation({ kind: "set-difficulty-option", axis, code: 2 })).toEqual({ kind: "completed" });
      const difficulty = session.readDifficulty();
      const value = axis === "preset" ? difficulty.presetCode : axis === "information" ? difficulty.informationCode
        : axis === "assistance" ? difficulty.assistanceCode : difficulty.weatherCode;
      expect(value).toBe(2);
    } finally { session.dispose(); }
  });
});
