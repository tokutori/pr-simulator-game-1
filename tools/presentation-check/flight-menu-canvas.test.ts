import { describe, expect, it } from "vitest";
import { createInitialAppModel, gameSessionState } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { currentFlightDisplayFixture } from "../game-check/current-session-fixture.js";
import { drawFlightMenu, flightMenuCanvasSize, prepareFlightMenuPaint } from "../../web/src/presentation/flight-menu-canvas.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";

describe("Small gaze Menu Canvas metrics and physical aspect", () => {
  it.each([false, true])("preflights all labels without glyph squeeze (HUD unavailable=%s)", (unavailable) => {
    const snapshot = currentFlightDisplayFixture();
    const gameSession = gameSessionState(5, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing session");
    const view = createGameViewModel({ ...createInitialAppModel(), gameSession, presentation: { type: "ready", mode: "phone-vr" } }, snapshot,
      null, unavailable ? { kind: "unavailable", reason: "text-overflow" } : { kind: "absent" });
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing Menu");
    const size = flightMenuCanvasSize(panel);
    expect(size.width / size.height).toBeCloseTo(panel.size.width / panel.size.height, 2);
    const context = new HudCanvasFixture();
    const preparation = prepareFlightMenuPaint(context, panel, size.width, size.height, "ja");
    expect(preparation.locale).toBe("ja");
    expect(context.locales).toEqual(["ja"]);
    expect(preparation.kind).toBe("ready");
    expect(context.painted).toEqual([]);
    if (preparation.kind !== "ready") throw new Error("Missing Menu plan");
    expect(Object.isFrozen(preparation.texts)).toBe(true);
    for (const text of preparation.texts) expect(text.fontSize * panel.size.height / size.height).toBeGreaterThanOrEqual(0.024);
    drawFlightMenu(context, preparation);
    expect(context.locales).toEqual(["ja", "ja"]);
    expect(context.clears).toEqual([[0, 0, size.width, size.height]]);
    expect(context.painted).toContain("Pause");
    if (unavailable) expect(context.painted.join("")).toContain("情報板の文字を読み取れる大きさで配置できない。");
  });

  it("rejects missing metrics before any glyph or card is published", () => {
    const initial = createInitialAppModel();
    const snapshot = currentFlightDisplayFixture();
    const gameSession = gameSessionState(5, 0, snapshot, true);
    if (gameSession === null) throw new Error("Missing session");
    const panel = createGameViewModel({ ...initial, gameSession, presentation: { type: "ready", mode: "phone-vr" } }, null).panels[0];
    if (panel === undefined) throw new Error("Missing Menu");
    const context = new HudCanvasFixture();
    context.invalidMetrics = true;
    const preparation = prepareFlightMenuPaint(context, panel, 1024, 413, "ja");
    expect(preparation.kind).toBe("unavailable");
    expect(context.painted).toEqual([]);
    drawFlightMenu(context, preparation);
    expect(context.fills).toEqual([]);
    expect(context.painted).toEqual([]);
  });
});
