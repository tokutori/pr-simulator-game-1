import { createBootViewModel } from "./boot-view.js";
import { createGameViewModel } from "./game-view.js";
import { flightDiagnosticNotice, gameSessionPhaseCode } from "./app-state.js";
import type { AppMessage, AppModel } from "./app-state.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import { createFlightHudModel } from "../presentation/flight-hud-model.js";
import type { FlightHudModel, InformationLevelCode } from "../presentation/flight-hud-model.js";
import { createHeadHudView, NO_HEAD_HUD_VIEW } from "../presentation/head-hud-view.js";
import type { HeadHudView } from "../presentation/head-hud-view.js";
import type { ViewerFrame } from "../render/contracts/viewer-frame.js";
import type { UiViewModel } from "../render/contracts/ui.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";

export interface FlightFrameViewDraft {
  readonly model: AppModel;
  readonly snapshot: FlightSnapshot | null;
  readonly analysis: AppModel["flightAnalysis"];
  readonly hud: FlightHudModel | null;
  readonly headHud: HeadHudView;
}

export function createFlightFrameViewDraft(model: AppModel, snapshot: FlightSnapshot | null, viewer: ViewerFrame): FlightFrameViewDraft {
  const displayedSnapshot = model.flightRuntime.kind === "projection-unavailable" ? null : snapshot;
  const phase = gameSessionPhaseCode(model.gameSession);
  const code = model.difficulty.informationCode;
  if (!Number.isInteger(code) || code < 0 || code > 4) throw new RangeError("Information code must lie in [0, 4]");
  const hud = displayedSnapshot !== null && (phase === 5 || phase === 6)
    ? createFlightHudModel(displayedSnapshot, code as InformationLevelCode, model.difficulty.hudProfile, flightDiagnosticNotice(model))
    : null;
  const headHud = hud !== null && phase === 5 && model.presentation.type === "ready" && model.presentation.mode !== "screen"
    ? createHeadHudView(hud, viewer)
    : NO_HEAD_HUD_VIEW;
  return Object.freeze({ model, snapshot: displayedSnapshot, analysis: model.flightAnalysis, hud, headHud });
}

export function finalizeFlightFrameView(draft: FlightFrameViewDraft, headHud: HeadHudView): UiViewModel {
  return gameSessionPhaseCode(draft.model.gameSession) < 0
    ? createBootViewModel(draft.model)
    : createGameViewModel(draft.model, draft.snapshot, draft.analysis, headHud);
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
