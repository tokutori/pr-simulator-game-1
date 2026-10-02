import { IDENTITY_POSE } from "../render/contracts/math.js";
import { NO_HEAD_HUD } from "../render/contracts/head-hud.js";
import { normalizedRect } from "../render/contracts/ui.js";
import type { UiViewModel } from "../render/contracts/ui.js";
import type { AppModel, PresentationUiState } from "./app-state.js";

export function createBootViewModel(model: AppModel): UiViewModel {
  const activeMode = activeModeOf(model.presentation);
  const canStart = model.presentation.type === "failed" ||
    (model.presentation.type === "ready" && model.presentation.mode === "screen");
  return Object.freeze({
    scene: "Boot",
    title: "鳥人間滑空ゲーム",
    description: "Rust/WASMの合成flightを開始した。A/D・矢印キー・J/LまたはGamepadで操縦する。",
    activeOverlay: null,
    headHud: NO_HEAD_HUD,
    panels: Object.freeze([Object.freeze({
      id: "boot-status",
      title: "表示基盤",
      anchor: "menu" as const,
      localPose: IDENTITY_POSE,
      size: Object.freeze({ width: 2.4, height: 1.8 }),
      controls: Object.freeze([
        Object.freeze({
          kind: "status" as const,
          id: "boot-state",
          label: "状態",
          value: model.status,
          enabled: false,
          rect: normalizedRect(0.08, 0.84, 0.84, 0.08)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-enter-webxr",
          label: model.webXrAvailable ? "WebXRで開始" : "WebXRは利用できない",
          enabled: model.webXrAvailable && canStart,
          rect: normalizedRect(0.08, 0.68, 0.84, 0.08)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-enter-phone-vr",
          label: model.phoneVrAvailable ? "Phone VRで開始" : "Phone VRは利用できない",
          enabled: model.phoneVrAvailable && canStart,
          rect: normalizedRect(0.08, 0.54, 0.84, 0.08)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-recenter-phone-tracking",
          label: "頭部追跡を正面に再設定",
          enabled: activeMode === "phone-vr",
          rect: normalizedRect(0.08, 0.4, 0.84, 0.08)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-recenter-menu",
          label: "メニューを正面に配置",
          enabled: activeMode === "webxr" || activeMode === "phone-vr",
          rect: normalizedRect(0.08, 0.26, 0.84, 0.08)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-exit-vr",
          label: "VRを終了",
          enabled: activeMode === "webxr" || activeMode === "phone-vr",
          rect: normalizedRect(0.08, 0.12, 0.84, 0.08)
        })
      ])
    })])
  });
}

function activeModeOf(state: PresentationUiState): "screen" | "webxr" | "phone-vr" | null {
  return state.type === "ready" ? state.mode : null;
}
