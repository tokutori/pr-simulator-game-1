import { describe, expect, it } from "vitest";
import { drawMeasuredMenu, menuCanvasSize } from "../../web/src/presentation/menu-canvas.js";
import type { SurfaceDrawingContext, SurfaceTextMetrics } from "../../web/src/presentation/browser-canvas.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { menuActionAt } from "./menu-fixture.js";
import type { GameUiFrame } from "../../web/src/generated/game-ui-frame.js";

class RecordingContext implements SurfaceDrawingContext {
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
  read: (value: string, pixels: number) => SurfaceTextMetrics = (value, pixels) => {
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
  measureText(value: string): SurfaceTextMetrics {
    if (this.baseline !== "alphabetic") throw new Error("Font observation requires the drawing baseline");
    const match = this.font.match(/([\d.]+)px/);
    if (match?.[1] === undefined) throw new Error("Font must be explicit");
    return this.read(value, Number(match[1]));
  }
}


const font = { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 } as const;

function prepare(context: SurfaceDrawingContext, session: TailAppSessionFacade, platform: GameUiPlatformInput, generation = 0,
  viewer = configuredViewerFixture(1280, 720), progress = 0) {
  const shared = session.readSharedUi(platform);
  const panel = createGameViewModel(shared).panels[0];
  if (panel === undefined) throw new Error("Missing shared Menu");
  const ready = prepareMenuPresentation(context, panel,
    { scope: { panelId: panel.id, viewKey: "canvas" }, generation: 1 }, progress, viewer,
    { locale: "ja", viewRevision: shared.view_revision, font: { ...font, generation } },
    (input, measure) => session.prepareSharedMenu(input, platform, measure));
  if (ready.kind !== "ready") throw new Error(ready.kind === "unavailable" ? ready.reason : ready.kind);
  return ready;
}

describe("Shared Menu Canvas adapter", () => {
  it.each(["title", "flight_paused"] as const)(
    "paints the shared %s projection with the observed Canvas baseline in both orientations", phase => {
      withSharedGameUiSession(phase, { platform: { presentation_mode: "hmd" } }, (session, platform) => {
        for (const [width, height] of [[1280, 720], [720, 1280]] as const) {
          const context = new RecordingContext();
          const ready = prepare(context, session, platform, 0, configuredViewerFixture(width, height));
          const size = menuCanvasSize(ready.panel.size);
          drawMeasuredMenu(context, ready.viewport, size.width, size.height);
          expect(context.drawings.length).toBeGreaterThan(0);
          expect(context.drawings.every(drawing => drawing.baseline === "alphabetic")).toBe(true);
          expect(context.saves).toBe(context.restores);
          expect(context.boxes[0]).toMatchObject({ color: ready.viewport.background, alpha: ready.viewport.opacity });
          const region = ready.viewport.surface.regions.find(region => region.target.kind === "control");
          if (region?.target.kind !== "control") throw new Error("Missing shared input target");
          const [left, top, targetWidth, targetHeight] = region.bounds;
          expect(menuActionAt(ready, { x: left + targetWidth / 2 - ready.panel.size.width / 2, y: ready.panel.size.height / 2 - top - targetHeight / 2 }))
            .toMatchObject({ kind: "action", action: { type: "menu-control", action: { controlId: region.target.id } } });
          expect(context.boxes.some(box => Math.abs(box.x - left * size.width / ready.panel.size.width) < 1e-10
            && Math.abs(box.y - top * size.height / ready.panel.size.height) < 1e-10)).toBe(true);
          const text = ready.viewport.content.items.find(item => item.kind === "text");
          if (text?.kind !== "text") throw new Error("Missing shared text");
          expect(context.translations.some(point => Math.abs(point.x - text.anchor.x * size.width / ready.panel.size.width) < 1e-10 && Math.abs(point.y - text.anchor.y * size.height / ready.panel.size.height) < 1e-10)).toBe(true);
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
      expect(cached.viewport).toEqual(first.viewport);
      prepare(context, session, platform, 1);
      expect(queries).toBeGreaterThan(0);
      const shared = session.readSharedUi(platform);
      const panel = createGameViewModel(shared).panels[0];
      if (panel === undefined) throw new Error("Missing Title Menu");
      session.updateSharedUi({ kind: "set_credits", open: true }, platform);
      const stale = prepareMenuPresentation(context, panel, first.context, 0, configuredViewerFixture(1280, 720),
        { locale: "ja", viewRevision: shared.view_revision, font },
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
        { scope: { panelId: panel.id, viewKey: "font" }, generation: 1 }, 0,
        configuredViewerFixture(1280, 720), { locale: "ja", viewRevision: shared.view_revision, font },
        (input, measure) => session.prepareSharedMenu(input, platform, measure));
      expect(result).toEqual({ kind: "unavailable", reason: failure === "throw" ? "measurement-unavailable" : "invalid-text-metrics" });
      expect(context.saves).toBe(context.restores);
    });
  });

  it.each(["map", "speed"] as const)("paints shared %s chart text baselines and the current frame after seeking", chart => {
    withSharedGameUiSession("result", { ticks: 20, platform: { presentation_mode: "hmd" },
      actions: [{ kind: "set_result_tab", tab: "analysis" }, { kind: "set_analysis_chart", chart }] }, (session, platform, bridge) => {
      const published = session.readSharedUi(platform);
      const context = new RecordingContext();
      const first = prepare(context, session, platform);
      const ready = [0, 0.5, 1].map(progress => prepare(context, session, platform, 0, configuredViewerFixture(1280, 720), progress))
        .find(menu => menu.viewport.content.items.some(item => item.kind === "plot"));
      if (ready === undefined) throw new Error("Missing Analysis plot");
      const size = menuCanvasSize(ready.panel.size);
      drawMeasuredMenu(context, ready.viewport, size.width, size.height);
      const rotated = ready.viewport.content.items.filter(item => item.kind === "text" && item.rotationDegrees !== 0);
      expect(rotated.length).toBeGreaterThan(0);
      for (const text of rotated) {
        if (text.kind !== "text") continue;
        expect(context.translations.some(point => Math.abs(point.x - text.anchor.x * size.width / ready.panel.size.width) < 1e-10 && Math.abs(point.y - text.anchor.y * size.height / ready.panel.size.height) < 1e-10)).toBe(true);
        expect(context.drawings.some(drawing => drawing.value === text.value && drawing.baseline === "alphabetic")).toBe(true);
      }
      session.updateSharedUi({ kind: "seek_analysis", time_seconds: 0.1 }, platform);
      const next = [0, 0.5, 1].map(progress => {
        const menu = prepareMenuPresentation(context, first.panel, first.context, progress, configuredViewerFixture(1280, 720),
          { locale: "ja", viewRevision: published.view_revision, font },
          (input, measure) => session.prepareSharedMenu(input, platform, measure));
        if (menu.kind !== "ready") throw new Error("Missing retained Menu after seek");
        return menu;
      });
      const retained = next.map(menu => menu.viewport.content.items
        .filter(item => item.kind === "text").map(item => item.value).join("").replace(/\s/g, ""));
      const frame = JSON.parse(bridge.game_ui_frame_json(published.view_revision, JSON.stringify(platform))) as GameUiFrame;
      for (const value of [...frame.ranges.map(range => range.display_value), ...frame.readouts.map(readout => readout.value)].filter(value => value.length > 0)) {
        expect(retained.some(text => text.includes(value.replace(/\s/g, "")))).toBe(true);
      }
      expect(context.saves).toBe(context.restores);
    });
  });
});
