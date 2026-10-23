import type { Pose } from "./math.js";
import type { HeadHudLayer } from "./head-hud.js";
import type { GameUiView } from "../../generated/game-ui-view.js";
import type { GameUiAction } from "../../generated/game-ui-action.js";
import type { MenuSessionInput } from "../../generated/menu-session-input.js";
import type { MenuSessionState } from "../../generated/menu-session-state.js";

type GameUiControlInput = Extract<GameUiAction, { kind: "control_input" }>["input"];

export type AnchorKind = GameUiView["panels"][number]["anchor"];

export type GameScene = GameUiView["scene"];

export { XR_FLIGHT_MENU_GEOMETRY as FLIGHT_MENU_GEOMETRY } from "../../generated/xr-flight-menu-geometry.js";

export type UiControl = (UiButton | UiToggle | UiRange | UiStatus | UiChart) & Readonly<{ section?: string }>;

export interface UiButton {
  readonly kind: "button";
  readonly id: string;
  readonly label: string;
  readonly enabled: boolean;
  readonly presentation?: UiButtonPresentation;
}

export type UiButtonPresentation =
  | { readonly kind: "action"; readonly emphasis: "primary" | "secondary" }
  | { readonly kind: "choice"; readonly group: string; readonly groupLabel: string; readonly selected: boolean; readonly description: string }
  | { readonly kind: "disclosure"; readonly expanded: boolean };

export function uiButtonLabel(control: UiButton): string {
  const presentation = control.presentation;
  if (presentation?.kind === "choice") {
    const label = `${presentation.selected ? "●" : "○"} ${control.label}`;
    return presentation.description.length === 0 ? label : `${label}\n${presentation.description}`;
  }
  if (presentation?.kind === "disclosure") return `${presentation.expanded ? "▾" : "▸"} ${control.label}`;
  return control.label;
}

export function uiControlBackground(control: UiControl, theme: GameUiView["theme"]): string {
  if (control.kind !== "button") return theme.choice_background;
  const presentation = control.presentation;
  if (presentation?.kind === "action") return presentation.emphasis === "primary" ? theme.primary_background : theme.panel_background;
  if (presentation?.kind === "choice") return presentation.selected ? theme.primary_background : theme.choice_background;
  if (presentation?.kind === "disclosure") return theme.panel_background;
  return theme.normal_background;
}

export interface UiToggle {
  readonly kind: "toggle";
  readonly id: string;
  readonly label: string;
  readonly value: boolean;
  readonly enabled: boolean;
}

export interface UiRange {
  readonly kind: "range";
  readonly id: string;
  readonly label: string;
  readonly value: number;
  readonly displayValue: string;
  readonly minimum: number;
  readonly maximum: number;
  readonly step: number;
  readonly enabled: boolean;
}

export interface UiStatus {
  readonly kind: "status";
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly enabled: boolean;
}

type SharedChart = Extract<GameUiView["panels"][number]["controls"][number]["content"], { kind: "chart" }>["plot"];

export type UiChart = Readonly<{
  readonly kind: "chart";
  readonly id: string;
  readonly label: string;
  readonly enabled: false;
}> & SharedChart;

export type UiChartSeries = SharedChart["series"][number];
export type UiChartVector = SharedChart["vectors"][number];
export type UiChartMarker = SharedChart["markers"][number];
export type UiChartReferenceLine = SharedChart["reference_lines"][number];

export interface PanelSize {
  readonly width: number;
  readonly height: number;
}

export type MenuScrollScope = Extract<MenuSessionState, { kind: "active" }>["scope"];
export type MenuScrollContext = Extract<MenuSessionInput, { type: "menu-scroll" }>["context"];
export type MenuScrollState = Extract<MenuSessionState, { kind: "closed" }>
  | Omit<Extract<MenuSessionState, { kind: "active" }>, "focus">;
export type MenuScrollIntent = Extract<MenuSessionInput, { type: "menu-scroll" }>["intent"];

export type MenuControlAction = Extract<UiAction, { readonly type: "activate" | "set-toggle" | "set-range" }>;

export interface UiPanel {
  readonly id: string;
  readonly title: string;
  readonly anchor: AnchorKind;
  readonly localPose: Pose;
  readonly size: PanelSize;
  readonly controls: readonly UiControl[];
  readonly sections?: GameUiView["panels"][number]["sections"];
}

export type UiPanelComposition = "physical" | "overlay";

export function uiPanelComposition(anchor: AnchorKind): UiPanelComposition {
  switch (anchor) {
    case "world":
    case "cockpit":
      return "physical";
    case "menu":
    case "head":
      return "overlay";
  }
}

export interface UiViewModel {
  readonly interactionScope: string;
  readonly scene: GameScene;
  readonly title: string;
  readonly description: string;
  readonly presentationStyle?: "default" | "cinematic";
  readonly activeOverlay: string | null;
  readonly panels: readonly UiPanel[];
  readonly headHud: HeadHudLayer;
  readonly sharedPresentation?: Readonly<{ layout: GameUiView["layout"]; theme: GameUiView["theme"] }>;
}

export type UiAction =
  | GameUiControlInput
  | { readonly type: "focus"; readonly controlId: string | null }
  | { readonly type: "back" }
  | { readonly type: "scroll"; readonly deltaX: number; readonly deltaY: number }
  | { readonly type: "menu-scroll"; readonly context: MenuScrollContext; readonly intent: MenuScrollIntent }
  | { readonly type: "menu-focus"; readonly context: MenuScrollContext; readonly focus: { readonly kind: "none" } | { readonly kind: "control"; readonly controlId: string } }
  | { readonly type: "menu-control"; readonly context: MenuScrollContext; readonly action: MenuControlAction }
  | { readonly type: "recenter-menu" };

export type UiActionDispatcher = (action: UiAction) => void;

export function controlInputAction(action: UiAction): GameUiAction | null {
  if (action.type === "menu-control") return controlInputAction(action.action);
  return action.type === "activate" || action.type === "set-toggle" || action.type === "set-range"
    ? { kind: "control_input", input: action } : null;
}

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
