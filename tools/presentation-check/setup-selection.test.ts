import { describe, expect, it, vi } from "vitest";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { executeGameSessionOperation } from "../../web/src/app/game-session-operation.js";
import type { GameSessionOperationPort } from "../../web/src/app/game-session-operation.js";

function setupModel(): AppModel {
  return { ...createInitialAppModel(), gameSession: { kind: "setup", phaseCode: 1, controlLayout: "legacy_three_axis" } };
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

  it.each(["preset", "information", "assistance", "weather"] as const)("uses one existing WASM setter for %s", (axis) => {
    const setters = {
      set_difficulty_preset: vi.fn(), set_information_level: vi.fn(),
      set_assistance_level: vi.fn(), set_weather_class: vi.fn()
    };
    const session = setters as unknown as GameSessionOperationPort;
    expect(executeGameSessionOperation(session, { kind: "set-difficulty-option", axis, code: 2 })).toEqual({ kind: "completed" });
    const name = axis === "preset" ? "set_difficulty_preset" : axis === "information" ? "set_information_level"
      : axis === "assistance" ? "set_assistance_level" : "set_weather_class";
    expect(setters[name]).toHaveBeenCalledExactlyOnceWith(2);
    expect(Object.values(setters).reduce((count, setter) => count + setter.mock.calls.length, 0)).toBe(1);
  });
});
