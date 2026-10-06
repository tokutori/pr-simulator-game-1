import type { BackendFrame, MenuPresentation, PreparedPresentationView, PresentationBackendAdapter } from "../../web/src/render/contracts/runtime.js";
import type { MenuControlLayout } from "../../web/src/render/contracts/menu-layout.js";
import type { UiAction, UiViewModel } from "../../web/src/render/contracts/ui.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { requestMenuLayout, layoutMeasuredMenu, menuViewport } from "../../web/src/presentation/menu-layout.js";
import { measureMenuLayout, menuCanvasSize } from "../../web/src/presentation/menu-canvas.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";

export function geometryMenuFixture(view: UiViewModel): MenuPresentation {
  const panel = view.panels[0];
  if (panel === undefined || panel.anchor !== "menu") return { kind: "absent" };
  const request = requestMenuLayout(panel, panel.size, { fontMeters: 0.025,
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
