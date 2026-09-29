import type { AnchorKind } from "../anchors.js";
import type { Pose } from "./math.js";

export const GAME_SCENES = ["Boot", "Title", "FlightSetup", "Briefing", "Countdown", "Flight", "Result", "Replay"] as const;
export type GameScene = typeof GAME_SCENES[number];

export type UiControl = UiButton | UiToggle | UiRange | UiStatus | UiChart;

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

export interface UiChart {
  readonly kind: "chart";
  readonly id: string;
  readonly label: string;
  readonly xAxisLabel: string;
  readonly yAxisLabel: string;
  readonly xMinimum: number;
  readonly xMaximum: number;
  readonly yMinimum: number;
  readonly yMaximum: number;
  readonly equalAxisScale: boolean;
  readonly series: readonly UiChartSeries[];
  readonly vectors: readonly UiChartVector[];
  readonly markers: readonly UiChartMarker[];
  readonly timeMarkers: readonly UiChartMarker[];
  readonly referenceLines: readonly UiChartReferenceLine[];
  readonly cursorX: number | null;
  readonly cursorPoints: readonly Readonly<{ x: number; y: number }>[];
  readonly enabled: false;
  readonly rect: NormalizedRect;
}

export interface UiChartSeries {
  readonly label: string;
  readonly color: string;
  readonly points: readonly Readonly<{ x: number; y: number }>[];
  readonly segmentColors?: readonly string[];
}

export interface UiChartVector {
  readonly label: string;
  readonly color: string;
  readonly start: Readonly<{ x: number; y: number }> | null;
  readonly end: Readonly<{ x: number; y: number }> | null;
}

export interface UiChartMarker {
  readonly label: string;
  readonly color: string;
  readonly point: Readonly<{ x: number; y: number }>;
}

export interface UiChartReferenceLine {
  readonly value: number;
  readonly label: string;
  readonly color: string;
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
  readonly presentationStyle?: "default" | "cinematic";
  readonly activeOverlay: string | null;
  readonly panels: readonly UiPanel[];
}

export type UiAction =
  | { readonly type: "activate"; readonly controlId: string }
  | { readonly type: "set-toggle"; readonly controlId: string; readonly value: boolean }
  | { readonly type: "set-range"; readonly controlId: string; readonly value: number }
  | { readonly type: "focus"; readonly controlId: string | null }
  | { readonly type: "back" }
  | { readonly type: "scroll"; readonly deltaX: number; readonly deltaY: number }
  | { readonly type: "recenter-menu" };

export type UiActionDispatcher = (action: UiAction) => void;

export function viewExposesAction(view: UiViewModel, action: UiAction): boolean {
  if (!("controlId" in action) || action.controlId === null) return false;
  return view.panels.some((panel) => panel.controls.some((control) => {
    if (control.id !== action.controlId || !control.enabled) return false;
    if (action.type === "activate") return control.kind === "button";
    if (action.type === "set-toggle") return control.kind === "toggle";
    if (action.type === "set-range") return control.kind === "range";
    return false;
  }));
}

export function normalizedRect(x: number, y: number, width: number, height: number): NormalizedRect {
  const values = [x, y, width, height];
  if (values.some((value) => !Number.isFinite(value)) || width <= 0 || height <= 0 ||
      x < 0 || y < 0 || x + width > 1 || y + height > 1) {
    throw new RangeError("UI rectangles must be finite and remain inside the normalized panel");
  }
  return Object.freeze({ x, y, width, height });
}

export function fitPlotRectToEqualScale(
  left: number,
  top: number,
  right: number,
  bottom: number,
  equalScale: boolean
): Readonly<{ left: number; top: number; right: number; bottom: number }> {
  if (![left, top, right, bottom].every(Number.isFinite) || left >= right || top >= bottom) {
    throw new RangeError("Chart plot bounds must have positive finite dimensions");
  }
  if (!equalScale) return Object.freeze({ left, top, right, bottom });
  const side = Math.min(right - left, bottom - top);
  const centerX = (left + right) / 2;
  const centerY = (top + bottom) / 2;
  return Object.freeze({
    left: centerX - side / 2,
    top: centerY - side / 2,
    right: centerX + side / 2,
    bottom: centerY + side / 2
  });
}

export function formatChartTick(value: number): string {
  if (!Number.isFinite(value)) throw new RangeError("Chart tick values must be finite");
  const magnitude = Math.abs(value);
  const digits = magnitude >= 100 ? 0 : magnitude >= 10 ? 1 : 2;
  const formatted = value.toFixed(digits).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "");
  return formatted === "-0" ? "0" : formatted;
}

export function chartScaleBarDistance(span: number): number {
  if (!Number.isFinite(span) || span <= 0) throw new RangeError("Chart scale span must be positive and finite");
  const desired = span / 4;
  const magnitude = 10 ** Math.floor(Math.log10(desired));
  const normalized = desired / magnitude;
  const factor = normalized >= 5 ? 5 : normalized >= 2 ? 2 : 1;
  return factor * magnitude;
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
      if (control.kind === "chart") {
        if (![control.xMinimum, control.xMaximum, control.yMinimum, control.yMaximum].every(Number.isFinite)
            || control.xMinimum >= control.xMaximum || control.yMinimum >= control.yMaximum
            || (control.equalAxisScale && Math.abs((control.xMaximum - control.xMinimum) - (control.yMaximum - control.yMinimum)) > 1.0e-9)
            || control.series.length === 0) {
          throw new RangeError(`Invalid chart scale or series: ${control.id}`);
        }
        if ((control.cursorX !== null && (!Number.isFinite(control.cursorX)
              || control.cursorX < control.xMinimum || control.cursorX > control.xMaximum))
            || control.cursorPoints.some((point) => !Number.isFinite(point.x) || !Number.isFinite(point.y))
            || control.vectors.some((vector) => (vector.start === null) !== (vector.end === null)
              || (vector.start !== null && vector.end !== null
                && ![vector.start.x, vector.start.y, vector.end.x, vector.end.y].every(Number.isFinite)))
            || control.markers.some((marker) => !Number.isFinite(marker.point.x) || !Number.isFinite(marker.point.y))) {
          throw new RangeError(`Invalid chart cursor: ${control.id}`);
        }
        if (control.timeMarkers.some((marker) => !Number.isFinite(marker.point.x) || !Number.isFinite(marker.point.y))) {
          throw new RangeError(`Invalid chart time marker: ${control.id}`);
        }
        if (control.referenceLines.some((line) => !Number.isFinite(line.value))) {
          throw new RangeError(`Invalid chart reference line: ${control.id}`);
        }
        for (const series of control.series) {
          if (series.points.some((point) => !Number.isFinite(point.x) || !Number.isFinite(point.y))) {
            throw new RangeError(`Chart series has non-finite points: ${control.id}`);
          }
          if (series.segmentColors !== undefined && (
            series.segmentColors.length !== Math.max(0, series.points.length - 1)
            || series.segmentColors.some((color) => !/^#[0-9a-f]{6}$/i.test(color))
          )) throw new RangeError(`Chart series has invalid segment colors: ${control.id}`);
        }
      }
    }
  }
}
