import { DEFAULT_KEYBOARD_BINDINGS, keyboardIntent } from "./keyboard-intent.js";
import type { KeyboardBindings, PilotIntent } from "./keyboard-intent.js";

export interface GamepadInputConfiguration {
  readonly axes: Readonly<{ roll: number; pitch: number; yaw: number; pilotPosition: number }>;
  readonly directions: Readonly<{ roll: 1 | -1; pitch: 1 | -1; yaw: 1 | -1; pilotPosition: 1 | -1 }>;
  readonly deadZone: number;
}

export interface BrowserPilotInputConfiguration {
  readonly physicsHz: number;
  readonly keyboardBindings: KeyboardBindings;
  readonly gamepad: GamepadInputConfiguration;
  readonly pilotPositionRangeMeters: number;
  readonly pilotPositionRateMetersPerSecond: number;
}

export const DEFAULT_PILOT_INPUT_CONFIGURATION: BrowserPilotInputConfiguration = Object.freeze({
  physicsHz: 100,
  keyboardBindings: DEFAULT_KEYBOARD_BINDINGS,
  gamepad: Object.freeze({
    axes: Object.freeze({ roll: 0, pitch: 1, yaw: 2, pilotPosition: 3 }),
    directions: Object.freeze({ roll: 1, pitch: -1, yaw: 1, pilotPosition: -1 }),
    deadZone: 0.08
  }),
  pilotPositionRangeMeters: 0.4,
  pilotPositionRateMetersPerSecond: 0.4
});

export class BrowserPilotInput {
  private activity: "active" | "suspended" | "disposed" = "active";
  private readonly pressed = new Set<string>();
  private pilotPositionTargetMeters = 0;
  private readonly controlKeys: ReadonlySet<string>;
  private gamepadNeedsNeutralConfirmation = true;

  constructor(
    private readonly target: Window,
    private readonly configuration: BrowserPilotInputConfiguration = DEFAULT_PILOT_INPUT_CONFIGURATION
  ) {
    validateConfiguration(configuration);
    this.controlKeys = new Set(Object.values(configuration.keyboardBindings));
    target.addEventListener("keydown", this.onKeyDown);
    target.addEventListener("keyup", this.onKeyUp);
    target.addEventListener("blur", this.clearKeys);
  }

  readIntent(gamepads: readonly (Gamepad | null)[]): PilotIntent {
    if (this.activity !== "active") {
      return { roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: this.pilotPositionTargetMeters };
    }
    const connectedGamepad = gamepads.find((gamepad) => gamepad !== null && gamepad.connected) ?? null;
    if (connectedGamepad === null) {
      this.gamepadNeedsNeutralConfirmation = true;
    } else if (this.gamepadNeedsNeutralConfirmation && isGamepadNeutral(connectedGamepad, this.configuration.gamepad)) {
      this.gamepadNeedsNeutralConfirmation = false;
    }
    const keyboardActive = [...this.pressed].some((code) => this.controlKeys.has(code));
    if (!keyboardActive) {
      const gamepad = this.gamepadNeedsNeutralConfirmation
        ? null
        : gamepadIntent([connectedGamepad], this.configuration.gamepad, this.configuration.pilotPositionRangeMeters);
      if (gamepad !== null) {
        this.pilotPositionTargetMeters = gamepad.pilotPositionMeters;
        return gamepad;
      }
    }
    const intent = keyboardIntent(
      this.pressed,
      this.pilotPositionTargetMeters,
      this.configuration.physicsHz,
      this.configuration.keyboardBindings,
      this.configuration.pilotPositionRangeMeters,
      this.configuration.pilotPositionRateMetersPerSecond
    );
    this.pilotPositionTargetMeters = intent.pilotPositionMeters;
    return intent;
  }

  reset(initialPilotPositionMeters: number): void {
    if (this.activity === "disposed") throw new Error("Cannot reset disposed pilot input");
    if (!Number.isFinite(initialPilotPositionMeters)
        || Math.abs(initialPilotPositionMeters) > this.configuration.pilotPositionRangeMeters) {
      throw new RangeError("Initial pilot position is outside the configured range");
    }
    this.pilotPositionTargetMeters = initialPilotPositionMeters;
    this.clearKeys();
  }

  suspend(): void {
    if (this.activity === "disposed") return;
    this.activity = "suspended";
    this.clearKeys();
  }

  resume(): void {
    if (this.activity === "disposed") return;
    this.clearKeys();
    this.activity = "active";
  }

  dispose(): void {
    this.activity = "disposed";
    this.target.removeEventListener("keydown", this.onKeyDown);
    this.target.removeEventListener("keyup", this.onKeyUp);
    this.target.removeEventListener("blur", this.clearKeys);
    this.pressed.clear();
  }

  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (this.activity !== "active"
        || !this.controlKeys.has(event.code) || isEditableTarget(event.target)) return;
    event.preventDefault();
    if (event.repeat) return;
    this.pressed.add(event.code);
  };

  private readonly onKeyUp = (event: KeyboardEvent): void => {
    this.pressed.delete(event.code);
  };

  private readonly clearKeys = (): void => {
    this.pressed.clear();
    this.gamepadNeedsNeutralConfirmation = true;
  };
}

export function gamepadIntent(
  gamepads: readonly (Gamepad | null)[],
  configuration: GamepadInputConfiguration = DEFAULT_PILOT_INPUT_CONFIGURATION.gamepad,
  pilotPositionRangeMeters = DEFAULT_PILOT_INPUT_CONFIGURATION.pilotPositionRangeMeters
): PilotIntent | null {
  for (const gamepad of gamepads) {
    if (gamepad === null || !gamepad.connected) continue;
    return Object.freeze({
      roll: directedAxis(gamepad.axes[configuration.axes.roll] ?? 0, configuration.deadZone, configuration.directions.roll),
      pitch: directedAxis(gamepad.axes[configuration.axes.pitch] ?? 0, configuration.deadZone, configuration.directions.pitch),
      yaw: directedAxis(gamepad.axes[configuration.axes.yaw] ?? 0, configuration.deadZone, configuration.directions.yaw),
      pilotPositionMeters: directedAxis(gamepad.axes[configuration.axes.pilotPosition] ?? 0,
        configuration.deadZone, configuration.directions.pilotPosition) * pilotPositionRangeMeters
    });
  }
  return null;
}

function axisValue(value: number, deadZone: number): number {
  if (!Number.isFinite(value)) return 0;
  const clamped = Math.max(-1, Math.min(1, value));
  const magnitude = Math.abs(clamped);
  if (magnitude <= deadZone) return 0;
  return Math.sign(clamped) * (magnitude - deadZone) / (1 - deadZone);
}

function directedAxis(value: number, deadZone: number, direction: 1 | -1): number {
  const result = axisValue(value, deadZone) * direction;
  return result === 0 ? 0 : result;
}

function isGamepadNeutral(gamepad: Gamepad, configuration: GamepadInputConfiguration): boolean {
  return Object.values(configuration.axes).every((axisIndex) =>
    Math.abs(gamepad.axes[axisIndex] ?? 0) <= configuration.deadZone
  );
}

function validateConfiguration(configuration: BrowserPilotInputConfiguration): void {
  if (!Number.isFinite(configuration.physicsHz) || configuration.physicsHz <= 0) {
    throw new RangeError("Input physics frequency must be positive and finite");
  }
  if (!Number.isFinite(configuration.pilotPositionRangeMeters) || configuration.pilotPositionRangeMeters <= 0 ||
      !Number.isFinite(configuration.pilotPositionRateMetersPerSecond) || configuration.pilotPositionRateMetersPerSecond <= 0) {
    throw new RangeError("Pilot position range and rate must be positive and finite");
  }
  if (!Number.isFinite(configuration.gamepad.deadZone) || configuration.gamepad.deadZone < 0 || configuration.gamepad.deadZone >= 1) {
    throw new RangeError("Gamepad dead zone must lie in [0, 1)");
  }
  const indices = Object.values(configuration.gamepad.axes);
  if (indices.some((index) => !Number.isInteger(index) || index < 0)) {
    throw new RangeError("Gamepad axes must be nonnegative integer indices");
  }
  const bindings = configuration.keyboardBindings;
  const keys = [bindings.rollPositive, bindings.rollNegative, bindings.pitchPositive, bindings.pitchNegative,
    bindings.yawPositive, bindings.yawNegative, bindings.pilotPositionIncrease, bindings.pilotPositionDecrease];
  if (keys.some((key) => key.length === 0) || new Set(keys).size !== keys.length) {
    throw new RangeError("Keyboard bindings must be nonempty and unique");
  }
}

function isEditableTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (
    target.isContentEditable || target.matches("input, textarea, select, button")
  );
}
