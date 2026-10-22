import { XrPointerBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { XrPointerProjection } from "../../web/src/generated/xr-pointer-projection.js";
import type { Vec3 } from "../../web/src/render/contracts/math.js";
import { XrPointerSession } from "../../web/src/presentation/xr-pointer-session.js";
import type { BackendFrame, MenuPresentation, PreparedPresentationView, PresentationBackendAdapter } from "../../web/src/render/contracts/runtime.js";
import type { MenuControlLayout } from "../../web/src/render/contracts/menu-layout.js";
import type { UiAction, UiPanel, UiViewModel } from "../../web/src/render/contracts/ui.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { XrReferenceFrame } from "../../web/src/generated/xr-reference-frame.js";
import { IDENTITY_POSE, composePose, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { requestMenuLayout, layoutMeasuredMenu, menuViewport } from "../../web/src/presentation/menu-layout.js";
import { measureMenuLayout, menuCanvasSize } from "../../web/src/presentation/menu-canvas.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import { currentSharedGameUiFixture } from "../game-check/shared-game-ui-fixture.js";

const fixtureTheme = currentSharedGameUiFixture("title").theme;

export function referenceFrameFixture(head: Pose): XrReferenceFrame {
  return { basis: "world", world: IDENTITY_POSE, cockpit: IDENTITY_POSE, head };
}

export function placeMenuPanel(head: Pose, distance: number): Pose {
  return composePose(head, pose(vec3(0, 0, -distance), IDENTITY_POSE.orientation));
}

export function geometryMenuFixture(view: UiViewModel): MenuPresentation {
  const panel = view.panels[0];
  if (panel === undefined || panel.anchor !== "menu") return { kind: "absent" };
  const request = requestMenuLayout(panel, panel.size, { theme: view.sharedPresentation?.theme ?? fixtureTheme, fontMeters: 0.025,
    font: { family: "sans-serif", weight: 600, style: "normal", generation: 0 }, locale: "ja", caption: "", previousLabel: "前頁", nextLabel: "次頁" });
  const dimensions = menuCanvasSize(panel.size);
  const measurements = measureMenuLayout(new HudCanvasFixture(), request, dimensions.width, dimensions.height);
  if (measurements.kind !== "ready") throw new Error("Missing geometry fixture metrics");
  const layout = layoutMeasuredMenu(request, measurements.measurements);
  if (layout.kind !== "ready") throw new Error("Missing geometry fixture document");
  const controls = panel.controls.map((control): MenuControlLayout => {
    const bounds = { x: control.rect.x * panel.size.width, y: control.rect.y * panel.size.height,
      width: control.rect.width * panel.size.width, height: control.rect.height * panel.size.height };
    return control.kind === "chart" ? { kind: "chart", control, bounds, plot: bounds, texts: [] } : { kind: "control", control, bounds, texts: [] };
  });
  const document = { ...layout.document, controls, contentHeightMeters: panel.size.height };
  const viewport = { ...menuViewport(document, 0), contentClip: { x: 0, y: 0, ...panel.size },
    offsetMeters: 0, maximumOffsetMeters: 0, previousBounds: { x: 0, y: panel.size.height, width: 0, height: 0 },
    nextBounds: { x: 0, y: panel.size.height, width: 0, height: 0 } };
  const context = { generation: 1, scope: view.activeOverlay === null
    ? { kind: "scene" as const, scene: view.scene, panelId: panel.id, viewKey: "geometry-fixture" }
    : { kind: "overlay" as const, scene: view.scene, panelId: panel.id, viewKey: "geometry-fixture", overlay: view.activeOverlay } };
  return { kind: "ready", panel, viewport, context };
}

export function fixtureBackendFrame(backend: PresentationBackendAdapter, timestampMs: number, view: UiViewModel, viewer: ViewerFrame): BackendFrame {
  return backend.currentFrame(timestampMs, view, viewer, geometryMenuFixture(view));
}

export function fixturePresentation(viewModel: UiViewModel): PreparedPresentationView {
  return { viewModel, menu: geometryMenuFixture(viewModel) };
}

export function fixtureSemanticAction(action: UiAction): UiAction {
  if (action.type === "menu-control") return action.action;
  if (action.type === "menu-focus") return { type: "focus", controlId: action.focus.kind === "control" ? action.focus.controlId : null };
  return action;
}

export function menuActionAt(menu: Extract<MenuPresentation, { kind: "ready" }>, point: { x: number; y: number }):
  { kind: "absent" } | { kind: "action"; action: UiAction } {
  const actions: UiAction[] = [];
  const owner = new XrPointerSession(action => actions.push(action));
  const ray = { origin: vec3(point.x, point.y, 1), direction: vec3(0, 0, -1),
    source: { kind: "controller" as const, id: 1 }, timestampMs: 0 };
  try {
    owner.observe("fixture", menu.panel, IDENTITY_POSE, menu, [ray], true, 0);
    owner.select({ ...ray, timestampMs: 1 });
    const action = actions.find(candidate => candidate.type === "menu-control" || candidate.type === "menu-scroll");
    return action === undefined ? { kind: "absent" } : { kind: "action", action };
  } finally { owner.reset(); }
}

export function sharedPanelHit(panel: UiPanel, panelPose: Pose, ray: { origin: Vec3; direction: Vec3 }):
  { x: number; y: number; controlId: string | null } | null {
  const owner = new XrPointerBridge();
  try {
    owner.set_surface_json(JSON.stringify({ scope: "fixture", sizeM: [panel.size.width, panel.size.height],
      regions: panel.controls.filter(control => control.enabled && ["button", "toggle", "range"].includes(control.kind)).map(control => ({
        target: { kind: "control", id: control.id }, bounds: [control.rect.x * panel.size.width, control.rect.y * panel.size.height,
          control.rect.width * panel.size.width, control.rect.height * panel.size.height]
      })) }));
    const projection = JSON.parse(owner.observe_json(JSON.stringify({ timestampMs: 0, focused: true, panelPose,
      pointers: [{ ...ray, source: { kind: "gaze" } }], gamepad: null }))) as XrPointerProjection;
    const hit = projection.primary;
    return hit === null ? null : { x: hit.point[0], y: hit.point[1],
      controlId: hit.target?.kind === "control" ? hit.target.id : null };
  } finally { owner.free(); }
}
