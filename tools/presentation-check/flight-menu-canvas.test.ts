import { describe, expect, it } from "vitest";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { currentSharedGameUiFixture } from "../game-check/shared-game-ui-fixture.js";
import { drawFlightMenu, flightMenuCanvasSize, prepareFlightMenuPaint } from "../../web/src/presentation/flight-menu-canvas.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";

describe("Small gaze Menu Canvas metrics and physical aspect", () => {
  it.each([false, true])("preflights all labels without glyph squeeze (HUD unavailable=%s)", (unavailable) => {
    const view = createGameViewModel(currentSharedGameUiFixture(5, { platform: { presentation_mode: "phone_vr",
      head_hud_failure: unavailable ? "情報板の文字を読み取れる大きさで配置できない。" : null } }));
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
    const panel = createGameViewModel(currentSharedGameUiFixture(5, { platform: { presentation_mode: "phone_vr" } })).panels[0];
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

  it.each([{ head: true, controller: false }, { head: false, controller: true }, { head: true, controller: true }])(
    "fits independent resource diagnostics and all exit controls without obscuring the normal upper edge (%j)", ({ head, controller }) => {
    const headCause = "情報板の文字を読み取れる大きさで配置できない。";
    const controllerCause = "input disposal failed";
    const normal = createGameViewModel(currentSharedGameUiFixture(5, { platform: { presentation_mode: "phone_vr" } })).panels[0];
    const panel = createGameViewModel(currentSharedGameUiFixture(5, { platform: {
      presentation_mode: "phone_vr", head_hud_failure: head ? headCause : null, flight_controller_failure: controller ? controllerCause : null
    } })).panels[0];
    if (normal === undefined || panel === undefined) throw new Error("Missing Menu");
    expect(panel.localPose.position.y + panel.size.height / 2).toBeCloseTo(normal.localPose.position.y + normal.size.height / 2, 12);
    const size = flightMenuCanvasSize(panel);
    const context = new HudCanvasFixture();
    const preparation = prepareFlightMenuPaint(context, panel, size.width, size.height, "ja");
    expect(preparation.kind).toBe("ready");
    if (preparation.kind !== "ready") throw new Error("Missing resource failure Menu plan");
    for (const text of preparation.texts) {
      const fontMeters = text.fontSize * panel.size.height / size.height;
      const tolerance = 4 * Number.EPSILON * Math.max(Math.abs(fontMeters), 0.024);
      expect(fontMeters + tolerance).toBeGreaterThanOrEqual(0.024);
    }
    drawFlightMenu(context, preparation);
    if (controller) expect(context.painted).not.toContain("Pause");
    else expect(context.painted).toContain("Pause");
    if (head) expect(context.painted.join("")).toContain(headCause);
    if (controller) {
      expect(context.painted.join("")).toContain(controllerCause);
      expect(context.painted).toContain("飛行を終了");
    }
  });
});
