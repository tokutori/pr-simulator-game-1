import initializeWasm, { HybridGameSessionBridge, physics_hz } from "../../pkg/birdman_game_wasm.js";
import { boundaryInteger } from "../game/tail-boundary-values.js";
import { TailAppSessionFacade } from "./session-facade.js";

let wasmInitialization: Promise<unknown> | null = null;

export interface AppSessionConfiguration {
  readonly controlModeCode: 0 | 1 | 2;
  readonly seedLow: number;
  readonly seedHigh: number;
}

export function createAppSession(configuration?: AppSessionConfiguration): TailAppSessionFacade {
  const bridge = configuration === undefined ? HybridGameSessionBridge.public_default()
    : new HybridGameSessionBridge(boundaryInteger(configuration.controlModeCode, 0, 2),
      boundaryInteger(configuration.seedLow, 0, 0xffff_ffff), boundaryInteger(configuration.seedHigh, 0, 0xffff_ffff));
  try {
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    facade.readLifecycle();
    return facade;
  } catch (error: unknown) {
    bridge.free();
    throw error;
  }
}

export async function initializeWasmRuntime(): Promise<void> {
  wasmInitialization ??= initializeWasm();
  await wasmInitialization;
}
