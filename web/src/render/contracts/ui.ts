import type { AnchorKind } from "../anchors.js";
import type { Pose } from "./math.js";

export const GAME_SCENES = ["Boot", "Title", "FlightSetup", "Briefing", "Countdown", "Flight", "Result", "Replay"] as const;
export type GameScene = typeof GAME_SCENES[number];

export type UiControl = UiButton | UiToggle | UiRange | UiStatus;

export interface UiButton {
  readonly kind: "button";
  readonly id: string;
  readonly label: string;
  readonly enabled: boolean;
  readonly rect: NormalizedRect;
}

export interface UiToggle {
  readonly kind: "toggle";
  readonly id: string;
  readonly label: string;
  readonly value: boolean;
  readonly enabled: boolean;
  readonly rect: NormalizedRect;
}

export interface UiRange {
  readonly kind: "range";
  readonly id: string;
  readonly label: string;
  readonly value: number;
  readonly minimum: number;
  readonly maximum: number;
  readonly step: number;
  readonly enabled: boolean;
  readonly rect: NormalizedRect;
}

export interface UiStatus {
  readonly kind: "status";
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly enabled: boolean;
  readonly rect: NormalizedRect;
}

export interface NormalizedRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface PanelSize {
  readonly width: number;
  readonly height: number;
}

export interface UiPanel {
  readonly id: string;
  readonly title: string;
  readonly anchor: AnchorKind;
  readonly localPose: Pose;
  readonly size: PanelSize;
  readonly controls: readonly UiControl[];
}

export interface UiViewModel {
  readonly scene: GameScene;
  readonly title: string;
  readonly description: string;
  readonly activeOverlay: string | null;
  readonly panels: readonly UiPanel[];
}

export type UiAction =
  | { readonly type: "activate"; readonly controlId: string }
  | { readonly type: "set-toggle"; readonly controlId: string; readonly value: boolean }
  | { readonly type: "set-range"; readonly controlId: string; readonly value: number }
  | { readonly type: "focus"; readonly controlId: string | null }
  | { readonly type: "recenter-menu" };

export type UiActionDispatcher = (action: UiAction) => void;

export function normalizedRect(x: number, y: number, width: number, height: number): NormalizedRect {
  const values = [x, y, width, height];
  if (values.some((value) => !Number.isFinite(value)) || width <= 0 || height <= 0 ||
      x < 0 || y < 0 || x + width > 1 || y + height > 1) {
    throw new RangeError("UI rectangles must be finite and remain inside the normalized panel");
  }
  return Object.freeze({ x, y, width, height });
}

export function validateUiViewModel(viewModel: UiViewModel): void {
  const ids = new Set<string>();
  for (const panel of viewModel.panels) {
    if (ids.has(panel.id)) throw new Error(`Duplicate UI identifier: ${panel.id}`);
    ids.add(panel.id);
    if (!Number.isFinite(panel.size.width) || !Number.isFinite(panel.size.height) || panel.size.width <= 0 || panel.size.height <= 0) {
      throw new RangeError(`Invalid panel size: ${panel.id}`);
    }
    for (const control of panel.controls) {
      if (ids.has(control.id)) throw new Error(`Duplicate UI identifier: ${control.id}`);
      ids.add(control.id);
      normalizedRect(control.rect.x, control.rect.y, control.rect.width, control.rect.height);
      if (control.kind === "range" &&
          (!Number.isFinite(control.minimum) || !Number.isFinite(control.maximum) || !Number.isFinite(control.step) ||
            !Number.isFinite(control.value) || control.minimum >= control.maximum || control.step <= 0 ||
            control.value < control.minimum || control.value > control.maximum)) {
        throw new RangeError(`Invalid range control: ${control.id}`);
      }
    }
  }
}
