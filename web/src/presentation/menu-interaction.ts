import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuScrollContext, MenuScrollState } from "../render/contracts/ui.js";

export type ReadyMenu = Extract<MenuPresentation, { readonly kind: "ready" }>;

export type MenuInputGeometry = { readonly kind: "none" } | {
  readonly kind: "ready"; readonly context: MenuScrollContext; readonly signature: string;
};

export function menuContextMatches(state: MenuScrollState, context: MenuScrollContext): boolean {
  const scope = context.scope;
  return state.kind === "active" && state.generation === context.generation &&
    state.scope.panelId === scope.panelId && state.scope.viewKey === scope.viewKey;
}

export function menuContextCanInteract(menu: MenuPresentation, state: MenuScrollState, context: MenuScrollContext): boolean {
  return menu.kind === "ready" && menuContextMatches(state, context) && menuContextMatches(state, menu.context);
}

export function menuInputGeometry(menu: ReadyMenu): Extract<MenuInputGeometry, { kind: "ready" }> {
  return Object.freeze({ kind: "ready", context: menu.context, signature: JSON.stringify(menu.viewport.geometryKey) });
}

export function menuInputGeometryChanged(previous: MenuInputGeometry, menu: ReadyMenu): boolean {
  return previous.kind === "ready" && menuContextMatches({ kind: "active", ...previous.context, progress: 0 }, menu.context) &&
    previous.signature !== menuInputGeometry(menu).signature;
}

export function menuPageProgress(menu: ReadyMenu): number {
  return menu.viewport.pageProgress;
}

export function menuExposesControl(menu: ReadyMenu, controlId: string): boolean {
  return menu.viewport.surface.regions.some(region => region.target.kind === "control" && region.target.id === controlId);
}
