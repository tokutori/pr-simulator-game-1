import type { PanelCursor } from "../render/contracts/runtime.js";
import type { UiActionDispatcher, UiPanel } from "../render/contracts/ui.js";
import type { PhoneVrGamepadState } from "./phone-vr-contracts.js";
import { actionForControl, hitTestControl, rangeAction } from "./panel-interaction.js";

const AXIS_DEADZONE = 0.2;
const CURSOR_SPEED_PER_SECOND = 1.2;
const SCROLL_PIXELS_PER_SECOND = 720;

export class GamepadUiSelector {
  private cursor = { x: 0.5, y: 0.5 };
  private focusedControlId: string | null = null;
  private previousPrimaryPressed = false;
  private previousBackPressed = false;
  private lastTimestampMs: number | null = null;

  constructor(private readonly dispatch: UiActionDispatcher) {}

  update(panel: UiPanel | null, gamepad: PhoneVrGamepadState | null, timestampMs: number): PanelCursor | null {
    if (!Number.isFinite(timestampMs)) throw new RangeError("Gamepad UI timestamp must be finite");
    if (panel === null || gamepad === null) {
      this.reset();
      return null;
    }
    const firstSample = this.lastTimestampMs === null;
    const elapsedSeconds = this.elapsedSeconds(timestampMs);
    const horizontal = readAxis(gamepad.axes, 0);
    const vertical = readAxis(gamepad.axes, 1);
    this.cursor = {
      x: clamp(this.cursor.x + horizontal * CURSOR_SPEED_PER_SECOND * elapsedSeconds, 0, 1),
      y: clamp(this.cursor.y + vertical * CURSOR_SPEED_PER_SECOND * elapsedSeconds, 0, 1)
    };
    const point = Object.freeze({
      x: (this.cursor.x - 0.5) * panel.size.width,
      y: (0.5 - this.cursor.y) * panel.size.height
    });
    const control = hitTestControl(panel, point);
    const nextFocus = control?.id ?? null;
    if (nextFocus !== this.focusedControlId) {
      this.focusedControlId = nextFocus;
      this.dispatch({ type: "focus", controlId: nextFocus });
    }

    const primaryPressed = gamepad.buttons[0] ?? false;
    if (!firstSample && primaryPressed && !this.previousPrimaryPressed && control !== null) {
      const action = control.kind === "range" ? rangeAction(control, panel, point) : actionForControl(control);
      if (action !== null) this.dispatch(action);
    }
    this.previousPrimaryPressed = primaryPressed;

    const backPressed = gamepad.buttons[1] ?? false;
    if (!firstSample && backPressed && !this.previousBackPressed) this.dispatch({ type: "back" });
    this.previousBackPressed = backPressed;

    const scrollX = readAxis(gamepad.axes, 2);
    const scrollY = readAxis(gamepad.axes, 3);
    if (elapsedSeconds > 0 && (scrollX !== 0 || scrollY !== 0)) {
      this.dispatch({
        type: "scroll",
        deltaX: scrollX * SCROLL_PIXELS_PER_SECOND * elapsedSeconds,
        deltaY: scrollY * SCROLL_PIXELS_PER_SECOND * elapsedSeconds
      });
    }
    return Object.freeze({ point, progress: 0 });
  }

  reset(): void {
    if (this.focusedControlId !== null) this.dispatch({ type: "focus", controlId: null });
    this.cursor = { x: 0.5, y: 0.5 };
    this.focusedControlId = null;
    this.previousPrimaryPressed = false;
    this.previousBackPressed = false;
    this.lastTimestampMs = null;
  }

  private elapsedSeconds(timestampMs: number): number {
    if (this.lastTimestampMs === null || timestampMs < this.lastTimestampMs) {
      this.lastTimestampMs = timestampMs;
      return 0;
    }
    const elapsedSeconds = Math.min((timestampMs - this.lastTimestampMs) / 1000, 0.05);
    this.lastTimestampMs = timestampMs;
    return elapsedSeconds;
  }
}

function readAxis(axes: readonly number[], index: number): number {
  const value = axes[index] ?? 0;
  if (!Number.isFinite(value) || Math.abs(value) <= AXIS_DEADZONE) return 0;
  return Math.sign(value) * (Math.abs(value) - AXIS_DEADZONE) / (1 - AXIS_DEADZONE);
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}
