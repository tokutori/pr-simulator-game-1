import { createBootViewModel } from "./boot-view.js";
import { createGameViewModel } from "./game-view.js";
import { gameSessionPhase } from "./app-state.js";
import type { AppMessage, AppModel } from "./app-state.js";
import type { FlightHudModel } from "../presentation/flight-hud-model.js";
import { projectSharedFlightHudModel } from "../presentation/flight-hud-model.js";
import type { HeadHudLayer } from "../render/contracts/head-hud.js";
import type { UiViewModel } from "../render/contracts/ui.js";
import type { PanelUnavailableReason } from "../render/contracts/runtime.js";

export function createFlightUiHudModel(model: AppModel): FlightHudModel | null {
  const view = model.sharedGameUi;
  return view === null || view.hud === null ? null : projectSharedFlightHudModel(view.hud, view.theme);
}

export function flightFrameView(model: AppModel, headHud: HeadHudLayer): UiViewModel {
  return gameSessionPhase(model.gameSession) === "boot" || model.sharedGameUi === null
    ? createBootViewModel(model)
    : createGameViewModel(model.sharedGameUi, headHud);
}

export function menuFrameFailureRecovery(model: AppModel, frameModel: AppModel, mode: "phone-vr" | "webxr", reason: PanelUnavailableReason): AppMessage | null {
  if (reason === "viewer-unavailable" || model !== frameModel || frameModel.presentation.type !== "ready" || frameModel.presentation.mode !== mode) return null;
  return { type: "backend-ended", mode, message: `VR Menu rendering failed: ${reason}` };
}
