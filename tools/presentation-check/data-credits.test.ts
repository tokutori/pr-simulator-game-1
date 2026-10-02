import { describe, expect, it } from "vitest";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { TERRAIN_ATTRIBUTION } from "../../web/src/app/data-attribution.js";
import { drawVrPanel } from "../../web/src/presentation/vr-panel-canvas.js";
import type { PanelDrawingContext } from "../../web/src/presentation/vr-panel-canvas.js";
import { validateUiViewModel } from "../../web/src/render/contracts/ui.js";

describe("shared data credits", () => {
  it.each(["screen", "phone-vr", "webxr"] as const)("opens and closes credits without a domain command in %s", (mode) => {
    const title = gameSessionState(0, 0, null);
    if (title === null) throw new Error("Invalid Title fixture");
    const model = {
      ...createInitialAppModel(), gameSession: title,
      presentation: { type: "ready" as const, mode }, webXrAvailable: true, phoneVrAvailable: true
    };
    const titleView = createGameViewModel(model, null);
    expect(titleView.panels[0]?.controls.find((control) => control.id === "data-credits-open"))
      .toMatchObject({ kind: "button", enabled: true });
    const opened = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "data-credits-open" } });
    expect(opened.effects).toEqual([]);
    expect(opened.model.presentation).toBe(model.presentation);
    expect(opened.model.gameSession).toEqual({ kind: "title", phaseCode: 0, overlay: "data-credits" });
    const credits = createGameViewModel(opened.model, null);
    validateUiViewModel(credits);
    expect(credits.activeOverlay).toBe("Credits");
    expect(credits.panels[0]?.anchor).toBe("menu");
    const text = credits.panels.flatMap((panel) => panel.controls)
      .flatMap((control) => control.kind === "status" ? [control.value] : []).join(" ").replace(/\s+/g, " ");
    for (const notice of Object.values(TERRAIN_ATTRIBUTION)) expect(text).toContain(notice);
    expect(text).toContain("https://www.openstreetmap.org/copyright");
    expect(gameSessionState(0, 0, null, false, opened.model.gameSession)).toEqual(opened.model.gameSession);
    const blocked = updateApp(opened.model, { type: "ui-action", action: { type: "activate", controlId: "game-title-start" } });
    expect(blocked.model).toBe(opened.model);
    expect(blocked.effects).toEqual([]);
    const closed = updateApp(opened.model, { type: "ui-action", action: { type: "activate", controlId: "data-credits-close" } });
    expect(closed.model.gameSession).toEqual(title);
    expect(closed.effects).toEqual([]);
    const backed = updateApp(opened.model, { type: "ui-action", action: { type: "back" } });
    expect(backed.model.gameSession).toEqual(title);
    expect(backed.effects).toEqual([]);
    const recentered = updateApp(opened.model, { type: "ui-action", action: { type: "recenter-menu" } });
    expect(recentered.effects).toEqual(mode === "screen" ? [] : [{ type: "recenter-menu" }]);
    if (mode !== "screen") {
      const exited = updateApp(opened.model, { type: "ui-action", action: { type: "activate", controlId: "boot-exit-vr" } });
      expect(exited.effects).toMatchObject([{ type: "switch-backend", mode: "screen" }]);
      expect(exited.model.gameSession).toEqual(opened.model.gameSession);
    }
  });

  it("renders every notice line inside the shared VR panel without intersecting the return button", () => {
    const title = gameSessionState(0, 0, null);
    if (title === null) throw new Error("Invalid Title fixture");
    const opened = updateApp({ ...createInitialAppModel(), gameSession: title }, {
      type: "ui-action", action: { type: "activate", controlId: "data-credits-open" }
    });
    const panel = createGameViewModel(opened.model, null).panels[0];
    if (panel === undefined) throw new Error("Missing Credits panel");
    const labels: string[] = [];
    const context: PanelDrawingContext = {
      clearRect() {}, fillRect() {}, strokeRect() {}, beginPath() {}, closePath() {}, rect() {}, clip() {},
      save() {}, restore() {}, fill() {}, moveTo() {}, lineTo() {}, stroke() {}, setFillStyle() {},
      setStrokeStyle() {}, setFont() {}, setTextBaseline() {}, setLineWidth() {}, setGlobalAlpha() {},
      fillText(text, x, y, maxWidth) {
        labels.push(text);
        expect(x).toBeGreaterThanOrEqual(0);
        expect(x + maxWidth).toBeLessThanOrEqual(1024);
        expect(y).toBeGreaterThan(0);
        expect(y).toBeLessThan(768);
      }
    };
    drawVrPanel(context, panel, 1024, 768);
    expect(labels.join(" ")).toContain(TERRAIN_ATTRIBUTION.modified);
    expect(labels.join(" ")).toContain(TERRAIN_ATTRIBUTION.liability);
    const back = panel.controls.find((control) => control.id === "data-credits-close");
    if (back === undefined) throw new Error("Missing Credits return control");
    for (const control of panel.controls.filter((control) => control.kind === "status")) {
      expect(control.rect.y + control.rect.height).toBeLessThan(back.rect.y);
      expect(control.value.split("\n").length * 22).toBeLessThanOrEqual(control.rect.height * 768);
    }
  });
});
