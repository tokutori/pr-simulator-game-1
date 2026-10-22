import { XrPointerBridge } from "../../pkg/birdman_game_wasm.js";
import type { XrPointerProjection, XrPointerHit } from "../generated/xr-pointer-projection.js";
import type { XrPointerObservation } from "../generated/xr-pointer-observation.js";
import type { Pose } from "../render/contracts/math.js";
import type { MenuPresentation, PanelCursor, SelectRay } from "../render/contracts/runtime.js";
import type { UiActionDispatcher } from "../render/contracts/ui.js";
import { menuPageProgress } from "./menu-interaction.js";

export class XrPointerSession {
  private bridge: XrPointerBridge | null = null;
  private geometry = "";
  private generation = 0;
  private focusDirty = false;
  private menu: MenuPresentation = { kind: "absent" };

  constructor(private readonly dispatch: UiActionDispatcher) {}

  observe(panelPose: Pose, menu: MenuPresentation,
    headPose: Pose | null, rays: readonly SelectRay[], focused: boolean, timestampMs: number, gamepad: XrPointerObservation["gamepad"] = null): Readonly<{ cursor: PanelCursor | null; secondaryCursors: readonly PanelCursor[] }> {
    const surface = menu.kind === "ready" ? menu.viewport.surface : null;
    const geometry = JSON.stringify(surface);
    const generation = this.generation;
    const geometryChanged = geometry !== this.geometry;
    if (geometryChanged) {
      this.owner().set_surface_json(geometry);
      this.geometry = geometry;
    }
    this.menu = menu;
    const observation: XrPointerObservation = { timestampMs, focused, panelPose, headPose, gamepad,
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
    const menu = this.menu;
    this.generation++;
    this.bridge = null;
    this.geometry = "";
    this.focusDirty = false;
    this.menu = { kind: "absent" };
    previous?.free();
    if (menu.kind === "ready") this.dispatch({ type: "menu-focus", context: menu.context, focus: { kind: "none" } });
  }

  private owner(): XrPointerBridge { this.bridge ??= new XrPointerBridge(); return this.bridge; }

  private focus(hit: XrPointerHit | null): void {
    const controlId = hit?.target?.kind === "control" ? hit.target.id : null;
    if (this.menu.kind === "ready") this.dispatch({ type: "menu-focus", context: this.menu.context,
      focus: controlId === null ? { kind: "none" } : { kind: "control", controlId } });
  }

  private activate(hit: XrPointerHit): void {
    if (hit.target === null || this.menu.kind !== "ready") return;
    const menu = this.menu;
    if (hit.target.kind === "page") {
      this.dispatch({ type: "menu-scroll", context: menu.context,
        intent: { kind: "page", direction: hit.target.next ? "next" : "previous", pageProgress: menuPageProgress(menu) } });
      return;
    }
    const action = hit.intent;
    if (action !== null) this.dispatch({ type: "menu-control", context: menu.context, action });
  }
}

function cursor(hit: XrPointerHit, progress: number): PanelCursor {
  return { point: { x: hit.point[0], y: hit.point[1] }, progress: hit.source.kind === "controller" ? 1 : progress };
}
