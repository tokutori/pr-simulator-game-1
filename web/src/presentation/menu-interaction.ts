import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuScrollContext, MenuScrollState } from "../render/contracts/ui.js";
import { menuRectInViewport } from "./menu-layout.js";

export type ReadyMenu = Extract<MenuPresentation, { readonly kind: "ready" }>;

export type MenuInputGeometry = { readonly kind: "none" } | {
  readonly kind: "ready"; readonly context: MenuScrollContext; readonly signature: string;
};

export function menuContextMatches(state: MenuScrollState, context: MenuScrollContext): boolean {
  const scope = context.scope;
  return state.kind === "active" && state.generation === context.generation && state.scope.kind === scope.kind &&
    state.scope.scene === scope.scene && state.scope.panelId === scope.panelId && state.scope.viewKey === scope.viewKey &&
    (state.scope.kind === "scene" || scope.kind === "overlay" && state.scope.overlay === scope.overlay);
}

export function menuContextCanInteract(menu: MenuPresentation, state: MenuScrollState, context: MenuScrollContext): boolean {
  return menu.kind === "ready" && menuContextMatches(state, context) && menuContextMatches(state, menu.context);
}

export function menuInputGeometry(menu: ReadyMenu): Extract<MenuInputGeometry, { kind: "ready" }> {
  const viewport = menu.viewport;
  const signature = JSON.stringify([menu.panel.size, viewport.contentClip, viewport.previousBounds, viewport.nextBounds,
    viewport.maximumOffsetMeters, viewport.document.title.bounds, viewport.document.caption.bounds,
    viewport.document.controls.map((layout) => [layout.control.id, layout.control.kind, layout.bounds,
      layout.control.kind === "range" ? [layout.control.minimum, layout.control.maximum, layout.control.step] : [],
      layout.kind === "chart" ? layout.plot : []])]);
  return Object.freeze({ kind: "ready", context: menu.context, signature });
}

export function menuInputGeometryChanged(previous: MenuInputGeometry, menu: ReadyMenu): boolean {
  return previous.kind === "ready" && menuContextMatches({ kind: "active", ...previous.context, progress: 0 }, menu.context) &&
    previous.signature !== menuInputGeometry(menu).signature;
}

export function menuPageProgress(menu: ReadyMenu): number {
  return menu.viewport.maximumOffsetMeters === 0 ? 0 : Math.min(1, menu.viewport.contentClip.height / menu.viewport.maximumOffsetMeters);
}

export function menuExposesControl(menu: ReadyMenu, controlId: string): boolean {
  return menu.viewport.document.controls.some((layout) => layout.control.id === controlId && layout.control.enabled &&
    menuRectInViewport(menu.viewport, layout.bounds).kind === "visible");
}
