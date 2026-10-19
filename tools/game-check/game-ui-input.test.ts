import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { encodeGameUiAction, encodeGameUiPlatformInput, GameUiBoundaryError } from "../../web/src/app/game-ui-codec.js";
import type { GameUiAction } from "../../web/src/generated/game-ui-action.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

describe("UI input data captured before Rust admission", () => {
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

  it("captures action getters once and preserves the original serialization error", () => {
    let reads = 0;
    const action = { get kind() { reads += 1; return reads === 1 ? "toggle_technical" as const : "unknown"; } };
    expect(JSON.parse(encodeGameUiAction(action as GameUiAction))).toEqual({ kind: "toggle_technical" });
    expect(reads).toBe(1);
    let calls = 0;
    try {
      encodeGameUiAction({ kind: "toggle_technical", toJSON: () => { calls += 1; return { kind: "exit" }; } } as GameUiAction);
      throw new Error("Expected JSON-data rejection");
    } catch (error: unknown) {
      expect(error).toBeInstanceOf(GameUiBoundaryError);
      expect((error as GameUiBoundaryError).cause).toBeInstanceOf(RangeError);
    }
    expect(calls).toBe(0);
  });

  it("serializes nested arrays and objects without invoking global prototype toJSON", () => {
    const platform = sharedGameUiPlatform();
    const expected = encodeGameUiPlatformInput(platform);
    const descriptor = Object.getOwnPropertyDescriptor(Object.prototype, "toJSON");
    let calls = 0;
    let observed = "";
    try {
      Object.defineProperty(Object.prototype, "toJSON", { configurable: true, value: () => { calls += 1; return "redirected"; } });
      observed = encodeGameUiPlatformInput(platform);
    } finally {
      if (descriptor === undefined) Reflect.deleteProperty(Object.prototype, "toJSON");
      else Object.defineProperty(Object.prototype, "toJSON", descriptor);
    }
    expect(calls).toBe(0);
    expect(observed).toBe(expected);
  });

  it("preserves a captured cyclic-input failure without invoking application effects", () => {
    const action: Record<string, unknown> = { kind: "toggle_technical" };
    action.self = action;
    expect(() => encodeGameUiAction(action as unknown as GameUiAction)).toThrow(GameUiBoundaryError);
  });
});
