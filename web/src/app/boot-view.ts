import { IDENTITY_POSE } from "../render/contracts/math.js";
import { normalizedRect } from "../render/contracts/ui.js";
import type { UiViewModel } from "../render/contracts/ui.js";

export function createBootViewModel(status: string): UiViewModel {
  return Object.freeze({
    scene: "Boot",
    title: "鳥人間滑空ゲーム",
    description: "共通表示基盤を初期化している。フライト機能は後続の実装計画で追加する。",
    activeOverlay: null,
    panels: Object.freeze([Object.freeze({
      id: "boot-status",
      title: "表示基盤",
      anchor: "menu" as const,
      localPose: IDENTITY_POSE,
      size: Object.freeze({ width: 2.4, height: 1.8 }),
      controls: Object.freeze([Object.freeze({
        kind: "status" as const,
        id: "boot-state",
        label: "状態",
        value: status,
        enabled: false,
        rect: normalizedRect(0.08, 0.22, 0.84, 0.12)
      })])
    })])
  });
}
