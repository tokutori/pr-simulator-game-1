import type { GameUiAction } from "../generated/game-ui-action.js";
import type { GameUiPlatformInput } from "../generated/game-ui-input.js";
import type { GameUiUpdate } from "../generated/game-ui-effects.js";
import type { GameUiView } from "../generated/game-ui-view.js";
import type { GameUiFrame } from "../generated/game-ui-frame.js";
import type { ReplayCameraView } from "../generated/replay-camera.js";
import { validateGameUiAction, validateGameUiPlatformInput, validateGameUiUpdate, validateGameUiView, validateGameUiFrame, validateReplayCameraView } from "../generated/game-ui-validators.js";
import type { ValidateFunction } from "ajv";

export const gameUiValidatorLicenseUrl = new URL("../../../assets/game-ui-validators.LICENSE.txt", import.meta.url).href;
export const gameUiNumberFormatLicenseUrl = new URL("../../../assets/ryu-js.LICENSE.txt", import.meta.url).href;
export const earcutLicenseUrl = new URL("../../../assets/earcut.LICENSE.txt", import.meta.url).href;

export class GameUiBoundaryError extends Error {
  constructor(readonly boundary: string, readonly diagnostics: readonly string[], options?: ErrorOptions) {
    super(`${boundary}: ${diagnostics.join("; ")}`, options);
    this.name = "GameUiBoundaryError";
  }
}

export function parseGameUiView(json: string): GameUiView {
  const view = parseBoundary(json, validateGameUiView, "GameUiView");
  if (view.schema_version !== 1) throw new GameUiBoundaryError("GameUiView", ["Unsupported schema version"]);
  return view;
}

export function parseGameUiUpdate(json: string): GameUiUpdate {
  return parseBoundary(json, validateGameUiUpdate, "GameUiUpdate");
}

export function parseGameUiFrame(json: string): GameUiFrame {
  return parseBoundary(json, validateGameUiFrame, "GameUiFrame");
}

export function parseReplayCameraView(json: string): ReplayCameraView {
  return parseBoundary(json, validateReplayCameraView, "ReplayCameraView");
}

export function encodeGameUiAction(action: GameUiAction): string {
  return encodeBoundary(action, validateGameUiAction, "GameUiAction");
}

export function encodeGameUiPlatformInput(input: GameUiPlatformInput): string {
  return encodeBoundary(input, validateGameUiPlatformInput, "GameUiPlatformInput");
}

function parseBoundary<Value>(json: string, validate: ValidateFunction<Value>, name: string): Value {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch (cause) {
    throw new GameUiBoundaryError(name, ["Invalid JSON document"], { cause });
  }
  requireBoundary(value, validate, name);
  freezeBoundary(value);
  return value;
}

function encodeBoundary<Value>(value: Value, validate: ValidateFunction<Value>, name: string): string {
  requireBoundary(value, validate, name);
  return JSON.stringify(value);
}

function requireBoundary<Value>(value: unknown, validate: ValidateFunction<Value>, name: string): asserts value is Value {
  if (!validate(value)) {
    throw new GameUiBoundaryError(name, (validate.errors ?? []).map((error) => `${error.instancePath || "/"} ${error.message ?? error.keyword}`));
  }
}

function freezeBoundary(value: unknown): void {
  if (typeof value !== "object" || value === null || Object.isFrozen(value)) return;
  for (const child of Object.values(value)) freezeBoundary(child);
  Object.freeze(value);
}
