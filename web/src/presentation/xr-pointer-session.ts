import { XrPointerBridge } from "../../pkg/birdman_game_wasm.js";
import type { XrPointerSurface, XrPointerRegion } from "../generated/xr-pointer-surface.js";
import type { XrPointerProjection, XrPointerHit } from "../generated/xr-pointer-projection.js";
import type { XrPointerObservation } from "../generated/xr-pointer-observation.js";
import type { Pose } from "../render/contracts/math.js";
import type { MenuPresentation, PanelCursor, SelectRay } from "../render/contracts/runtime.js";
import type { UiActionDispatcher, UiPanel } from "../render/contracts/ui.js";
import { actionForControl, rangeAction } from "./panel-interaction.js";
import { menuPageProgress } from "./menu-interaction.js";
import { menuRangeAction, menuRectInViewport } from "./menu-layout.js";

export class XrPointerSession {
  private bridge: XrPointerBridge | null = null;
  private geometry = "";
  private generation = 0;
  private focusDirty = false;
  private panel: UiPanel | null = null;
  private menu: MenuPresentation = { kind: "absent" };

  constructor(private readonly dispatch: UiActionDispatcher) {}

  observe(scope: string, panel: UiPanel | null, panelPose: Pose, menu: MenuPresentation,
    rays: readonly SelectRay[], focused: boolean, timestampMs: number, gamepad: XrPointerObservation["gamepad"] = null): Readonly<{ cursor: PanelCursor | null; secondaryCursors: readonly PanelCursor[] }> {
    const surface = pointerSurface(scope, panel, menu);
    const geometry = JSON.stringify(surface);
    const generation = this.generation;
    const geometryChanged = geometry !== this.geometry;
    if (geometryChanged) {
      this.owner().set_surface_json(geometry);
      this.geometry = geometry;
    }
    this.panel = panel;
    this.menu = menu;
    const observation: XrPointerObservation = { timestampMs, focused, panelPose, gamepad,
      pointers: rays.map((ray) => ({ source: ray.source ?? { kind: "gaze" }, origin: ray.origin, direction: ray.direction })) };
    const projection = JSON.parse(this.owner().observe_json(JSON.stringify(observation))) as XrPointerProjection;
    this.focusDirty ||= geometryChanged || projection.focusChanged;
    if (this.focusDirty) {
      this.focus(projection.primary);
      if (generation === this.generation) this.focusDirty = false;
    }
    if (generation !== this.generation) return { cursor: null, secondaryCursors: [] };
    if (projection.activation !== null) this.activate(projection.activation);
    if (generation !== this.generation) return { cursor: null, secondaryCursors: [] };
    if (projection.back) this.dispatch({ type: "back" });
    if (generation !== this.generation) return { cursor: null, secondaryCursors: [] };
    if (projection.scrollPages !== 0 && menu.kind === "ready") this.dispatch({ type: "menu-scroll", context: menu.context,
      intent: { kind: "delta", viewportPages: projection.scrollPages, pageProgress: menuPageProgress(menu) } });
    if (generation !== this.generation) return { cursor: null, secondaryCursors: [] };
    return { cursor: projection.primary === null ? null : cursor(projection.primary, projection.progress),
      secondaryCursors: projection.secondary.map((hit) => cursor(hit, 1)) };
  }

  select(ray: SelectRay): void {
    if (this.bridge === null) return;
    const generation = this.generation;
    const pointer = { source: ray.source ?? { kind: "gaze" as const }, origin: ray.origin, direction: ray.direction };
    const hit = JSON.parse(this.bridge.select_json(JSON.stringify(pointer), ray.timestampMs)) as XrPointerHit | null;
    if (hit !== null) { this.focus(hit); if (generation === this.generation) this.activate(hit); }
  }

  reset(): void {
    const previous = this.bridge;
    const panel = this.panel;
    const menu = this.menu;
    this.generation++;
    this.bridge = null;
    this.geometry = "";
    this.focusDirty = false;
    this.panel = null;
    this.menu = { kind: "absent" };
    previous?.free();
    if (panel !== null) this.dispatch(menu.kind === "ready" ? { type: "menu-focus", context: menu.context, focus: { kind: "none" } }
      : { type: "focus", controlId: null });
  }

  private owner(): XrPointerBridge { this.bridge ??= new XrPointerBridge(); return this.bridge; }

  private focus(hit: XrPointerHit | null): void {
    const controlId = hit?.target?.kind === "control" ? hit.target.id : null;
    if (this.menu.kind === "ready") this.dispatch({ type: "menu-focus", context: this.menu.context,
      focus: controlId === null ? { kind: "none" } : { kind: "control", controlId } });
    else this.dispatch({ type: "focus", controlId });
  }

  private activate(hit: XrPointerHit): void {
    if (hit.target === null || this.panel === null) return;
    const menu = this.menu;
    if (hit.target.kind === "page") {
      if (menu.kind === "ready") this.dispatch({ type: "menu-scroll", context: menu.context,
        intent: { kind: "page", direction: hit.target.next ? "next" : "previous", pageProgress: menuPageProgress(menu) } });
      return;
    }
    const identifier = hit.target.id;
    const control = this.panel.controls.find((value) => value.id === identifier && value.enabled);
    if (control === undefined) return;
    const point = { x: hit.point[0], y: hit.point[1] };
    const range = menu.kind === "ready" ? menuRangeAction(menu.viewport, point) : null;
    const action = control.kind === "range" ? menu.kind === "ready"
      ? range?.kind === "action" && range.controlId === control.id ? { type: "set-range" as const, controlId: control.id, value: range.value } : null
      : rangeAction(control, this.panel, point) : actionForControl(control);
    if (action !== null) this.dispatch(menu.kind === "ready" ? { type: "menu-control", context: menu.context, action } : action);
  }
}

function cursor(hit: XrPointerHit, progress: number): PanelCursor {
  return { point: { x: hit.point[0], y: hit.point[1] }, progress: hit.source.kind === "controller" ? 1 : progress };
}

function pointerSurface(scope: string, panel: UiPanel | null, menu: MenuPresentation): XrPointerSurface | null {
  if (panel === null) return null;
  const regions: XrPointerRegion[] = [];
  const size = panel.size;
  if (menu.kind === "ready") {
    const viewport = menu.viewport;
    for (const [next, bounds, enabled] of [
      [false, viewport.previousBounds, viewport.offsetMeters > 0],
      [true, viewport.nextBounds, viewport.offsetMeters < viewport.maximumOffsetMeters]
    ] as const) if (enabled) regions.push({ target: { kind: "page", next }, bounds: [bounds.x, bounds.y, bounds.width, bounds.height] });
    for (const layout of viewport.document.controls) {
      if (!layout.control.enabled) continue;
      const visible = menuRectInViewport(viewport, layout.bounds);
      if (visible.kind === "outside") continue;
      const bounds = visible.rect;
      regions.push({ target: { kind: "control", id: layout.control.id }, bounds: [bounds.x, bounds.y, bounds.width, bounds.height] });
    }
  } else for (const control of panel.controls) {
    if (!control.enabled) continue;
    const bounds = control.rect;
    regions.push({ target: { kind: "control", id: control.id }, bounds: [bounds.x * size.width, bounds.y * size.height, bounds.width * size.width, bounds.height * size.height] });
  }
  return { scope: JSON.stringify([scope, menu.kind === "ready" ? menu.context.scope : null]),
    revision: menu.kind === "ready" ? menu.context.generation : 0, sizeM: [size.width, size.height], regions };
}
