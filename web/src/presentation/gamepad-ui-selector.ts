import type { PanelCursor } from "../render/contracts/runtime.js";
import type { UiAction, UiActionDispatcher, UiPanel } from "../render/contracts/ui.js";
import type { PhoneVrGamepadConnection, PhoneVrGamepadState } from "./phone-vr-contracts.js";
import { actionForControl, hitTestControl, rangeAction } from "./panel-interaction.js";
import { menuActionAt, menuFocusAction, menuHitIdentity, menuPageProgress } from "./menu-interaction.js";
import type { ReadyMenu } from "./menu-interaction.js";

const AXIS_DEADZONE = 0.2;
const CURSOR_SPEED_PER_SECOND = 1.2;
const SCROLL_PIXELS_PER_SECOND = 720;

type GamepadHistory =
  | { readonly type: "unselected" }
  | {
    readonly type: "selected";
    readonly connection: PhoneVrGamepadConnection;
    readonly primaryPressed: boolean;
    readonly backPressed: boolean;
    readonly timestampMs: number;
  };

export class GamepadUiSelector {
  private cursor = { x: 0.5, y: 0.5 };
  private focusedControlId: string | null = null;
  private history: GamepadHistory = { type: "unselected" };
  private menuIdentity = "";

  constructor(private readonly dispatch: UiActionDispatcher) {}

  update(panel: UiPanel | null, gamepad: PhoneVrGamepadState | null, timestampMs: number): PanelCursor | null {
    if (!Number.isFinite(timestampMs)) throw new RangeError("Gamepad UI timestamp must be finite");
    if (panel === null || gamepad === null) {
      this.reset();
      return null;
    }
    const previous = this.history;
    const sameConnection = previous.type === "selected" && previous.connection.index === gamepad.connection.index &&
      previous.connection.generation === gamepad.connection.generation;
    const elapsedSeconds = sameConnection ? clamp((timestampMs - previous.timestampMs) / 1000, 0, 0.05) : 0;
    const actions: UiAction[] = [];
    if (!sameConnection) {
      if (this.focusedControlId !== null) actions.push({ type: "focus", controlId: null });
      this.cursor = { x: 0.5, y: 0.5 };
      this.focusedControlId = null;
    }
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
      actions.push({ type: "focus", controlId: nextFocus });
    }

    const primaryPressed = gamepad.buttons[0] ?? false;
    if (sameConnection && primaryPressed && !previous.primaryPressed && control !== null) {
      const action = control.kind === "range" ? rangeAction(control, panel, point) : actionForControl(control);
      if (action !== null) actions.push(action);
    }

    const backPressed = gamepad.buttons[1] ?? false;
    if (sameConnection && backPressed && !previous.backPressed) actions.push({ type: "back" });

    const scrollX = readAxis(gamepad.axes, 2);
    const scrollY = readAxis(gamepad.axes, 3);
    if (elapsedSeconds > 0 && (scrollX !== 0 || scrollY !== 0)) {
      actions.push({
        type: "scroll",
        deltaX: scrollX * SCROLL_PIXELS_PER_SECOND * elapsedSeconds,
        deltaY: scrollY * SCROLL_PIXELS_PER_SECOND * elapsedSeconds
      });
    }
    const history: GamepadHistory = {
      type: "selected", connection: gamepad.connection, primaryPressed, backPressed, timestampMs
    };
    this.history = history;
    for (const action of actions) {
      if (this.history !== history) return null;
      this.dispatch(action);
    }
    if (this.history !== history) return null;
    return Object.freeze({ point, progress: 0 });
  }

  reset(): void {
    const hadFocus = this.focusedControlId !== null;
    this.cursor = { x: 0.5, y: 0.5 };
    this.focusedControlId = null;
    this.history = { type: "unselected" };
    this.menuIdentity = "";
    if (hadFocus) this.dispatch({ type: "focus", controlId: null });
  }

  updateMenu(menu: ReadyMenu, gamepad: PhoneVrGamepadState, timestampMs: number): PanelCursor | null {
    if (!Number.isFinite(timestampMs)) throw new RangeError("Gamepad UI timestamp must be finite");
    const identity = JSON.stringify(menu.context);
    if (identity !== this.menuIdentity) {
      this.focusedControlId = null;
      this.menuIdentity = identity;
    }
    const previous = this.history;
    const sameConnection = previous.type === "selected" && previous.connection.index === gamepad.connection.index && previous.connection.generation === gamepad.connection.generation;
    const elapsedSeconds = sameConnection ? clamp((timestampMs - previous.timestampMs) / 1000, 0, 0.05) : 0;
    if (!sameConnection) this.cursor = { x: 0.5, y: 0.5 };
    this.cursor = { x: clamp(this.cursor.x + readAxis(gamepad.axes, 0) * CURSOR_SPEED_PER_SECOND * elapsedSeconds, 0, 1),
      y: clamp(this.cursor.y + readAxis(gamepad.axes, 1) * CURSOR_SPEED_PER_SECOND * elapsedSeconds, 0, 1) };
    const point = Object.freeze({ x: (this.cursor.x - 0.5) * menu.panel.size.width, y: (0.5 - this.cursor.y) * menu.panel.size.height });
    const controlId = menuHitIdentity(menu, point);
    const actions: UiAction[] = [];
    if (controlId !== this.focusedControlId) {
      this.focusedControlId = controlId;
      actions.push(menuFocusAction(menu, point));
    }
    const primaryPressed = gamepad.buttons[0] ?? false;
    if (sameConnection && primaryPressed && !previous.primaryPressed) {
      const action = menuActionAt(menu, point);
      if (action.kind === "action") actions.push(action.action);
    }
    const backPressed = gamepad.buttons[1] ?? false;
    if (sameConnection && backPressed && !previous.backPressed) actions.push({ type: "back" });
    const scroll = readAxis(gamepad.axes, 3);
    if (scroll !== 0 && elapsedSeconds > 0) actions.push({ type: "menu-scroll", context: menu.context,
      intent: { kind: "delta", viewportPages: scroll * elapsedSeconds * 2, pageProgress: menuPageProgress(menu) } });
    const history: GamepadHistory = { type: "selected", connection: gamepad.connection, primaryPressed, backPressed, timestampMs };
    this.history = history;
    for (const action of actions) {
      if (this.history !== history) return null;
      this.dispatch(action);
    }
    return this.history === history ? Object.freeze({ point, progress: 0 }) : null;
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
