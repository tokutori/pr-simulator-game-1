import { GameSessionBridge, HybridGameSessionBridge, physics_hz } from "../../pkg/birdman_game_wasm.js";
import { boundaryInteger } from "../game/tail-boundary-values.js";
import { initializeWasmRuntime } from "../game/wasm-flight.js";
import { LegacyAppSessionFacade, TailAppSessionFacade } from "./session-facade.js";
import type { AppSessionFacade } from "./session-facade.js";

export interface LegacySessionConfiguration {
  readonly controlLayout: "legacy_three_axis";
  readonly controlModeCode: 0 | 1 | 2;
}
export interface TailSessionConfiguration {
  readonly controlLayout: "tail_incidence";
  readonly controlModeCode: 0 | 1 | 2;
  readonly seedLow: number;
  readonly seedHigh: number;
}
export type AppSessionConfiguration = LegacySessionConfiguration | TailSessionConfiguration;

export function createAppSession(configuration: LegacySessionConfiguration): LegacyAppSessionFacade;
export function createAppSession(configuration: TailSessionConfiguration): TailAppSessionFacade;
export function createAppSession(configuration: AppSessionConfiguration): AppSessionFacade;
export function createAppSession(configuration: AppSessionConfiguration): AppSessionFacade {
  const controlModeCode = boundaryInteger(configuration.controlModeCode, 0, 2);
  const physicsHz = physics_hz();
  switch (configuration.controlLayout) {
    case "legacy_three_axis": {
      const bridge = new GameSessionBridge(controlModeCode);
      try {
        const facade = new LegacyAppSessionFacade(bridge, physicsHz);
        facade.readLifecycle();
        return facade;
      } catch (error: unknown) {
        bridge.free();
        throw error;
      }
    }
    case "tail_incidence": {
      const seedLow = boundaryInteger(configuration.seedLow, 0, 0xffff_ffff);
      const seedHigh = boundaryInteger(configuration.seedHigh, 0, 0xffff_ffff);
      const bridge = new HybridGameSessionBridge(controlModeCode, seedLow, seedHigh);
      try {
        const facade = new TailAppSessionFacade(bridge, physicsHz);
        facade.readLifecycle();
        return facade;
      } catch (error: unknown) {
        bridge.free();
        throw error;
      }
    }
    default:
      throw new RangeError("Unknown application session control layout");
  }
}

export async function initializeAppSession(configuration: LegacySessionConfiguration): Promise<LegacyAppSessionFacade>;
export async function initializeAppSession(configuration: TailSessionConfiguration): Promise<TailAppSessionFacade>;
export async function initializeAppSession(configuration: AppSessionConfiguration): Promise<AppSessionFacade>;
export async function initializeAppSession(configuration: AppSessionConfiguration): Promise<AppSessionFacade> {
  await initializeWasmRuntime();
  return createAppSession(configuration);
}
