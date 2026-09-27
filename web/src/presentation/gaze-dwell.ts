import type { PanelCursor, SelectRay } from "../render/contracts/runtime.js";
import type { UiActionDispatcher, UiPanel } from "../render/contracts/ui.js";
import { actionForControl, hitTestControl, rangeAction } from "./panel-interaction.js";
import type { PanelPoint } from "./panel-interaction.js";

interface FocusedControl {
  readonly controlId: string;
  readonly startedAtMs: number;
  readonly activated: boolean;
}

interface GazeActivation {
  readonly controlId: string;
  readonly timestampMs: number;
}

export class GazeDwellSelector {
  private focused: FocusedControl | null = null;
  private lastActivation: GazeActivation | null = null;

  constructor(private readonly dispatch: UiActionDispatcher, private readonly dwellDurationMs = 1000) {
    if (!Number.isFinite(dwellDurationMs) || dwellDurationMs <= 0) {
      throw new RangeError("Gaze dwell duration must be positive and finite");
    }
  }

  update(panel: UiPanel | null, point: PanelPoint | null, timestampMs: number): PanelCursor | null {
    if (!Number.isFinite(timestampMs)) throw new RangeError("Gaze timestamp must be finite");
    const control = panel === null || point === null ? null : hitTestControl(panel, point);
    if (control === null || point === null) {
      this.clearFocus();
      return point === null ? null : Object.freeze({ point, progress: 0 });
    }
    if (this.focused?.controlId !== control.id || timestampMs < this.focused.startedAtMs) {
      this.clearFocus();
      this.focused = { controlId: control.id, startedAtMs: timestampMs, activated: false };
      this.dispatch({ type: "focus", controlId: control.id });
    }
    const focused = this.focused;
    const progress = Math.min(1, Math.max(0, (timestampMs - focused.startedAtMs) / this.dwellDurationMs));
    if (progress >= 1 && !focused.activated && panel !== null) {
      const action = control.kind === "range" ? rangeAction(control, panel, point) : actionForControl(control);
      if (action !== null) {
        this.dispatch(action);
        this.lastActivation = { controlId: control.id, timestampMs };
      }
      this.focused = { ...focused, activated: true };
    }
    return Object.freeze({ point, progress });
  }

  wasActivatedRecently(controlId: string, ray: SelectRay, intervalMs = 500): boolean {
    if (this.lastActivation === null || !Number.isFinite(intervalMs) || intervalMs < 0) return false;
    const age = ray.timestampMs - this.lastActivation.timestampMs;
    return this.lastActivation.controlId === controlId && age >= 0 && age <= intervalMs;
  }

  reset(): void {
    this.clearFocus();
    this.lastActivation = null;
  }

  private clearFocus(): void {
    if (this.focused !== null) this.dispatch({ type: "focus", controlId: null });
    this.focused = null;
  }
}
