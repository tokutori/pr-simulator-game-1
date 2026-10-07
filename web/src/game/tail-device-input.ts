import { boundaryNumber } from "./tail-boundary-values.js";
import type { TailLogicalInput, TailPilotPositionCommand } from "./tail-session-codec.js";

export interface TailPilotDemand {
  readonly controlLayout: "tail_incidence";
  readonly noseUp: number;
  readonly turnRight: number;
  readonly pilotPositionCommand: TailPilotPositionCommand;
}
export interface TailRateDemand {
  readonly pitchRadiansPerSecond: number;
  readonly yawRadiansPerSecond: number;
}
export interface TailKeyboardBindings {
  readonly noseUp: string;
  readonly noseDown: string;
  readonly turnRight: string;
  readonly turnLeft: string;
  readonly pilotPositionIncrease: string;
  readonly pilotPositionDecrease: string;
}
export interface TailKeyboardConfiguration {
  readonly physicsHz: number;
  readonly pilotTargetRatePerSecond: number;
  readonly bindings: TailKeyboardBindings;
}
export interface TailGamepadConfiguration {
  readonly axes: Readonly<{ noseUp: number; turnRight: number; pilotPosition: number }>;
  readonly directions: Readonly<{ noseUp: 1 | -1; turnRight: 1 | -1; pilotPosition: 1 | -1 }>;
  readonly deadZone: number;
}
export interface TailGamepadSample {
  readonly connected: boolean;
  readonly axes: readonly number[];
}
export const DEFAULT_TAIL_KEYBOARD_CONFIGURATION: TailKeyboardConfiguration = Object.freeze({
  physicsHz: 100,
  pilotTargetRatePerSecond: 1,
  bindings: Object.freeze({ noseUp: "ArrowUp", noseDown: "ArrowDown", turnRight: "ArrowRight", turnLeft: "ArrowLeft",
    pilotPositionIncrease: "KeyL", pilotPositionDecrease: "KeyJ" })
});
export const DEFAULT_TAIL_GAMEPAD_CONFIGURATION: TailGamepadConfiguration = Object.freeze({
  axes: Object.freeze({ noseUp: 1, turnRight: 0, pilotPosition: 3 }),
  directions: Object.freeze({ noseUp: -1, turnRight: 1, pilotPosition: -1 }),
  deadZone: 0.08
});

export function tailKeyboardDemand(
  pressed: ReadonlySet<string>,
  previousNormalizedPilotTarget = 0,
  configuration: TailKeyboardConfiguration = DEFAULT_TAIL_KEYBOARD_CONFIGURATION
): Readonly<{ demand: TailPilotDemand; normalizedPilotTarget: number }> {
  boundaryNumber(previousNormalizedPilotTarget, -1, 1);
  boundaryNumber(configuration.physicsHz, Number.MIN_VALUE);
  boundaryNumber(configuration.pilotTargetRatePerSecond, Number.MIN_VALUE);
  const bindings = configuration.bindings;
  const keys = [bindings.noseUp, bindings.noseDown, bindings.turnRight, bindings.turnLeft,
    bindings.pilotPositionIncrease, bindings.pilotPositionDecrease];
  if (keys.some((key) => key.length === 0) || new Set(keys).size !== 6) throw new RangeError("Tail keyboard bindings must be unique and nonempty");
  const axis = (positive: string, negative: string) => Number(pressed.has(positive)) - Number(pressed.has(negative));
  const movement = axis(configuration.bindings.pilotPositionIncrease, configuration.bindings.pilotPositionDecrease);
  const normalizedPilotTarget = Math.max(-1, Math.min(1, previousNormalizedPilotTarget
    + movement * configuration.pilotTargetRatePerSecond / configuration.physicsHz));
  const pilotPositionCommand: TailPilotPositionCommand = movement === 0 ? Object.freeze({ kind: "hold" })
    : Object.freeze({ kind: "set", normalized: normalizedPilotTarget });
  const demand: TailPilotDemand = Object.freeze({ controlLayout: "tail_incidence",
    noseUp: axis(configuration.bindings.noseUp, configuration.bindings.noseDown),
    turnRight: axis(configuration.bindings.turnRight, configuration.bindings.turnLeft), pilotPositionCommand });
  return Object.freeze({ demand, normalizedPilotTarget });
}

export function tailGamepadDemand(
  gamepads: readonly (TailGamepadSample | null)[],
  configuration: TailGamepadConfiguration = DEFAULT_TAIL_GAMEPAD_CONFIGURATION
): TailPilotDemand | null {
  const indices = Object.values(configuration.axes);
  if (indices.some((index) => !Number.isSafeInteger(index) || index < 0) || new Set(indices).size !== 3) {
    throw new RangeError("Tail gamepad axes must be distinct nonnegative indices");
  }
  if (Object.values(configuration.directions).some((direction) => Math.abs(direction) !== 1)) {
    throw new RangeError("Tail gamepad directions must be +1 or -1");
  }
  if (!Number.isFinite(configuration.deadZone) || configuration.deadZone < 0 || configuration.deadZone >= 1) {
    throw new RangeError("Tail gamepad dead zone must lie in [0, 1)");
  }
  const gamepad = gamepads.find((candidate) => candidate !== null && candidate.connected);
  if (gamepad === undefined || gamepad === null) return null;
  const axis = (name: keyof TailGamepadConfiguration["axes"]) => {
    const value = gamepad.axes[configuration.axes[name]] ?? 0;
    if (!Number.isFinite(value)) return 0;
    const clamped = Math.max(-1, Math.min(1, value));
    const magnitude = Math.abs(clamped);
    if (magnitude <= configuration.deadZone) return 0;
    return Math.sign(clamped) * (magnitude - configuration.deadZone) / (1 - configuration.deadZone) * configuration.directions[name];
  };
  return Object.freeze({ controlLayout: "tail_incidence", noseUp: axis("noseUp"), turnRight: axis("turnRight"),
    pilotPositionCommand: Object.freeze({ kind: "set", normalized: axis("pilotPosition") }) });
}

export function tailInputWithRateDemand(demand: TailPilotDemand, rates: TailRateDemand): TailLogicalInput {
  return Object.freeze({ ...demand,
    desiredPitchRateRadiansPerSecond: boundaryNumber(rates.pitchRadiansPerSecond),
    desiredYawRateRadiansPerSecond: boundaryNumber(rates.yawRadiansPerSecond) });
}
