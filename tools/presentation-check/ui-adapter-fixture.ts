import { IDENTITY_POSE, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { NO_HEAD_HUD } from "../../web/src/render/contracts/head-hud.js";
import type { VisibleHeadHud } from "../../web/src/render/contracts/head-hud.js";
import type { AnchorKind, GameScene, UiPanel, UiViewModel } from "../../web/src/render/contracts/ui.js";

const PANEL_SIZE = Object.freeze({ width: 2.4, height: 1.8 });

export function createSceneFixture(scene: GameScene, overlay: string | null = null): UiViewModel {
  const anchor: AnchorKind = scene === "Flight" && overlay === null ? "cockpit" : "menu";
  const title = overlay === null ? scene : `${scene} / ${overlay}`;
  const panel: UiPanel = Object.freeze({
    id: `${scene.toLowerCase()}-panel`,
    title,
    anchor,
    localPose: IDENTITY_POSE,
    size: PANEL_SIZE,
    controls: Object.freeze([
      Object.freeze({
        kind: "button" as const,
        id: `${scene.toLowerCase()}-action`,
        label: "共通actionを送信",
        enabled: true,
        rect: { x: 0.08, y: 0.68, width: 0.84, height: 0.12 }
      }),
      Object.freeze({
        kind: "toggle" as const,
        id: `${scene.toLowerCase()}-toggle`,
        label: "表示補助",
        value: false,
        enabled: true,
        rect: { x: 0.08, y: 0.49, width: 0.84, height: 0.1 }
      }),
      Object.freeze({
        kind: "range" as const,
        id: `${scene.toLowerCase()}-range`,
        label: "表示値",
        value: 0.5,
        displayValue: "0.5",
        minimum: 0,
        maximum: 1,
        step: 0.1,
        enabled: true,
        rect: { x: 0.08, y: 0.29, width: 0.84, height: 0.12 }
      }),
      Object.freeze({
        kind: "status" as const,
        id: `${scene.toLowerCase()}-status`,
        label: "状態",
        value: "fixture",
        enabled: false,
        rect: { x: 0.08, y: 0.1, width: 0.84, height: 0.1 }
      })
    ])
  });
  return Object.freeze({
    interactionScope: JSON.stringify([scene, overlay]),
    scene,
    title,
    description: "描画adapter用fixture",
    activeOverlay: overlay,
    panels: Object.freeze([panel]),
    headHud: NO_HEAD_HUD
  });
}

export function createAnchorFixture(anchor: AnchorKind): UiPanel {
  return Object.freeze({
    id: `anchor-${anchor}`,
    title: `${anchor} anchor fixture`,
    anchor,
    localPose: pose(vec3(0, 0, 0), IDENTITY_POSE.orientation),
    size: PANEL_SIZE,
    controls: Object.freeze([])
  });
}

export function createHeadHudFixture(): VisibleHeadHud {
  return {
    kind: "visible", localPose: pose(vec3(0, 0, -2.4), IDENTITY_POSE.orientation),
    size: { width: 1.8, height: 1.2 }, canvasSize: [2048, 1365],
    clearRegion: { x: 0.25, y: 0.1, width: 0.5, height: 0.5 }, items: []
  };
}
