import type { GameUiAction } from "../generated/game-ui-action.js";
import type { GameUiView } from "../generated/game-ui-view.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import type { HeadHudLayer } from "../render/contracts/head-hud.js";
import { IDENTITY_POSE, pose, vec3 } from "../render/contracts/math.js";
import { normalizedRect } from "../render/contracts/ui.js";
import type { UiAction, UiButtonPresentation, UiControl, UiViewModel } from "../render/contracts/ui.js";

type SharedControl = GameUiView["panels"][number]["controls"][number];

export function createGameViewModel(shared: GameUiView, headHud: HeadHudLayer = NO_HEAD_HUD): UiViewModel {
  return Object.freeze({
    scene: shared.scene,
    title: shared.title,
    description: shared.description,
    activeOverlay: shared.active_overlay,
    presentationStyle: shared.layout.shell === "cinematic" ? "cinematic" : "default",
    sharedPresentation: Object.freeze({ layout: shared.layout, theme: shared.theme }),
    headHud,
    panels: Object.freeze(shared.panels.map((panel) => Object.freeze({
      id: panel.id,
      title: panel.title,
      anchor: panel.anchor,
      localPose: pose(vec3(panel.local_position[0], panel.local_position[1], panel.local_position[2]), IDENTITY_POSE.orientation),
      size: Object.freeze({ width: panel.size[0], height: panel.size[1] }),
      sections: panel.sections,
      controls: Object.freeze(panel.controls.map(projectControl))
    })))
  });
}

export function sharedGameUiAction(shared: GameUiView, action: UiAction): GameUiAction | null {
  if (action.type === "menu-control") return sharedGameUiAction(shared, action.action);
  if (action.type !== "activate" && action.type !== "set-toggle" && action.type !== "set-range") return null;
  const control = shared.panels.flatMap((panel) => panel.controls).find((candidate) => candidate.id === action.controlId);
  if (control === undefined || !control.enabled) return null;
  const content = control.content;
  if (action.type === "activate" && content.kind === "button") return content.action;
  if (action.type === "set-toggle" && content.kind === "toggle") {
    return Object.freeze({ kind: "session", command: Object.freeze({ kind: "set_cue", cue: content.cue, visible: action.value }) });
  }
  if (action.type === "set-range" && content.kind === "range" && Number.isFinite(action.value)
      && action.value >= content.minimum && action.value <= content.maximum) {
    return Object.freeze({ kind: content.target === "analysis" ? "seek_analysis" : "seek_replay", time_seconds: action.value });
  }
  return null;
}

function projectControl(control: SharedControl): UiControl {
  const base = { id: control.id, label: control.label, enabled: control.enabled, section: control.section,
    rect: normalizedRect(control.rect.x, control.rect.y, control.rect.width, control.rect.height) };
  const content = control.content;
  switch (content.kind) {
    case "button": {
      const presentation = projectButtonPresentation(content.presentation);
      return Object.freeze({ ...base, kind: "button", ...(presentation === null ? {} : { presentation }) });
    }
    case "toggle":
      return Object.freeze({ ...base, kind: "toggle", value: content.value });
    case "range":
      return Object.freeze({ ...base, kind: "range", value: content.value, displayValue: content.display_value,
        minimum: content.minimum, maximum: content.maximum, step: content.step });
    case "status":
      return Object.freeze({ ...base, kind: "status", value: content.value });
    case "chart": {
      return Object.freeze({ ...base, ...content.plot, kind: "chart", enabled: false });
    }
  }
}

function projectButtonPresentation(presentation: Extract<SharedControl["content"], { kind: "button" }>["presentation"]): UiButtonPresentation | null {
  switch (presentation.kind) {
    case "action":
      return presentation.emphasis === "normal" ? null : Object.freeze({ kind: "action", emphasis: presentation.emphasis });
    case "choice":
      return Object.freeze({ kind: "choice", group: presentation.group, groupLabel: presentation.group_label,
        selected: presentation.selected, description: presentation.description });
    case "disclosure":
      return Object.freeze({ kind: "disclosure", expanded: presentation.expanded });
  }
}
