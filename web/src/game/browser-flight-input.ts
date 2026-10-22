import type { FlightGamepadObservation } from "../generated/flight-gamepad.js";
import type { FlightKeyObservation } from "../generated/flight-key.js";

export interface FlightInputSink {
  observeFlightKey(observation: FlightKeyObservation): boolean;
  invalidateFlightInput(): void;
}

const bindings = new Map<string, FlightKeyObservation["control"]>([
  ["ArrowUp", "nose_up"], ["ArrowDown", "nose_down"],
  ["ArrowRight", "turn_right"], ["ArrowLeft", "turn_left"],
  ["KeyL", "pilot_increase"], ["KeyJ", "pilot_decrease"]
]);

export class BrowserFlightInput {
  private disposed = false;
  private readonly connectionGenerations = new Map<number, number>();

  constructor(private readonly target: Window, private readonly sink: FlightInputSink,
    private readonly sample: () => readonly (Gamepad | null)[] = availableGamepads) {
    target.addEventListener("keydown", this.onKeyDown);
    target.addEventListener("keyup", this.onKeyUp);
    target.addEventListener("blur", this.onBlur);
    target.addEventListener("gamepadconnected", this.onGamepadConnectionChange);
    target.addEventListener("gamepaddisconnected", this.onGamepadConnectionChange);
  }

  readGamepads(): readonly FlightGamepadObservation[] {
    if (this.disposed) return [];
    return this.sample().flatMap(gamepad => {
      if (gamepad === null || !gamepad.connected) return [];
      const generation = this.connectionGenerations.get(gamepad.index) ?? 0;
      if (!Number.isSafeInteger(gamepad.index) || gamepad.index < 0 || gamepad.index > 0xffff_ffff
        || generation > 0xffff_ffff) throw new RangeError("Gamepad connection identity is unavailable");
      return [{ identity: { slot_low: gamepad.index, slot_high: 0, generation }, connected: true,
        axes: [axis(gamepad, 1, -1), axis(gamepad, 0, 1), axis(gamepad, 3, -1)] as const }];
    });
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.target.removeEventListener("keydown", this.onKeyDown);
    this.target.removeEventListener("keyup", this.onKeyUp);
    this.target.removeEventListener("blur", this.onBlur);
    this.target.removeEventListener("gamepadconnected", this.onGamepadConnectionChange);
    this.target.removeEventListener("gamepaddisconnected", this.onGamepadConnectionChange);
    this.connectionGenerations.clear();
    this.sink.invalidateFlightInput();
  }

  private readonly onGamepadConnectionChange = (event: GamepadEvent): void => {
    const index = event.gamepad.index;
    this.connectionGenerations.set(index, (this.connectionGenerations.get(index) ?? 0) + 1);
  };

  private readonly onKeyDown = (event: KeyboardEvent): void => {
    const control = bindings.get(event.code);
    if (control === undefined || editableTarget(event.target)) return;
    if (this.sink.observeFlightKey({ control, pressed: true, repeat: event.repeat })) event.preventDefault();
  };

  private readonly onKeyUp = (event: KeyboardEvent): void => {
    const control = bindings.get(event.code);
    if (control !== undefined) this.sink.observeFlightKey({ control, pressed: false, repeat: false });
  };

  private readonly onBlur = (): void => { this.sink.invalidateFlightInput(); };
}

function axis(gamepad: Gamepad, index: number, direction: 1 | -1): number | null {
  const value = gamepad.axes[index];
  return value === undefined || !Number.isFinite(value) ? null : value * direction;
}

function availableGamepads(): readonly (Gamepad | null)[] {
  return typeof navigator === "undefined" || typeof navigator.getGamepads !== "function" ? [] : navigator.getGamepads();
}

function editableTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && (target.isContentEditable || target.matches("input, textarea, select"));
}
