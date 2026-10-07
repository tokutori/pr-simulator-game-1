import { boundaryNumber } from "./tail-boundary-values.js";
import { DEFAULT_TAIL_GAMEPAD_CONFIGURATION, DEFAULT_TAIL_KEYBOARD_CONFIGURATION, tailGamepadDemand, tailKeyboardDemand } from "./tail-device-input.js";
import type { TailGamepadConfiguration, TailKeyboardConfiguration, TailPilotDemand } from "./tail-device-input.js";

export interface BrowserTailInputConfiguration {
  readonly keyboard: TailKeyboardConfiguration;
  readonly gamepad: TailGamepadConfiguration;
}
export const DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION: BrowserTailInputConfiguration = Object.freeze({
  keyboard: DEFAULT_TAIL_KEYBOARD_CONFIGURATION,
  gamepad: DEFAULT_TAIL_GAMEPAD_CONFIGURATION
});
interface GamepadConnection {
  readonly index: number;
  readonly generation: number;
}
type GamepadAcquisition =
  | Readonly<{ kind: "unselected" }>
  | Readonly<{ kind: "waiting-neutral"; connection: GamepadConnection }>
  | Readonly<{ kind: "waiting-pilot"; connection: GamepadConnection; previousTarget: number }>
  | Readonly<{ kind: "active"; connection: GamepadConnection }>;
type GamepadSelection = Readonly<{ kind: "selected"; gamepad: Gamepad }> | Readonly<{ kind: "absent" }>;
type GamepadDemand = Readonly<{ kind: "ready"; demand: TailPilotDemand }> | Readonly<{ kind: "waiting" }>;

export class BrowserTailPilotInput {
  private activity: "active" | "suspended" | "disposed" = "active";
  private readonly pressed = new Set<string>();
  private readonly controlKeys: ReadonlySet<string>;
  private readonly connectionGenerations = new Map<number, number>();
  private acquisition: GamepadAcquisition = { kind: "unselected" };

  constructor(private readonly target: Window,
    private readonly configuration: BrowserTailInputConfiguration = DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION) {
    tailKeyboardDemand(new Set(), 0, configuration.keyboard);
    tailGamepadDemand([], configuration.gamepad);
    this.controlKeys = new Set(Object.values(configuration.keyboard.bindings));
    target.addEventListener("keydown", this.onKeyDown);
    target.addEventListener("keyup", this.onKeyUp);
    target.addEventListener("blur", this.clearKeys);
    target.addEventListener("gamepadconnected", this.onGamepadConnectionChange);
    target.addEventListener("gamepaddisconnected", this.onGamepadConnectionChange);
  }

  readDemand(gamepads: readonly (Gamepad | null)[], heldTargetNormalized: number): TailPilotDemand {
    boundaryNumber(heldTargetNormalized, -1, 1);
    if (this.activity !== "active") return neutralHold();
    const selection = this.selectGamepad(gamepads);
    if ([...this.pressed].some((code) => this.controlKeys.has(code))) {
      this.rearmGamepad();
    } else if (selection.kind === "selected") {
      const sample = this.readGamepad(selection.gamepad, heldTargetNormalized);
      if (sample.kind === "ready") return sample.demand;
    }
    return tailKeyboardDemand(this.pressed, heldTargetNormalized, this.configuration.keyboard).demand;
  }

  reset(heldTargetNormalized: number): void {
    if (this.activity === "disposed") throw new Error("Cannot reset disposed tail input");
    boundaryNumber(heldTargetNormalized, -1, 1);
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
    if (this.activity === "disposed") return;
    this.activity = "disposed";
    this.target.removeEventListener("keydown", this.onKeyDown);
    this.target.removeEventListener("keyup", this.onKeyUp);
    this.target.removeEventListener("blur", this.clearKeys);
    this.target.removeEventListener("gamepadconnected", this.onGamepadConnectionChange);
    this.target.removeEventListener("gamepaddisconnected", this.onGamepadConnectionChange);
    this.pressed.clear();
    this.connectionGenerations.clear();
    this.acquisition = { kind: "unselected" };
  }

  private selectGamepad(gamepads: readonly (Gamepad | null)[]): GamepadSelection {
    const acquisition = this.acquisition;
    if (acquisition.kind !== "unselected") {
      const connection = acquisition.connection;
      const current = gamepads.find((gamepad) => gamepad !== null && gamepad.connected && gamepad.index === connection.index);
      if (current !== undefined && current !== null && connection.generation === (this.connectionGenerations.get(connection.index) ?? 0)) {
        return { kind: "selected", gamepad: current };
      }
    }
    const candidate = gamepads.find((gamepad) => gamepad !== null && gamepad.connected);
    if (candidate === undefined || candidate === null) {
      this.acquisition = { kind: "unselected" };
      return { kind: "absent" };
    }
    this.acquisition = { kind: "waiting-neutral", connection: { index: candidate.index, generation: this.connectionGenerations.get(candidate.index) ?? 0 } };
    return { kind: "selected", gamepad: candidate };
  }

  private readGamepad(gamepad: Gamepad, heldTargetNormalized: number): GamepadDemand {
    const acquisition = this.acquisition;
    if (acquisition.kind === "unselected") return { kind: "waiting" };
    if (acquisition.kind === "waiting-neutral" && !Object.values(this.configuration.gamepad.axes)
      .every((index) => Number.isFinite(gamepad.axes[index] ?? 0) && Math.abs(gamepad.axes[index] ?? 0) <= this.configuration.gamepad.deadZone)) {
      return { kind: "waiting" };
    }
    const demand = tailGamepadDemand([gamepad], this.configuration.gamepad);
    if (demand === null) return { kind: "waiting" };
    if (acquisition.kind === "active" || demand.pilotPositionCommand.kind === "hold") return { kind: "ready", demand };
    const current = demand.pilotPositionCommand.normalized;
    const previous = acquisition.kind === "waiting-neutral" ? current : acquisition.previousTarget;
    const pickedUp = (previous <= heldTargetNormalized && heldTargetNormalized <= current)
      || (current <= heldTargetNormalized && heldTargetNormalized <= previous);
    this.acquisition = pickedUp ? { kind: "active", connection: acquisition.connection }
      : { kind: "waiting-pilot", connection: acquisition.connection, previousTarget: current };
    return { kind: "ready", demand: Object.freeze({ ...demand, pilotPositionCommand: Object.freeze({ kind: "hold" }) }) };
  }

  private rearmGamepad(): void {
    if (this.acquisition.kind !== "unselected") this.acquisition = { kind: "waiting-neutral", connection: this.acquisition.connection };
  }

  private readonly onGamepadConnectionChange = (event: GamepadEvent): void => {
    if (this.activity === "disposed") return;
    const index = event.gamepad.index;
    this.connectionGenerations.set(index, (this.connectionGenerations.get(index) ?? 0) + 1);
  };

  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (this.activity !== "active" || !this.controlKeys.has(event.code) || editableTarget(event.target)) return;
    event.preventDefault();
    if (event.repeat) return;
    this.pressed.add(event.code);
    this.rearmGamepad();
  };

  private readonly onKeyUp = (event: KeyboardEvent): void => {
    this.pressed.delete(event.code);
  };

  private readonly clearKeys = (): void => {
    this.pressed.clear();
    this.rearmGamepad();
  };
}

function neutralHold(): TailPilotDemand {
  return Object.freeze({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0, pilotPositionCommand: Object.freeze({ kind: "hold" }) });
}

function editableTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || target.matches("input, textarea, select, button"));
}
