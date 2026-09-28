import initializeWasm, { SyntheticFlightSession, physics_hz } from "../../pkg/birdman_game_wasm.js";

let wasmInitialization: Promise<unknown> | null = null;

export async function initializeSyntheticFlight(): Promise<{
  readonly session: SyntheticFlightSession;
  readonly physicsHz: number;
}> {
  wasmInitialization ??= initializeWasm();
  await wasmInitialization;
  return Object.freeze({ session: new SyntheticFlightSession(0), physicsHz: physics_hz() });
}

export type { SyntheticFlightSession };
