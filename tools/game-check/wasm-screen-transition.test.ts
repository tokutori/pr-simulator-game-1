import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import { executeGameSessionOperation } from "../../web/src/app/game-session-operation.js";
import type { AppModel, GameSessionProjection } from "../../web/src/app/app-state.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));

describe("Screen UI to WebAssembly GameSession transitions", () => {
  it("completes the visible Title-to-Result-to-Retry flow through actual DOM clicks", async () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = window.document.createElement("main") as unknown as HTMLElement;
    window.document.body.append(root as never);
    let model: AppModel = Object.freeze({
      ...createInitialAppModel(),
      presentation: Object.freeze({ type: "ready", mode: "screen" })
    });

    const sessionProjection = (): GameSessionProjection => {
      const phaseCode = session.phase_code();
      return {
        phaseCode,
        controlModeCode: session.control_mode_code(),
        difficulty: {
          presetCode: session.difficulty_preset_code(),
          informationCode: session.information_level_code(),
          assistanceCode: session.assistance_level_code(),
          weatherCode: session.weather_class_code()
        },
        configurationMetadata: null,
        countdownRemaining: session.countdown_remaining(),
        canResume: session.can_resume(),
        snapshot: [5, 6, 7].includes(phaseCode) ? parseFlightSnapshot(session.snapshot()) : null
      };
    };

    const render = (): void => {
      const projection = sessionProjection();
      model = updateApp(model, { type: "game-session-synced", ...projection }).model;
      adapter.render(createGameViewModel(model, projection.snapshot, model.flightAnalysis));
    };

    const adapter = new ScreenUiAdapter(root, (action) => {
      const requested = updateApp(model, { type: "ui-action", action });
      model = requested.model;
      adapter.render(createGameViewModel(model, sessionProjection().snapshot, model.flightAnalysis));
      const effect = requested.effects.find((candidate) => candidate.type === "game-session-operation");
      if (effect?.type !== "game-session-operation") return;
      const result = executeGameSessionOperation(session, effect.operation);
      const projection = sessionProjection();
      model = updateApp(model, {
        type: "game-operation-completed",
        requestId: effect.requestId,
        ...projection
      }).model;
      adapter.render(createGameViewModel(model, projection.snapshot, model.flightAnalysis));
      if (result.kind === "countdown-started") return;
      if (model.status.length > 0) throw new Error(model.status);
    });

    const activate = (controlId: string): void => {
      const button = root.querySelector(`button[data-control-id="${controlId}"]`) as unknown as HTMLButtonElement | null;
      if (button === null) throw new Error(`Visible control not found: ${controlId}`);
      if (button.disabled) throw new Error(`Visible control is disabled: ${controlId}`);
      button.click();
      expect(model.status).not.toMatch(/unavailable|rejected|InvalidTransition/i);
    };

    try {
      render();
      expect(model.gameSession.kind).toBe("title");
      activate("game-title-start");
      expect(model.gameSession.kind).toBe("setup");
      activate("game-setup-start");
      expect(model.gameSession.kind).toBe("briefing-ready");
      activate("game-briefing-start");
      expect(model.gameSession.kind).toBe("countdown");
      while (session.countdown_remaining() > 0) session.advance_countdown();
      session.launch();
      render();
      expect(model.gameSession.kind).toBe("flight");

      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }
      expect(snapshot.terminal).toBe("water-contact");
      render();
      expect(model.gameSession.kind).toBe("result");
      activate("game-result-retry");
      expect(model.gameSession.kind).toBe("briefing-ready");
    } finally {
      session.free();
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });
});
