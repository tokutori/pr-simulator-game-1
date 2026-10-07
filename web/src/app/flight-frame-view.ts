import { createBootViewModel } from "./boot-view.js";
import { createGameViewModel } from "./game-view.js";
import { gameSessionPhaseCode } from "./app-state.js";
import type { AppMessage, AppModel } from "./app-state.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import { NO_ENVIRONMENT_BRIEFING } from "../game/environment-briefing.js";
import type { EnvironmentBriefingProjection } from "../game/environment-briefing.js";
import { createFlightHudModel } from "../presentation/flight-hud-model.js";
import type { FlightHudModel, InformationLevelCode } from "../presentation/flight-hud-model.js";
import { createHeadHudView, NO_HEAD_HUD_VIEW } from "../presentation/head-hud-view.js";
import type { HeadHudView } from "../presentation/head-hud-view.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { UiViewModel } from "../render/contracts/ui.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import type { PanelUnavailableReason } from "../render/contracts/runtime.js";

export interface FlightFrameViewDraft {
  readonly model: AppModel;
  readonly snapshot: FlightSnapshot | null;
  readonly analysis: AppModel["flightAnalysis"];
  readonly hud: FlightHudModel | null;
  readonly headHud: HeadHudView;
  readonly environment: EnvironmentBriefingProjection;
}

export function createFlightFrameViewDraft(model: AppModel, snapshot: FlightSnapshot | null, viewer: ViewerFrame, locale: string,
  environment: EnvironmentBriefingProjection = NO_ENVIRONMENT_BRIEFING): FlightFrameViewDraft {
  const phase = gameSessionPhaseCode(model.gameSession);
  const code = model.difficulty.informationCode;
  if (!Number.isInteger(code) || code < 0 || code > 4) throw new RangeError("Information code must lie in [0, 4]");
  const hud = snapshot !== null && (phase === 5 || phase === 6)
    ? createFlightHudModel(snapshot, code as InformationLevelCode, model.difficulty.hudProfile)
    : null;
  const headHud = hud !== null && phase === 5 && model.presentation.type === "ready" && model.presentation.mode !== "screen"
    ? createHeadHudView(hud, viewer, locale)
    : NO_HEAD_HUD_VIEW;
  return Object.freeze({ model, snapshot, analysis: model.flightAnalysis, hud, headHud, environment });
}

export function finalizeFlightFrameView(draft: FlightFrameViewDraft, headHud: HeadHudView): UiViewModel {
  return gameSessionPhaseCode(draft.model.gameSession) < 0
    ? createBootViewModel(draft.model)
    : createGameViewModel(draft.model, draft.snapshot, draft.analysis, headHud, draft.environment);
}

export interface FlightMenuFrameFailure {
  readonly kind: "flight-menu-unavailable";
  readonly reason: "context-unavailable" | "text-overflow";
  readonly mode: "phone-vr" | "webxr";
  readonly presentation: AppModel["presentation"];
  readonly requestGeneration: number;
  readonly viewModel: UiViewModel;
}

export function failFlightMenuFrame(draft: FlightFrameViewDraft, viewModel: UiViewModel, reason: FlightMenuFrameFailure["reason"]): FlightMenuFrameFailure {
  const presentation = draft.model.presentation;
  if (presentation.type !== "ready" || presentation.mode === "screen") throw new Error("Flight Menu failure requires an active VR presentation");
  return Object.freeze({ kind: "flight-menu-unavailable", reason, mode: presentation.mode, presentation,
    requestGeneration: draft.model.nextRequestId,
    viewModel: Object.freeze({ ...viewModel, panels: Object.freeze([]), headHud: NO_HEAD_HUD }) });
}

export function flightMenuFailureRecovery(model: AppModel, failure: FlightMenuFrameFailure): AppMessage | null {
  if (model.presentation !== failure.presentation || model.nextRequestId !== failure.requestGeneration) return null;
  return Object.freeze({ type: "backend-ended", mode: failure.mode, message: `VR Flight Menu rendering failed: ${failure.reason}` });
}

export function menuFrameFailureRecovery(model: AppModel, frameModel: AppModel, mode: "phone-vr" | "webxr", reason: PanelUnavailableReason): AppMessage | null {
  if (reason === "viewer-unavailable" || model !== frameModel || frameModel.presentation.type !== "ready" || frameModel.presentation.mode !== mode) return null;
  return Object.freeze({ type: "backend-ended", mode, message: `VR Menu rendering failed: ${reason}` });
}
