import type { UiChart, UiControl, UiPanel, PanelSize } from "./ui.js";
export type { MenuScrollScope, MenuScrollContext, MenuScrollState, MenuScrollIntent } from "./ui.js";
import type { ViewerGeometryUnavailableReason } from "./viewer-frame.js";

export const MENU_MINIMUM_INK_HEIGHT_DEGREES = 0.35;

export interface MenuPoint {
  readonly x: number;
  readonly y: number;
}

export interface MenuRect extends MenuPoint, PanelSize {}

export type MenuFixedTextRole = "title" | "caption" | "previous" | "next";
export type MenuControlTextRole = "label" | "value" |
  "x-axis" | "y-axis" | "x-tick" | "y-tick" | "legend" | "annotation" | "north" | "scale";

export type MenuTextSlot =
  | { readonly kind: "fixed"; readonly role: MenuFixedTextRole }
  | { readonly kind: "control"; readonly controlId: string; readonly role: MenuControlTextRole; readonly index: number };

export interface MenuFontIdentity {
  readonly family: string;
  readonly weight: number;
  readonly style: "normal" | "italic";
  readonly generation: number;
}

export interface MenuTextRequest {
  readonly identity: string;
  readonly slot: MenuTextSlot;
  readonly value: string;
  readonly locale: string;
  readonly fontMeters: number;
  readonly font: MenuFontIdentity;
  readonly widthMeters: number;
}

export interface MenuMeasuredLine {
  readonly paragraphIndex: number;
  readonly value: string;
  readonly advanceMeters: number;
  readonly leftMeters: number;
  readonly rightMeters: number;
  readonly ascentMeters: number;
  readonly descentMeters: number;
}

export interface MenuTextMeasurement {
  readonly identity: string;
  readonly lines: readonly MenuMeasuredLine[];
}

export interface MenuLayoutRow {
  readonly controls: readonly Readonly<{ control: UiControl; left: number; width: number }>[];
}

export interface MenuLayoutRequest {
  readonly panel: UiPanel;
  readonly surfaceSize: PanelSize;
  readonly fontMeters: number;
  readonly font: MenuFontIdentity;
  readonly lineHeightMeters: number;
  readonly paddingMeters: number;
  readonly gapMeters: number;
  readonly rows: readonly MenuLayoutRow[];
  readonly texts: readonly MenuTextRequest[];
}

export interface MenuTextLayout {
  readonly request: MenuTextRequest;
  readonly lines: readonly MenuMeasuredLine[];
  readonly bounds: MenuRect;
}

export type MenuControlLayout = Readonly<{ bounds: MenuRect; texts: readonly MenuTextLayout[] }> & (
  | { readonly kind: "control"; readonly control: Exclude<UiControl, UiChart> }
  | { readonly kind: "chart"; readonly control: UiChart; readonly plot: MenuRect }
);

export interface MenuDocument {
  readonly request: MenuLayoutRequest;
  readonly title: MenuTextLayout;
  readonly caption: MenuTextLayout;
  readonly controls: readonly MenuControlLayout[];
  readonly contentHeightMeters: number;
  readonly previous: MenuTextMeasurement;
  readonly next: MenuTextMeasurement;
  readonly footerHeightMeters: number;
}

export interface MenuViewport {
  readonly document: MenuDocument;
  readonly contentClip: MenuRect;
  readonly previousBounds: MenuRect;
  readonly nextBounds: MenuRect;
  readonly progress: number;
  readonly offsetMeters: number;
  readonly maximumOffsetMeters: number;
}

export type MenuLayoutUnavailableReason = "missing-text-metrics" | "stale-text-metrics" |
  "invalid-text-metrics" | "invalid-layout" | "insufficient-ink-angle";

export type MenuDocumentResult =
  | { readonly kind: "ready"; readonly document: MenuDocument }
  | { readonly kind: "unavailable"; readonly reason: MenuLayoutUnavailableReason };

export type MenuOpeningReadability =
  | { readonly kind: "ready"; readonly minimumInkAngleDegrees: number }
  | { readonly kind: "unavailable"; readonly reason: ViewerGeometryUnavailableReason | "insufficient-view-area" | "insufficient-ink-angle" };

export type MenuHit =
  | { readonly kind: "none" }
  | { readonly kind: "page"; readonly direction: "previous" | "next" }
  | { readonly kind: "control"; readonly layout: MenuControlLayout; readonly documentPoint: MenuPoint };

export type MenuContentPoint =
  | { readonly kind: "outside" }
  | { readonly kind: "inside"; readonly point: MenuPoint };

export type MenuClippedRect =
  | { readonly kind: "outside" }
  | { readonly kind: "visible"; readonly rect: MenuRect };

export type MenuRangeResult =
  | { readonly kind: "absent" }
  | { readonly kind: "action"; readonly controlId: string; readonly value: number };
