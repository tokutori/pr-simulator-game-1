import { IDENTITY_POSE, pose, vec3 } from "../render/contracts/math.js";
import type { AnchorKind } from "../render/anchors.js";
import { GAME_SCENES, normalizedRect } from "../render/contracts/ui.js";
import type { GameScene, UiPanel, UiViewModel } from "../render/contracts/ui.js";

export const SCENE_FIXTURE_OVERLAYS: Readonly<Record<GameScene, readonly string[]>> = Object.freeze({
  Boot: [],
  Title: ["Settings", "Help", "Credits"],
  FlightSetup: ["Settings", "Help", "Credits"],
  Briefing: ["Help", "Credits"],
  Countdown: [],
  Flight: ["Pause", "Pause/Settings", "Pause/Help"],
  Result: ["Help", "Credits"],
  Replay: ["Settings", "Help", "Credits"]
} as const satisfies Record<GameScene, readonly string[]>);

const SCENE_DESCRIPTIONS: Readonly<Record<GameScene, string>> = Object.freeze({
  Boot: "初期化状態の表示fixture",
  Title: "title screenの表示fixture",
  FlightSetup: "flight setupの表示fixture",
  Briefing: "briefingの表示fixture",
  Countdown: "countdownの表示fixture",
  Flight: "cockpit HUDの表示fixture",
  Result: "result analysisの表示fixture",
  Replay: "replay controlsの表示fixture"
});

const PANEL_SIZE = Object.freeze({ width: 2.4, height: 1.8 });

export function createSceneFixture(scene: GameScene, overlay: string | null = null): UiViewModel {
  if (!GAME_SCENES.includes(scene)) throw new RangeError(`Unsupported game scene: ${scene}`);
  if (overlay !== null && !SCENE_FIXTURE_OVERLAYS[scene].includes(overlay)) {
    throw new RangeError(`Overlay ${overlay} is not available in ${scene}`);
  }
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
        rect: normalizedRect(0.08, 0.68, 0.84, 0.12)
      }),
      Object.freeze({
        kind: "toggle" as const,
        id: `${scene.toLowerCase()}-toggle`,
        label: "表示補助",
        value: false,
        enabled: true,
        rect: normalizedRect(0.08, 0.49, 0.84, 0.1)
      }),
      Object.freeze({
        kind: "range" as const,
        id: `${scene.toLowerCase()}-range`,
        label: "表示値",
        value: 0.5,
        minimum: 0,
        maximum: 1,
        step: 0.1,
        enabled: true,
        rect: normalizedRect(0.08, 0.29, 0.84, 0.12)
      }),
      Object.freeze({
        kind: "status" as const,
        id: `${scene.toLowerCase()}-status`,
        label: "状態",
        value: "fixture",
        enabled: false,
        rect: normalizedRect(0.08, 0.1, 0.84, 0.1)
      })
    ])
  });
  return Object.freeze({
    scene,
    title,
    description: SCENE_DESCRIPTIONS[scene],
    activeOverlay: overlay,
    panels: Object.freeze([panel])
  });
}

export function createAllSceneFixtures(): readonly UiViewModel[] {
  return Object.freeze(GAME_SCENES.map((scene) => createSceneFixture(scene)));
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
