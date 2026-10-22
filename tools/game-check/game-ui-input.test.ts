import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { encodeGameUiAction, encodeGameUiPlatformInput, GameUiBoundaryError } from "../../web/src/app/game-ui-codec.js";
import type { GameUiAction } from "../../web/src/generated/game-ui-action.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

describe("UI input serialization and Rust admission", () => {
  it("lets Rust reject missing and surplus action fields without changing UI or session", () => {
    const session = new HybridGameSessionBridge(0, 21, 22);
    const platform = encodeGameUiPlatformInput(sharedGameUiPlatform());
    try {
      const before = session.game_ui_view_json(platform);
      for (const action of [Object.assign(Object.create({ kind: "toggle_technical" }) as object, {}),
        { kind: "toggle_technical", extra: true }, { kind: "unknown" }]) {
        expect(() => session.game_ui_action_json(encodeGameUiAction(action as GameUiAction), platform)).toThrow();
        expect(session.game_ui_view_json(platform)).toBe(before);
      }
    } finally { session.free(); }
  });

  it("preserves a cyclic-input serialization error", () => {
    const action: Record<string, unknown> = { kind: "toggle_technical" };
    action.self = action;
    expect(() => encodeGameUiAction(action as unknown as GameUiAction)).toThrow(GameUiBoundaryError);
  });
});
