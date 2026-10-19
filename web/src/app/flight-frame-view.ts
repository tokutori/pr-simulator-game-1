import { createBootViewModel } from "./boot-view.js";
import { createGameViewModel } from "./game-view.js";
import { gameSessionPhaseCode } from "./app-state.js";
import type { AppMessage, AppModel } from "./app-state.js";
import type { FlightHudModel } from "../presentation/flight-hud-model.js";
import { projectSharedFlightHudModel } from "../presentation/flight-hud-model.js";
import { createHeadHudView, DEFAULT_HEAD_HUD_PROFILE, NO_HEAD_HUD_VIEW } from "../presentation/head-hud-view.js";
import type { HeadHudView } from "../presentation/head-hud-view.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { UiViewModel } from "../render/contracts/ui.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import type { PanelUnavailableReason } from "../render/contracts/runtime.js";

export interface FlightFrameViewDraft {
  readonly model: AppModel;
  readonly hud: FlightHudModel | null;
  readonly headHud: HeadHudView;
  readonly frameRateEnabled: boolean;
}

export function createFlightFrameViewDraft(model: AppModel, viewer: ViewerFrame, locale: string,
  framesPerSecond?: number | null): FlightFrameViewDraft {
  const hud = createFlightUiHudModel(model);
  const headHud = model.presentation.type === "ready" && model.presentation.mode !== "screen" && (hud !== null || framesPerSecond !== undefined)
    ? createHeadHudView(hud, viewer, locale, DEFAULT_HEAD_HUD_PROFILE, framesPerSecond, model.sharedGameUi?.theme ?? null)
    : NO_HEAD_HUD_VIEW;
  return Object.freeze({ model, hud, headHud, frameRateEnabled: framesPerSecond !== undefined });
}

export function createFlightUiHudModel(model: AppModel): FlightHudModel | null {
  const view = model.sharedGameUi;
  return view === null || view.hud === null ? null : projectSharedFlightHudModel(view.hud, view.theme);
}

export function finalizeFlightFrameView(draft: FlightFrameViewDraft, headHud: HeadHudView): UiViewModel {
  const view = gameSessionPhaseCode(draft.model.gameSession) < 0
    ? createBootViewModel(draft.model)
    : draft.model.sharedGameUi === null ? createBootViewModel(draft.model)
    : createGameViewModel(draft.model.sharedGameUi, headHud.kind === "visible" ? headHud.layer : NO_HEAD_HUD);
  return draft.frameRateEnabled && headHud.kind === "visible" ? Object.freeze({ ...view, headHud: headHud.layer }) : view;
}

export function menuFrameFailureRecovery(model: AppModel, frameModel: AppModel, mode: "phone-vr" | "webxr", reason: PanelUnavailableReason): AppMessage | null {
  if (reason === "viewer-unavailable" || model !== frameModel || frameModel.presentation.type !== "ready" || frameModel.presentation.mode !== mode) return null;
  return Object.freeze({ type: "backend-ended", mode, message: `VR Menu rendering failed: ${reason}` });
}
