import { readFileSync } from "node:fs";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";

export function initializeWasmFixture(): void {
  initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
}
