import { IDENTITY_POSE } from "../render/contracts/math.js";
import { normalizedRect } from "../render/contracts/ui.js";
import type { UiViewModel } from "../render/contracts/ui.js";

export function createBootViewModel(status: string, webXrAvailable = false, webXrActive = false): UiViewModel {
  return Object.freeze({
    scene: "Boot",
    title: "鳥人間滑空ゲーム",
    description: "Screen/VR共通表示基盤。フライト機能は後続の実装計画で追加する。",
    activeOverlay: null,
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
          value: status,
          enabled: false,
          rect: normalizedRect(0.08, 0.72, 0.84, 0.1)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-enter-webxr",
          label: webXrAvailable ? "WebXRで開始" : "WebXRは利用できない",
          enabled: webXrAvailable && !webXrActive,
          rect: normalizedRect(0.08, 0.52, 0.84, 0.1)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-recenter-menu",
          label: "メニューを正面に配置",
          enabled: webXrActive,
          rect: normalizedRect(0.08, 0.32, 0.84, 0.1)
        }),
        Object.freeze({
          kind: "button" as const,
          id: "boot-exit-webxr",
          label: "WebXRを終了",
          enabled: webXrActive,
          rect: normalizedRect(0.08, 0.12, 0.84, 0.1)
        })
      ])
    })])
  });
}
