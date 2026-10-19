import type { GameUiAction } from "../generated/game-ui-action.js";
import type { GameUiPlatformInput, GameUiStoredRecord } from "../generated/game-ui-input.js";
import { gameUiInputQuery } from "../generated/game-ui-input-query.js";
import { captureJsonInput } from "../game/tail-boundary-values.js";

export type GameUiPlatformObservation = Omit<GameUiPlatformInput, "stored_records"> & {
  readonly stored_records: readonly GameUiStoredRecord[];
};

export class GameUiBoundaryError extends Error {
  constructor(readonly boundary: string, readonly diagnostics: readonly string[], options?: ErrorOptions) {
    super(`${boundary}: ${diagnostics.join("; ")}`, options);
    this.name = "GameUiBoundaryError";
  }
}

export function encodeGameUiAction(action: GameUiAction): string {
  return encodeBoundary(action, "GameUiAction");
}

export function encodeGameUiPlatformInput(observation: GameUiPlatformObservation): string {
  const input = captureJsonInput(observation);
  const water = input.water_quality_operation;
  const status = input.screen_render_scale.status;
  const bounded = { ...input,
    notification: displayText(input.notification, gameUiInputQuery.noticeCharacters),
    flight_controller_failure: input.flight_controller_failure == null ? null : displayText(input.flight_controller_failure, gameUiInputQuery.noticeCharacters),
    head_hud_failure: input.head_hud_failure == null ? null : displayText(input.head_hud_failure, gameUiInputQuery.noticeCharacters),
    water_quality_operation: "cause" in water ? { ...water, cause: displayText(water.cause, gameUiInputQuery.noticeCharacters) } : water,
    screen_render_scale: { ...input.screen_render_scale,
      status: "cause" in status ? { ...status, cause: displayText(status.cause, gameUiInputQuery.noticeCharacters) } : status },
    stored_records: Array.from(input.stored_records, (record) => ({ ...record, saved_at: displayText(record.saved_at, gameUiInputQuery.savedAtCharacters) }))
  };
  const json = encodeBoundary(bounded, "GameUiPlatformInput");
  if (new TextEncoder().encode(json).byteLength > gameUiInputQuery.maximumBytes) {
    throw new GameUiBoundaryError("GameUiPlatformInput", ["Encoded input exceeds the shared byte limit"]);
  }
  return json;
}

function displayText(value: string, maximumCharacters: number): string {
  if (value.length <= maximumCharacters) return value;
  const characters: string[] = [];
  for (const character of value) {
    if (characters.length === maximumCharacters) return `${characters.slice(0, -1).join("")}…`;
    characters.push(character);
  }
  return value;
}

function encodeBoundary(value: unknown, name: string): string {
  try {
    const json = JSON.stringify(captureJsonInput(value));
    return json;
  } catch (cause) {
    throw new GameUiBoundaryError(name, ["Input cannot be represented as JSON data"], { cause });
  }
}
