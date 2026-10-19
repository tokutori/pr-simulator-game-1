import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { describe, expect, it } from "vitest";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { sharedGameUiAction } from "../../web/src/app/game-view.js";
import { currentGameSessionProjectionFixture, currentSharedGameUiFixture, sharedGameUiPlatform, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";

function setupModel(): AppModel {
  return { ...createInitialAppModel(), gameSession: currentGameSessionProjectionFixture("flight_setup"),
    presentation: { type: "ready", mode: "screen" }, sharedGameUi: currentSharedGameUiFixture("flight_setup") };
}

describe("Direct flight setup selection", () => {
  it.each([
    ["preset", 2], ["information", 2], ["assistance", 2], ["weather", 3]
  ] as const)("requests the selected %s value without owning a domain draft", (axis, code) => {
    withSharedGameUiSession("flight_setup", {}, (session, platform) => {
      const model = { ...setupModel(), sharedGameUi: session.readSharedUi(platform) };
      const action = { type: "activate" as const, controlId: `game-setup-select-${axis}-${String(code)}` };
      const admitted = sharedGameUiAction(model.sharedGameUi, action);
      if (admitted === null) throw new Error("Missing shared difficulty choice");
      const next = updateApp(model, { type: "ui-action", action });
      expect(next.model.gameSession).toBe(model.gameSession);
      expect(next.effects).toEqual([{ type: "update-shared-game-ui", action: admitted }]);
      const shared = session.updateSharedUi(admitted, platform);
      expect(shared.effects).toHaveLength(1);
      const effect = shared.effects[0];
      if (effect?.kind !== "session") throw new Error("Missing Rust difficulty command");
      const pending = updateApp(next.model, { type: "shared-game-ui-effect", effect });
      expect(pending.model.gameSession).toBe(model.gameSession);
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
      gameSession: currentGameSessionProjectionFixture("title"), sharedGameUi: currentSharedGameUiFixture("title") }]) {
      const next = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-setup-select-weather-0" } });
      expect(next.model).toBe(model);
      expect(next.effects).toEqual([]);
    }
  });

  it.each([
    { axis: "preset", command: { kind: "select_preset", preset: "expert" } },
    { axis: "information", command: { kind: "select_information", information: "minimal" } },
    { axis: "assistance", command: { kind: "select_assistance", assistance: "light" } },
    { axis: "weather", command: { kind: "select_weather", weather: "typical" } }
  ] as const)("sets the selected $axis through the current Rust session", ({ axis, command }) => {
    initSync({ module: new Uint8Array(readFileSync(fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url)))) });
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      session.executeSharedSessionCommand({ kind: "open_setup" });
      expect(session.executeSharedSessionCommand(command)).toEqual({ kind: "completed" });
      const selected = session.readSharedUi(sharedGameUiPlatform()).panels.flatMap((panel) => panel.controls)
        .find((control) => control.id === `game-setup-select-${axis}-2`);
      expect(selected?.content.kind === "button" ? selected.content.presentation : null).toMatchObject({ kind: "choice", selected: true });
    } finally { session.dispose(); }
  });
});
