import type { UiChart, UiControl, UiPanel, PanelSize } from "./ui.js";
import type { MenuLayoutReply } from "../../generated/menu-layout-reply.js";
import type { GameUiView } from "../../generated/game-ui-view.js";
export type { MenuScrollScope, MenuScrollContext, MenuScrollState, MenuScrollIntent } from "./ui.js";

import type { MenuPoint, MenuRect, MenuFont as MenuFontIdentity, MenuTextRequest, MenuMeasuredLine,
  MenuTextMeasurement, MenuChartProjection, MenuLineGeometry } from "../../generated/menu-layout-reply.js";
export type { MenuPoint, MenuRect, MenuFont as MenuFontIdentity, MenuTextRequest, MenuMeasuredLine,
  MenuTextMeasurement } from "../../generated/menu-layout-reply.js";

export type MenuFontMeasurer = (value: string, fontMeters: number, weight: number, widthMeters: number, heightMeters: number) => Float64Array;

export interface MenuLayoutRequest {
  readonly panel: UiPanel;
  readonly theme: GameUiView["theme"];
  readonly surfaceSize: PanelSize;
  readonly fontMeters: number;
  readonly font: MenuFontIdentity;
  readonly lineHeightMeters: number;
  readonly paddingMeters: number;
  readonly gapMeters: number;
  readonly texts: readonly MenuTextRequest[];
}

export interface MenuTextLayout {
  readonly request: MenuTextRequest;
  readonly lines: readonly MenuMeasuredLine[];
  readonly bounds: MenuRect;
  readonly chartProjection?: MenuChartProjection;
  readonly lineGeometry?: readonly MenuLineGeometry[];
}

export type MenuControlLayout = Readonly<{ bounds: MenuRect; texts: readonly MenuTextLayout[] }> & (
  | { readonly kind: "control"; readonly control: Exclude<UiControl, UiChart> }
  | { readonly kind: "chart"; readonly control: UiChart; readonly plot: MenuRect }
);

export interface MenuDocument {
  readonly request: MenuLayoutRequest;
  readonly title: MenuTextLayout;
  readonly caption: MenuTextLayout;
  readonly sectionHeadings: readonly MenuTextLayout[];
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
  readonly pageProgress: number;
}

export type MenuLayoutUnavailableReason = Extract<MenuLayoutReply, { kind: "unavailable" }>["reason"];

export type MenuDocumentResult =
  | { readonly kind: "ready"; readonly document: MenuDocument }
  | { readonly kind: "unavailable"; readonly reason: MenuLayoutUnavailableReason };

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
