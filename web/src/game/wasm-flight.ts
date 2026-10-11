import initializeWasm, { physics_hz } from "../../pkg/birdman_game_wasm.js";

let wasmInitialization: Promise<unknown> | null = null;

export async function initializeWasmRuntime(): Promise<number> {
  wasmInitialization ??= initializeWasm();
  await wasmInitialization;
  return physics_hz();
}
