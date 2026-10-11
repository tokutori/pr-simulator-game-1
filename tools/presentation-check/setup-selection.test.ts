import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { describe, expect, it } from "vitest";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { sharedGameUiAction } from "../../web/src/app/game-view.js";
import { currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";

function setupModel(): AppModel {
  return { ...createInitialAppModel(), gameSession: { kind: "setup", phaseCode: 1, controlLayout: "tail_incidence" },
    presentation: { type: "ready", mode: "screen" }, sharedGameUi: currentSharedGameUiFixture(1) };
}

describe("Direct flight setup selection", () => {
  it.each([
    ["preset", 2], ["information", 2], ["assistance", 2], ["weather", 3]
  ] as const)("requests the selected %s value without owning a domain draft", (axis, code) => {
    withSharedGameUiSession(1, {}, (session, platform) => {
      const model = { ...setupModel(), sharedGameUi: session.readSharedUi(platform) };
      const action = { type: "activate" as const, controlId: `game-setup-select-${axis}-${String(code)}` };
      const admitted = sharedGameUiAction(model.sharedGameUi, action);
      if (admitted === null) throw new Error("Missing shared difficulty choice");
      const next = updateApp(model, { type: "ui-action", action });
      expect(next.model.difficulty).toBe(model.difficulty);
      expect(next.effects).toEqual([{ type: "update-shared-game-ui", action: admitted }]);
      const shared = session.updateSharedUi(admitted, platform);
      expect(shared.effects).toHaveLength(1);
      const effect = shared.effects[0];
      if (effect?.kind !== "session") throw new Error("Missing Rust difficulty command");
      const pending = updateApp(next.model, { type: "shared-game-ui-effect", effect });
      expect(pending.model.difficulty).toBe(model.difficulty);
      expect(pending.effects).toEqual([{ type: "shared-game-session-command", command: effect.command, requestId: 1 }]);
      const observed = updateApp(pending.model, { type: "shared-game-ui-observed",
        view: session.readSharedUi({ ...platform, operation_pending: true }) }).model;
      const repeated = updateApp(observed, { type: "ui-action", action: { type: "activate", controlId: "game-setup-select-weather-4" } });
      expect(repeated.model).toBe(observed);
      expect(repeated.effects).toEqual([]);
    });
  });

  it.each(["game-setup-select-preset-4", "game-setup-select-assistance-4", "game-setup-select-weather-5", "game-setup-select-weather-NaN", "game-setup-select-roll-1"])("rejects invalid candidate %s", (controlId) => {
    const model = setupModel();
    const next = updateApp(model, { type: "ui-action", action: { type: "activate", controlId } });
    expect(next.model).toBe(model);
    expect(next.effects).toEqual([]);
  });

  it("rejects selection outside Setup and preserves the selected value", () => {
    for (const model of [createInitialAppModel(), { ...setupModel(),
      gameSession: { kind: "title" as const, phaseCode: 0 as const, controlLayout: "tail_incidence" as const }, sharedGameUi: currentSharedGameUiFixture(0) }]) {
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
