import type { MenuPresentation } from "../render/contracts/runtime.js";
import type { MenuPoint } from "../render/contracts/menu-layout.js";
import type { MenuScrollContext, MenuScrollState, UiAction } from "../render/contracts/ui.js";
import { hitMenuViewport, menuRangeAction, menuRectInViewport } from "./menu-layout.js";
import { actionForControl } from "./panel-interaction.js";

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

export function menuHitIdentity(menu: ReadyMenu, point: MenuPoint): string {
  const hit = hitMenuViewport(menu.viewport, point);
  return hit.kind === "none" ? "" : hit.kind === "page" ? `page:${hit.direction}` : `control:${hit.layout.control.id}`;
}

export function menuFocusAction(menu: ReadyMenu, point: MenuPoint): Extract<UiAction, { type: "menu-focus" }> {
  const hit = hitMenuViewport(menu.viewport, point);
  return Object.freeze({ type: "menu-focus", context: menu.context, focus: hit.kind === "control"
    ? Object.freeze({ kind: "control", controlId: hit.layout.control.id }) : Object.freeze({ kind: "none" }) });
}

export type MenuActionResult = { readonly kind: "absent" } | { readonly kind: "action"; readonly action: UiAction };

export function menuActionAt(menu: ReadyMenu, point: MenuPoint): MenuActionResult {
  const hit = hitMenuViewport(menu.viewport, point);
  if (hit.kind === "none") return Object.freeze({ kind: "absent" });
  if (hit.kind === "page") return Object.freeze({ kind: "action", action: Object.freeze({ type: "menu-scroll", context: menu.context,
    intent: Object.freeze({ kind: "page", direction: hit.direction, pageProgress: menuPageProgress(menu) }) }) });
  const range = menuRangeAction(menu.viewport, point);
  const action = range.kind === "action" ? Object.freeze({ type: "set-range" as const, controlId: range.controlId, value: range.value })
    : actionForControl(hit.layout.control);
  return action === null ? Object.freeze({ kind: "absent" }) : Object.freeze({ kind: "action",
    action: Object.freeze({ type: "menu-control", context: menu.context, action }) });
}

export function menuExposesControl(menu: ReadyMenu, controlId: string): boolean {
  return menu.viewport.document.controls.some((layout) => layout.control.id === controlId && layout.control.enabled &&
    menuRectInViewport(menu.viewport, layout.bounds).kind === "visible");
}
