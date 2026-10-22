import { describe, expect, it } from "vitest";
import { drawMeasuredMenu, menuCanvasSize, menuPaintedTextInk } from "../../web/src/presentation/menu-canvas.js";
import type { MenuDrawingContext } from "../../web/src/presentation/menu-canvas.js";
import type { HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";
import { menuViewport, menuRectInViewport } from "../../web/src/presentation/menu-layout.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";

class RecordingContext implements MenuDrawingContext {
  font = "";
  locale = "";
  baseline = "";
  align = "";
  saves = 0;
  restores = 0;
  clips = 0;
  fillStyle = "";
  globalAlpha = 1;
  readonly drawings: { value: string; x: number; y: number; font: string; locale: string; baseline: string; align: string; color: string; alpha: number }[] = [];
  readonly boxes: { x: number; y: number; width: number; height: number; color: string; alpha: number }[] = [];
  readonly gradients: { x: number; y: number; width: number; height: number; colors: readonly [string, string] }[] = [];
  readonly translations: { x: number; y: number }[] = [];
  readonly rotations: number[] = [];
  readonly paths: { x: number; y: number }[] = [];
  read: (value: string, pixels: number) => HeadHudTextMetrics = (value, pixels) => {
    const width = Array.from(value).length * pixels * 0.6;
    const visible = value.trim() !== "";
    return { width, left: visible ? -pixels * 0.1 : 0, right: visible ? width + pixels * 0.1 : width,
      ascent: visible ? pixels * 0.6 : 0, descent: visible ? pixels * 0.2 : 0 };
  };
  clearRect(): void {}
  fillRect(coordinateX: number, coordinateY: number, width: number, height: number): void { this.boxes.push({ x: coordinateX, y: coordinateY, width, height, color: this.fillStyle, alpha: this.globalAlpha }); }
  fillGradientRect(x: number, y: number, width: number, height: number, colors: readonly [string, string]): void { this.gradients.push({ x, y, width, height, colors }); }
  fillText(value: string, coordinateX: number, coordinateY: number): void { this.drawings.push({ value, x: coordinateX, y: coordinateY,
    font: this.font, locale: this.locale, baseline: this.baseline, align: this.align, color: this.fillStyle, alpha: this.globalAlpha }); }
  strokeRect(): void {}
  beginPath(): void {}
  closePath(): void {}
  rect(): void {}
  clip(): void { this.clips++; }
  save(): void { this.saves++; }
  restore(): void { this.restores++; }
  fill(): void {}
  moveTo(coordinateX: number, coordinateY: number): void { this.paths.push({ x: coordinateX, y: coordinateY }); }
  lineTo(coordinateX: number, coordinateY: number): void { this.paths.push({ x: coordinateX, y: coordinateY }); }
  stroke(): void {}
  setFillStyle(value: string): void { this.fillStyle = value; }
  setStrokeStyle(): void {}
  setFont(value: string): void { this.font = value; }
  setTextBaseline(value: "middle" | "alphabetic"): void { this.baseline = value; }
  translate(coordinateX: number, coordinateY: number): void { this.translations.push({ x: coordinateX, y: coordinateY }); }
  rotate(radians: number): void { this.rotations.push(radians); }
  setLineWidth(): void {}
  setGlobalAlpha(value: number): void { this.globalAlpha = value; }
  setLocale(value: string): void { this.locale = value; }
  setTextAlign(value: "left"): void { this.align = value; }
  measureText(value: string): HeadHudTextMetrics {
    if (this.baseline !== "alphabetic") throw new Error("Font observation requires the drawing baseline");
    const match = this.font.match(/([\d.]+)px/);
    if (match?.[1] === undefined) throw new Error("Font must be explicit");
    return this.read(value, Number(match[1]));
  }
}


const font = { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 } as const;

function prepare(context: MenuDrawingContext, session: TailAppSessionFacade, platform: GameUiPlatformInput, generation = 0,
  viewer = configuredViewerFixture(1280, 720)) {
  const shared = session.readSharedUi(platform);
  const panel = createGameViewModel(shared).panels[0];
  if (panel === undefined) throw new Error("Missing shared Menu");
  const ready = prepareMenuPresentation(context, panel,
    { scope: { kind: "scene", scene: shared.scene, panelId: panel.id, viewKey: "canvas" }, generation: 1 }, 0, viewer,
    { theme: shared.theme, locale: "ja", viewRevision: shared.view_revision, font: { ...font, generation } },
    (input, measure) => session.prepareSharedMenu(input, platform, measure));
  if (ready.kind !== "ready") throw new Error(ready.kind === "unavailable" ? ready.reason : ready.kind);
  return ready;
}

describe("Shared Menu Canvas adapter", () => {
  it.each(["title", "flight_setup", "briefing_ready", "countdown", "flight_running", "flight_paused", "result", "replay", "attract"] as const)(
    "paints the actual %s Menu and keeps controls reachable in both optical orientations", phase => {
      withSharedGameUiSession(phase, { platform: { presentation_mode: "hmd" } }, (session, platform) => {
        for (const [width, height] of [[1280, 720], [720, 1280]] as const) {
          const context = new RecordingContext();
          const ready = prepare(context, session, platform, 0, configuredViewerFixture(width, height));
          const document = ready.viewport.document;
          const size = menuCanvasSize(ready.panel.size);
          for (const layout of document.controls) {
            const progress = Math.max(0, Math.min(1, layout.bounds.y / (ready.viewport.maximumOffsetMeters || 1)));
            const viewport = menuViewport(document, progress);
            expect(menuRectInViewport(viewport, layout.bounds).kind, layout.control.id).toBe("visible");
          }
          drawMeasuredMenu(context, ready.viewport, size.width, size.height);
          expect(context.drawings.length).toBeGreaterThan(0);
          expect(context.drawings.every(drawing => drawing.baseline === "alphabetic")).toBe(true);
          expect(context.saves).toBe(context.restores);
          expect(context.boxes[0]).toMatchObject({ color: document.request.theme.panel_background,
            alpha: document.request.theme.panel_opacity });
          const title = document.title.lines.find(line => line.value.length > 0);
          if (title !== undefined) {
            const request = document.title.request;
            expect(title.ascentMeters).toBeCloseTo(request.fontMeters * 0.6, 12);
            expect(title.descentMeters).toBeCloseTo(request.fontMeters * 0.2, 12);
            expect(title.leftMeters).toBeLessThan(0);
          }
        }
      });
    });

  it("retains font observations in the Rust owner and refreshes them for a font generation", () => {
    withSharedGameUiSession("title", { platform: { presentation_mode: "hmd" } }, (session, platform) => {
      const context = new RecordingContext();
      const read = context.read;
      let queries = 0;
      context.read = (value, pixels) => { queries++; return read(value, pixels); };
      const first = prepare(context, session, platform);
      expect(queries).toBeGreaterThan(0);
      queries = 0;
      const cached = prepare(context, session, platform);
      expect(queries).toBe(0);
      expect(cached.viewport.document).toEqual(first.viewport.document);
      prepare(context, session, platform, 1);
      expect(queries).toBeGreaterThan(0);
      const shared = session.readSharedUi(platform);
      const panel = createGameViewModel(shared).panels[0];
      if (panel === undefined) throw new Error("Missing Title Menu");
      session.updateSharedUi({ kind: "set_credits", open: true }, platform);
      const stale = prepareMenuPresentation(context, panel, first.context, 0, configuredViewerFixture(1280, 720),
        { theme: shared.theme, locale: "ja", viewRevision: shared.view_revision, font },
        (input, measure) => session.prepareSharedMenu(input, platform, measure));
      expect(stale).toEqual({ kind: "unavailable", reason: "invalid-layout" });
      expect(context.saves).toBe(context.restores);
    });
  });

  it.each(["invalid", "throw"] as const)("propagates %s font observations and restores Canvas state", failure => {
    withSharedGameUiSession("title", { platform: { presentation_mode: "hmd" } }, (session, platform) => {
      const shared = session.readSharedUi(platform);
      const panel = createGameViewModel(shared).panels[0];
      if (panel === undefined) throw new Error("Missing Title Menu");
      const context = new RecordingContext();
      context.read = () => {
        if (failure === "throw") throw new Error("Font service failed");
        return { width: NaN, left: 0, right: 0, ascent: 1, descent: 0 };
      };
      const result = prepareMenuPresentation(context, panel,
        { scope: { kind: "scene", scene: "Title", panelId: panel.id, viewKey: "font" }, generation: 1 }, 0,
        configuredViewerFixture(1280, 720), { theme: shared.theme, locale: "ja", viewRevision: shared.view_revision, font },
        (input, measure) => session.prepareSharedMenu(input, platform, measure));
      expect(result).toEqual({ kind: "unavailable", reason: failure === "throw" ? "measurement-unavailable" : "invalid-text-metrics" });
      expect(context.saves).toBe(context.restores);
    });
  });

  it.each(["map", "speed"] as const)("paints shared %s chart text baselines and the current frame after seeking", chart => {
    withSharedGameUiSession("result", { ticks: 20, platform: { presentation_mode: "hmd" },
      actions: [{ kind: "set_result_tab", tab: "analysis" }, { kind: "set_analysis_chart", chart }] }, (session, platform) => {
      const context = new RecordingContext();
      const first = prepare(context, session, platform);
      const document = first.viewport.document;
      const plot = document.controls.find(control => control.kind === "chart");
      if (plot?.kind !== "chart") throw new Error("Missing Analysis chart");
      const progress = Math.min(1, plot.bounds.y / (first.viewport.maximumOffsetMeters || 1));
      const viewport = menuViewport(document, progress);
      const size = menuCanvasSize(first.panel.size);
      drawMeasuredMenu(context, viewport, size.width, size.height);
      const inks = menuPaintedTextInk(viewport).filter(ink => ink.visible.kind === "visible");
      expect(inks.length).toBeGreaterThan(0);
      for (const text of plot.texts.filter(text => text.chartProjection !== undefined)) {
        const ink = inks.find(ink => ink.identity === text.request.identity);
        if (ink === undefined) continue;
        expect(context.translations.some(point => Math.abs(point.x * first.panel.size.width / size.width - ink.anchorX) < 1e-12
          && Math.abs(point.y * first.panel.size.height / size.height - ink.baselineY) < 1e-12)).toBe(true);
        expect(context.drawings.some(drawing => drawing.value === ink.value && drawing.baseline === "alphabetic")).toBe(true);
      }
      session.updateSharedUi({ kind: "seek_analysis", time_seconds: 0.1 }, platform);
      const next = prepare(context, session, platform);
      const values = next.viewport.document.controls.flatMap(control => control.texts.map(text => text.request.value));
      const shared = session.readSharedUi(platform);
      for (const control of shared.panels.flatMap(panel => panel.controls)) {
        if (control.content.kind === "range") expect(values).toContain(control.content.display_value);
        if (control.content.kind === "status" && control.content.value.length > 0) expect(values).toContain(control.content.value);
      }
      expect(context.saves).toBe(context.restores);
    });
  });
});
