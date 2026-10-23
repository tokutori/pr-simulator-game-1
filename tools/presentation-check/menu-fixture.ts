import { XrPointerBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { XrPointerProjection } from "../../web/src/generated/xr-pointer-projection.js";
import type { Vec3 } from "../../web/src/render/contracts/math.js";
import { XrPointerSession } from "../../web/src/presentation/xr-pointer-session.js";
import type { BackendFrame, MenuPresentation, PreparedPresentationView, PresentationBackendAdapter } from "../../web/src/render/contracts/runtime.js";
import type { UiAction, UiPanel, UiViewModel } from "../../web/src/render/contracts/ui.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { XrReferenceFrame } from "../../web/src/generated/xr-reference-frame.js";
import { IDENTITY_POSE, composePose, pose, vec3 } from "../../web/src/render/contracts/math.js";


export function referenceFrameFixture(head: Pose): XrReferenceFrame {
  return { basis: "world", world: IDENTITY_POSE, cockpit: IDENTITY_POSE, head };
}

export function placeMenuPanel(head: Pose, distance: number): Pose {
  return composePose(head, pose(vec3(0, 0, -distance), IDENTITY_POSE.orientation));
}

export function fixtureControlBounds(panel: UiPanel, controlId: string): readonly [number, number, number, number] {
  const index = panel.controls.findIndex(control => control.id === controlId);
  if (index < 0) throw new Error(`Missing fixture control: ${controlId}`);
  const rectangles = [[0.08, 0.68, 0.84, 0.12], [0.08, 0.49, 0.84, 0.1], [0.08, 0.29, 0.84, 0.12], [0.08, 0.1, 0.84, 0.1]] as const;
  const rectangle = panel.controls.length === 1 ? [0, 0, 1, 1] as const : rectangles[index];
  if (rectangle === undefined) throw new Error("Fixture geometry supports at most four controls");
  return [rectangle[0] * panel.size.width, rectangle[1] * panel.size.height, rectangle[2] * panel.size.width, rectangle[3] * panel.size.height];
}

export function geometryMenuFixture(view: UiViewModel): MenuPresentation {
  const panel = view.panels[0];
  if (panel === undefined) return { kind: "absent" };
  const context = { generation: 1, scope: { panelId: panel.id, viewKey: view.interactionScope } };
  const viewport = {
    size: panel.size, title: panel.title, background: "#123", opacity: 0.5,
    content: { clip: { x: 0, y: 0, ...panel.size }, items: [] }, footer: [],
    surface: { scope: view.interactionScope, revision: 1, sizeM: [panel.size.width, panel.size.height] as const,
      regions: panel.controls.filter(control => control.enabled).map(control => {
        const bounds = fixtureControlBounds(panel, control.id);
        return { target: { kind: "control" as const, id: control.id }, bounds,
          control: control.kind === "range" ? { kind: "range" as const, minimum: control.minimum, maximum: control.maximum, step: control.step, track: bounds }
          : control.kind === "toggle" ? { kind: "toggle" as const, value: control.value } : null };
      }) },
    pageProgress: 0, offsetMeters: 0, maximumOffsetMeters: 0, geometryKey: { size: panel.size, clips: [{ x: 0, y: 0, ...panel.size }, { x: 0, y: 0, width: 0, height: 0 }, { x: 0, y: 0, width: 0, height: 0 }] as const, maximumOffsetMeters: 0, headings: [{ x: 0, y: 0, width: 0, height: 0 }, { x: 0, y: 0, width: 0, height: 0 }] as const, controls: panel.controls.map(control => {
      const [x, y, width, height] = fixtureControlBounds(panel, control.id);
      return {
        identity: [control.id, control.kind] as const, bounds: { x, y, width, height },
        plot: null, range: control.kind === "range" ? [control.minimum, control.maximum, control.step] as const : null
      };
    }) }
  };
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
    referenceFromSpace: IDENTITY_POSE, source: { kind: "controller" as const, id: 1 }, timestampMs: 0 };
  try {
    owner.observe(IDENTITY_POSE, menu, null, [ray], true, 0);
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
        target: { kind: "control", id: control.id }, bounds: fixtureControlBounds(panel, control.id)
      })) }));
    const projection = JSON.parse(owner.observe_json(JSON.stringify({ timestampMs: 0, focused: true, panelPose, headPose: null,
      pointers: [{ ...ray, referenceFromSpace: IDENTITY_POSE, source: { kind: "controller", id: 1 } }], gamepad: null }))) as XrPointerProjection;
    const hit = projection.primary;
    return hit === null ? null : { x: hit.point[0], y: hit.point[1],
      controlId: hit.target?.kind === "control" ? hit.target.id : null };
  } finally { owner.free(); }
}
