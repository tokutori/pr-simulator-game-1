import { XrPointerBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { XrPointerProjection } from "../../web/src/generated/xr-pointer-projection.js";
import type { Vec3 } from "../../web/src/render/contracts/math.js";
import { XrPointerSession } from "../../web/src/presentation/xr-pointer-session.js";
import type { BackendFrame, MenuPresentation, PreparedPresentationView, PresentationBackendAdapter } from "../../web/src/render/contracts/runtime.js";
import type { MenuControlLayout, MenuViewport, MenuPoint, MenuHit, MenuContentPoint } from "../../web/src/render/contracts/menu-layout.js";
import type { UiAction, UiPanel, UiViewModel } from "../../web/src/render/contracts/ui.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { XrReferenceFrame } from "../../web/src/generated/xr-reference-frame.js";
import { IDENTITY_POSE, composePose, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { menuViewport, projectMenuViewport } from "../../web/src/presentation/menu-layout.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";

const geometryLayout = withSharedGameUiSession("title", { platform: { presentation_mode: "hmd" } }, (session, platform) => {
    const shared = session.readSharedUi(platform);
    const source = createGameViewModel(shared).panels[0];
    if (source === undefined) throw new Error("Missing shared Title panel");
    return prepareMenuPresentation(new HudCanvasFixture(), source,
      { generation: 1, scope: { panelId: source.id, viewKey: "geometry-fixture" } }, 0,
      configuredViewerFixture(1280, 1280, 0.1, 100, 120),
      { theme: shared.theme, viewRevision: shared.view_revision,
        font: { family: "sans-serif", weight: 600, style: "normal", generation: 0 }, locale: "ja" },
      (input, measure) => session.prepareSharedMenu(input, platform, measure));
  });
if (geometryLayout.kind !== "ready") throw new Error("Missing geometry fixture document");
const geometryDocument = geometryLayout.viewport.document;

export function referenceFrameFixture(head: Pose): XrReferenceFrame {
  return { basis: "world", world: IDENTITY_POSE, cockpit: IDENTITY_POSE, head };
}

export function placeMenuPanel(head: Pose, distance: number): Pose {
  return composePose(head, pose(vec3(0, 0, -distance), IDENTITY_POSE.orientation));
}

export function geometryMenuFixture(view: UiViewModel): MenuPresentation {
  const panel = view.panels[0];
  if (panel === undefined || panel.anchor !== "menu") return { kind: "absent" };
  const controls = panel.controls.map((control): MenuControlLayout => {
    const bounds = { x: control.rect.x * panel.size.width, y: control.rect.y * panel.size.height,
      width: control.rect.width * panel.size.width, height: control.rect.height * panel.size.height };
    return control.kind === "chart" ? { kind: "chart", control, bounds, plot: bounds, texts: [] } : { kind: "control", control, bounds, texts: [] };
  });
  const document = { ...geometryDocument, controls, contentHeightMeters: panel.size.height, footerHeightMeters: 0,
    request: { ...geometryDocument.request, panel, surfaceSize: panel.size, paddingMeters: 0, gapMeters: 0 } };
  const viewport = menuViewport(document, 0);
  const context = { generation: 1, scope: { panelId: panel.id, viewKey: view.interactionScope } };
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
    owner.observe("fixture", menu.panel, IDENTITY_POSE, menu, null, [ray], true, 0);
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
    const projection = JSON.parse(owner.observe_json(JSON.stringify({ timestampMs: 0, focused: true, panelPose, headPose: null,
      pointers: [{ ...ray, source: { kind: "controller", id: 1 } }], gamepad: null }))) as XrPointerProjection;
    const hit = projection.primary;
    return hit === null ? null : { x: hit.point[0], y: hit.point[1],
      controlId: hit.target?.kind === "control" ? hit.target.id : null };
  } finally { owner.free(); }
}

export function menuPanelPointToDocument(viewport: MenuViewport, point: MenuPoint): MenuContentPoint {
  if (![point.x, point.y].every(Number.isFinite)) return { kind: "outside" };
  const projected = projectMenuViewport(viewport, [], [point]).panelPoints[0];
  return projected == null ? { kind: "outside" } : { kind: "inside", point: projected };
}

export function menuDocumentPointToPanel(viewport: MenuViewport, point: MenuPoint): MenuPoint {
  const projected = projectMenuViewport(viewport, [], [], [point]).documentPoints[0];
  if (projected === undefined) throw new RangeError("Menu document point lost its projection");
  return projected;
}

export function hitMenuViewport(viewport: MenuViewport, point: MenuPoint): MenuHit {
  if (![point.x, point.y].every(Number.isFinite)) return { kind: "none" };
  const panel = { ...viewport.document.request.panel, size: viewport.document.request.surfaceSize };
  const result = menuActionAt({ kind: "ready", panel, viewport,
    context: { generation: 1, scope: { panelId: panel.id, viewKey: "viewport-hit" } } }, point);
  if (result.kind === "absent") return { kind: "none" };
  if (result.action.type === "menu-scroll" && result.action.intent.kind === "page") {
    return { kind: "page", direction: result.action.intent.direction };
  }
  if (result.action.type !== "menu-control") return { kind: "none" };
  const controlId = result.action.action.controlId;
  const layout = viewport.document.controls.find(layout => layout.control.id === controlId);
  const content = menuPanelPointToDocument(viewport, point);
  return layout === undefined || content.kind === "outside" ? { kind: "none" }
    : { kind: "control", layout, documentPoint: content.point };
}
