import { HybridGameSessionBridge, physics_hz } from "../../pkg/birdman_game_wasm.js";
import { boundaryInteger } from "../game/tail-boundary-values.js";
import { initializeWasmRuntime } from "../game/wasm-flight.js";
import { TailAppSessionFacade } from "./session-facade.js";

export interface AppSessionConfiguration {
  readonly controlModeCode: 0 | 1 | 2;
  readonly seedLow: number;
  readonly seedHigh: number;
}

export function createAppSession(configuration: AppSessionConfiguration): TailAppSessionFacade {
  const controlModeCode = boundaryInteger(configuration.controlModeCode, 0, 2);
  const seedLow = boundaryInteger(configuration.seedLow, 0, 0xffff_ffff);
  const seedHigh = boundaryInteger(configuration.seedHigh, 0, 0xffff_ffff);
  const bridge = new HybridGameSessionBridge(controlModeCode, seedLow, seedHigh);
  try {
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    facade.readLifecycle();
    return facade;
  } catch (error: unknown) {
    bridge.free();
    throw error;
  }
}

export async function initializeAppSession(configuration: AppSessionConfiguration): Promise<TailAppSessionFacade> {
  await initializeWasmRuntime();
  return createAppSession(configuration);
}
