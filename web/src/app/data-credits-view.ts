import { IDENTITY_POSE } from "../render/contracts/math.js";
import { normalizedRect } from "../render/contracts/ui.js";
import type { UiStatus, UiViewModel } from "../render/contracts/ui.js";
import { TERRAIN_ATTRIBUTION } from "./data-attribution.js";

export function createDataCreditsViewModel(): UiViewModel {
  return Object.freeze({
    scene: "Title",
    title: "地図・地形データの帰属表示",
    description: "ScreenとVRで共通の利用条件を表示する。",
    activeOverlay: "Credits",
    panels: Object.freeze([Object.freeze({
      id: "data-credits",
      title: "Data credits",
      anchor: "menu",
      localPose: IDENTITY_POSE,
      size: Object.freeze({ width: 2.4, height: 1.8 }),
      controls: Object.freeze([
        notice("data-credits-map", "Map / Terrain", `© OpenStreetMap contributors\nhttps://www.openstreetmap.org/copyright\n${TERRAIN_ATTRIBUTION.jaxa}\nhttps://earth.jaxa.jp/en/data/policy/`, 0.20, 0.20),
        notice("data-credits-copernicus", "Copernicus GLO-30", wrapNotice(TERRAIN_ATTRIBUTION.copernicus), 0.43, 0.15),
        notice("data-credits-modified", "Modification", TERRAIN_ATTRIBUTION.modified, 0.61, 0.06),
        notice("data-credits-liability", "Liability", wrapNotice(TERRAIN_ATTRIBUTION.liability), 0.70, 0.14),
        Object.freeze({
          kind: "button", id: "data-credits-close", label: "Titleへ戻る", enabled: true,
          rect: normalizedRect(0.08, 0.87, 0.84, 0.075)
        })
      ])
    })])
  });
}

function notice(id: string, label: string, value: string, top: number, height: number): UiStatus {
  return Object.freeze({ kind: "status", id, label, value, enabled: false, rect: normalizedRect(0.08, top, 0.84, height) });
}

function wrapNotice(value: string): string {
  const lines: string[] = [];
  let line = "";
  for (const word of value.split(" ")) {
    if (line.length + word.length + 1 > 76) {
      lines.push(line);
      line = word;
    } else {
      line = line === "" ? word : `${line} ${word}`;
    }
  }
  lines.push(line);
  return lines.join("\n");
}
