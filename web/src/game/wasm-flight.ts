import initializeWasm, {
  GameSessionBridge,
  PersonalBestSelectionBridge,
  physics_hz
} from "../../pkg/birdman_game_wasm.js";

let wasmInitialization: Promise<unknown> | null = null;

export async function initializeGameSession(): Promise<{
  readonly session: GameSessionBridge;
  readonly physicsHz: number;
}> {
  wasmInitialization ??= initializeWasm();
  await wasmInitialization;
  const session = new GameSessionBridge(0);
  return Object.freeze({ session, physicsHz: physics_hz() });
}

export function createPersonalBestSelection(recordJson: string): PersonalBestSelectionBridge {
  return new PersonalBestSelectionBridge(recordJson);
}

export type { GameSessionBridge, PersonalBestSelectionBridge };
