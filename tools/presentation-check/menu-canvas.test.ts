import { describe, expect, it } from "vitest";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import type { UiPanel } from "../../web/src/render/contracts/ui.js";
import { normalizedRect } from "../../web/src/render/contracts/ui.js";
import { drawMeasuredMenu, measureMenuLayout, menuCanvasSize, menuPaintedTextInk } from "../../web/src/presentation/menu-canvas.js";
import type { HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";
import type { MenuDrawingContext } from "../../web/src/presentation/menu-canvas.js";
import { hitMenuViewport, layoutMeasuredMenu, menuViewport, requestMenuLayout, validateMenuOpeningInk } from "../../web/src/presentation/menu-layout.js";
import { fitMenuView } from "../../web/src/presentation/menu-view-fit.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { createAllSceneFixtures, createSceneFixture } from "./ui-adapter-fixture.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import type { MenuMeasurementCache } from "../../web/src/presentation/menu-preparation.js";
import { menuActionAt, menuContextCanInteract, menuInputGeometry, menuInputGeometryChanged, menuPageProgress } from "../../web/src/presentation/menu-interaction.js";
import { menuDocumentPointToPanel, menuRectInViewport } from "../../web/src/presentation/menu-layout.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { currentSharedGameUiFixture } from "../game-check/shared-game-ui-fixture.js";

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
    const match = this.font.match(/([\d.]+)px/);
    if (match?.[1] === undefined) throw new Error("Font must be explicit");
    return this.read(value, Number(match[1]));
  }
}

const panel: UiPanel = { id: "menu", title: "Pause", anchor: "menu", localPose: IDENTITY_POSE, size: { width: 0.5, height: 1.8 }, controls: [
  { kind: "button", id: "resume", label: "Resume", enabled: true, rect: normalizedRect(0.04, 0.2, 0.92, 0.08) },
  { kind: "status", id: "help", label: "方法", value: "A\n\nB\n", enabled: false, rect: normalizedRect(0.04, 0.4, 0.92, 0.2) }
] };
const style = { theme: currentSharedGameUiFixture("title").theme, fontMeters: 0.04, font: { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 } as const,
  locale: "ja", previousLabel: "前頁", nextLabel: "次頁", caption: "Build" };

function prepare(context: MenuDrawingContext, source = panel, presentation = style) {
  const request = requestMenuLayout(source, source.size, presentation);
  const canvas = menuCanvasSize(source.size);
  const measurement = measureMenuLayout(context, request, canvas.width, canvas.height);
  if (measurement.kind !== "ready") throw new Error(measurement.reason);
  const result = layoutMeasuredMenu(request, measurement.measurements);
  if (result.kind !== "ready") throw new Error(result.reason);
  return { request, canvas, measurement, document: result.document };
}

describe("Canvas measurement boundary and shared Menu painting", () => {
  it("keeps shared section headings and paints solid and gradient legend swatches after measurement", () => {
    const view = createGameViewModel(currentSharedGameUiFixture("result", { ticks: 20, actions: [{ kind: "set_result_tab", tab: "analysis" }] }));
    const plot = view.panels.flatMap((panel) => panel.controls).find((control) => control.kind === "chart");
    if (plot?.kind !== "chart") throw new Error("Missing chart fixture");
    const legend = [
      { label: "AIR SPEED", color: "#ee1234", segment_gradient: null },
      { label: "GROUND SPEED", color: "#1234ee", segment_gradient: null },
      { label: "PROGRESS", color: "#112233", segment_gradient: ["#112233", "#aabbcc"] as const }
    ];
    const source: UiPanel = { ...panel, size: { width: 2.4, height: 4 },
      sections: [{ id: "chart", label: "共通の見出し", layout: "column" }],
      controls: [{ ...plot, section: "chart", presentation: { ...plot.presentation, legend } }] };
    const context = new RecordingContext();
    const prepared = prepare(context, source);
    expect(prepared.request.texts.filter((text) => text.legend !== undefined).map((text) => text.legend)).toEqual(legend);
    drawMeasuredMenu(context, menuViewport(prepared.document, 0), prepared.canvas.width, prepared.canvas.height);
    expect(context.drawings.some((text) => text.value === "共通の見出し")).toBe(true);
    for (const color of ["#ee1234", "#1234ee"]) expect(context.boxes.some((box) => box.color === color && box.width > 0 && box.height > 0)).toBe(true);
    expect(context.gradients.some((gradient) => gradient.colors[0] === "#112233" && gradient.colors[1] === "#aabbcc" && gradient.width > 0)).toBe(true);
  });

  it("paints the supplied shared theme for panels, text and every control emphasis without a local palette", () => {
    const theme = { ...style.theme, panel_background: "#102030", text: "#d1e2f3", muted_text: "#a1b2c3",
      primary_background: "#234567", normal_background: "#345678", choice_background: "#456789", panel_opacity: 0.37 };
    const source: UiPanel = { ...panel, size: { width: 2.4, height: 3 }, controls: [
      { kind: "button", id: "normal", label: "Normal", enabled: true, rect: normalizedRect(0, 0, 1, 0.08) },
      { kind: "button", id: "primary", label: "Primary", enabled: true, rect: normalizedRect(0, 0.1, 1, 0.08),
        presentation: { kind: "action", emphasis: "primary" } },
      { kind: "button", id: "secondary", label: "Secondary", enabled: true, rect: normalizedRect(0, 0.2, 1, 0.08),
        presentation: { kind: "action", emphasis: "secondary" } },
      { kind: "button", id: "selected", label: "Selected", enabled: true, rect: normalizedRect(0, 0.3, 1, 0.08),
        presentation: { kind: "choice", group: "choice", groupLabel: "Choice", selected: true, description: "Selected detail" } },
      { kind: "button", id: "choice", label: "Choice", enabled: true, rect: normalizedRect(0, 0.4, 1, 0.08),
        presentation: { kind: "choice", group: "choice", groupLabel: "Choice", selected: false, description: "Choice detail" } },
      { kind: "button", id: "disclosure", label: "Disclosure", enabled: true, rect: normalizedRect(0, 0.5, 1, 0.08),
        presentation: { kind: "disclosure", expanded: false } },
      { kind: "button", id: "disabled", label: "Disabled", enabled: false, rect: normalizedRect(0, 0.6, 1, 0.08) },
      { kind: "status", id: "status", label: "Status", value: "Ready", enabled: false, rect: normalizedRect(0, 0.7, 1, 0.08) }
    ] };
    const context = new RecordingContext();
    const prepared = prepare(context, source, { ...style, theme });
    const viewport = menuViewport(prepared.document, 0);
    expect(viewport.maximumOffsetMeters).toBe(0);
    expect(prepared.request.theme).toBe(theme);
    drawMeasuredMenu(context, viewport, prepared.canvas.width, prepared.canvas.height);
    expect(context.boxes[0]).toEqual({ x: 0, y: 0, width: prepared.canvas.width, height: prepared.canvas.height,
      color: theme.panel_background, alpha: theme.panel_opacity });
    const backgrounds = new Map([
      ["normal", theme.normal_background], ["primary", theme.primary_background], ["secondary", theme.panel_background],
      ["selected", theme.primary_background], ["choice", theme.choice_background], ["disclosure", theme.panel_background],
      ["disabled", theme.normal_background], ["status", theme.choice_background]
    ]);
    for (const [index, layout] of prepared.document.controls.entries()) {
      const clipped = menuRectInViewport(viewport, layout.bounds);
      if (clipped.kind !== "visible") throw new Error("Missing visible themed control");
      const rectangle = context.boxes[index + 1];
      if (rectangle === undefined) throw new Error("Missing themed control paint");
      expect(rectangle).toMatchObject({ color: backgrounds.get(layout.control.id), alpha: 1 });
      expect(rectangle.x).toBeCloseTo(clipped.rect.x * prepared.canvas.width / source.size.width, 10);
      expect(rectangle.y).toBeCloseTo(clipped.rect.y * prepared.canvas.height / source.size.height, 10);
      expect(rectangle.width).toBeCloseTo(clipped.rect.width * prepared.canvas.width / source.size.width, 10);
      expect(rectangle.height).toBeCloseTo(clipped.rect.height * prepared.canvas.height / source.size.height, 10);
    }
    expect(context.drawings.find((draw) => draw.value === source.title)).toMatchObject({ color: theme.text, alpha: 1 });
    expect(context.drawings.find((draw) => draw.value === style.caption)).toMatchObject({ color: theme.muted_text, alpha: 1 });
    expect(context.drawings.find((draw) => draw.value === "Disabled")).toMatchObject({ color: theme.muted_text, alpha: 1 });
    for (const value of ["Normal", "Primary", "Secondary", "● Selected", "○ Choice", "▸ Disclosure", "Status", "Ready"]) {
      expect(context.drawings.find((draw) => draw.value === value)).toMatchObject({ color: theme.text, alpha: 1 });
    }
  });

  it("paints page controls using the current shared normal background and enabled or muted text", () => {
    const theme = { ...style.theme, panel_background: "#243546", text: "#daeafb", muted_text: "#9aabbc",
      normal_background: "#526374", panel_opacity: 0.49 };
    const source = { ...panel, size: { width: 0.5, height: 0.45 } };
    const prepared = prepare(new RecordingContext(), source, { ...style, theme });
    for (const progress of [0, 1]) {
      const context = new RecordingContext();
      const viewport = menuViewport(prepared.document, progress);
      expect(viewport.maximumOffsetMeters).toBeGreaterThan(0);
      drawMeasuredMenu(context, viewport, prepared.canvas.width, prepared.canvas.height);
      for (const [index, bounds] of [viewport.previousBounds, viewport.nextBounds].entries()) {
        const rectangle = context.boxes[context.boxes.length - 2 + index];
        if (rectangle === undefined) throw new Error("Missing themed page paint");
        expect(rectangle).toMatchObject({ color: theme.normal_background, alpha: 1 });
        expect(rectangle.x).toBeCloseTo(bounds.x * prepared.canvas.width / source.size.width, 10);
        expect(rectangle.y).toBeCloseTo(bounds.y * prepared.canvas.height / source.size.height, 10);
        expect(rectangle.width).toBeCloseTo(bounds.width * prepared.canvas.width / source.size.width, 10);
        expect(rectangle.height).toBeCloseTo(bounds.height * prepared.canvas.height / source.size.height, 10);
      }
      expect(context.drawings.find((draw) => draw.value === style.previousLabel)).toMatchObject({
        color: progress === 0 ? theme.muted_text : theme.text, alpha: 1 });
      expect(context.drawings.find((draw) => draw.value === style.nextLabel)).toMatchObject({
        color: progress === 1 ? theme.muted_text : theme.text, alpha: 1 });
    }
  });

  it("projects shared chart baselines, anchors, rotation and series cursor colours without a second label layout", () => {
    for (const kind of ["map", "speed"] as const) {
      const view = createGameViewModel(currentSharedGameUiFixture("result", { ticks: 20, actions: [
        { kind: "set_result_tab", tab: "analysis" }, { kind: "set_analysis_chart", chart: kind }
      ] }));
      const chart = view.panels.flatMap((entry) => entry.controls).find((control) => control.kind === "chart");
      if (chart?.kind !== "chart") throw new Error("Missing shared chart");
      const source: UiPanel = { ...panel, size: { width: 2.4, height: 4 }, controls: [chart] };
      const context = new RecordingContext();
      const prepared = prepare(context, source);
      const layout = prepared.document.controls[0];
      if (layout?.kind !== "chart") throw new Error("Missing measured chart");
      const [left, top, right, bottom] = chart.presentation.plot;
      for (const text of layout.texts) {
        const descriptor = text.request.chartText;
        if (descriptor === undefined) continue;
        const projection = text.chartProjection;
        if (projection === undefined) throw new Error("Missing shared chart text projection");
        expect(projection.anchor).toBe(descriptor.anchor);
        expect(projection.rotationDegrees).toBe(descriptor.rotation_degrees);
        expect((projection.baseline.x - layout.plot.x) / layout.plot.width).toBeCloseTo((descriptor.position.x - left) / (right - left), 12);
        expect((projection.baseline.y - layout.plot.y) / layout.plot.height).toBeCloseTo((descriptor.position.y - top) / (bottom - top), 12);
      }
      const viewport = menuViewport(prepared.document, 0);
      drawMeasuredMenu(context, viewport, prepared.canvas.width, prepared.canvas.height);
      const inks = menuPaintedTextInk(viewport);
      for (const text of layout.texts.filter((entry) => entry.chartProjection !== undefined)) {
        const ink = inks.find((entry) => entry.identity === text.request.identity);
        if (ink === undefined || ink.visible.kind !== "visible") throw new Error("Shared label is unreachable");
        expect(context.translations.some((translation) => Math.abs(translation.x * source.size.width / prepared.canvas.width - ink.anchorX) < 1e-12
          && Math.abs(translation.y * source.size.height / prepared.canvas.height - ink.baselineY) < 1e-12)).toBe(true);
        expect(context.drawings.some((drawing) => drawing.value === ink.value && drawing.x === 0 && drawing.y === 0 && drawing.baseline === "alphabetic")).toBe(true);
      }
      expect(context.rotations).toContain(-Math.PI / 2);
      for (const cursor of chart.cursor_points) expect(context.boxes.some((box) => box.width === 8 && box.height === 8 && box.color === cursor.color)).toBe(true);
      expect(context.saves).toBe(context.restores);
    }
  });

  it("reflows every semantic Scene to both profile orientations and preserves reachable controls", () => {
    for (const view of [...createAllSceneFixtures(), createSceneFixture("Flight", "Pause"), createSceneFixture("Flight", "Pause/Settings")]) {
      const source = view.panels[0];
      if (source === undefined) throw new Error("Missing Scene fixture panel");
      const menuPanel: UiPanel = { ...source, anchor: "menu" };
      for (const [width, height] of [[1280, 720], [720, 1280]]) {
        if (width === undefined || height === undefined) throw new Error("Missing profile dimensions");
        const context = new RecordingContext();
        const viewer = configuredViewerFixture(width, height);
        const ready = prepareMenuPresentation(context, menuPanel,
          { scope: { kind: "scene", scene: view.scene, panelId: source.id, viewKey: "main" }, generation: 1 }, 0, viewer,
          { theme: style.theme, locale: "ja", caption: view.description, font: style.font }, new Map());
        if (ready.kind !== "ready") throw new Error(`${view.scene}/${String(width)}: ${ready.kind === "unavailable" ? ready.reason : ready.kind}`);
        expect(ready.viewport.document.controls).toHaveLength(source.controls.length);
        for (const control of source.controls) expect(ready.viewport.document.controls.find((layout) => layout.control.id === control.id)?.control).toBe(control);
        const fit = fitMenuView(viewer, ready.panel.size, ready.panel.localPose);
        if (fit.kind !== "ready") throw new Error("Missing fitted menu");
        expect(validateMenuOpeningInk(ready.viewport.document, viewer, fit.headFromMenu).kind).toBe("ready");
        const found = new Set<string>();
        for (let pageIndex = 0; pageIndex <= 100; pageIndex++) {
          const viewport = menuViewport(ready.viewport.document, pageIndex / 100);
          for (const layout of viewport.document.controls) if (menuRectInViewport(viewport, layout.bounds).kind === "visible") found.add(layout.control.id);
        }
        expect(found.size).toBe(source.controls.length);
        const dimensions = menuCanvasSize(ready.panel.size);
        drawMeasuredMenu(context, ready.viewport, dimensions.width, dimensions.height);
        expect(context.clips).toBeGreaterThan(0);
      }
    }
  });

  it("uses the same measured viewport for page controls, clipped ranges and paint without a second geometry", () => {
    const context = new RecordingContext();
    const rangePanel: UiPanel = { ...panel, controls: [...panel.controls,
      { kind: "range", id: "trim", label: "Trim", value: 0, displayValue: "0.0", minimum: -1, maximum: 1, step: 0.1, enabled: true, rect: normalizedRect(0.04, 0.7, 0.92, 0.08) }] };
    const ready = prepareMenuPresentation(context, rangePanel,
      { scope: { kind: "overlay", scene: "Flight", overlay: "Pause", panelId: panel.id, viewKey: "main" }, generation: 7 }, 0,
      configuredViewerFixture(720, 1280), { theme: style.theme, locale: "ja", caption: "Long note ".repeat(80), font: style.font }, new Map());
    if (ready.kind !== "ready") throw new Error("Missing prepared Menu");
    expect(ready.viewport.maximumOffsetMeters).toBeGreaterThan(0);
    const bounds = ready.viewport.nextBounds;
    const point = { x: bounds.x + bounds.width / 2 - ready.panel.size.width / 2,
      y: ready.panel.size.height / 2 - bounds.y - bounds.height / 2 };
    expect(menuActionAt(ready, point)).toEqual({ kind: "action", action: { type: "menu-scroll", context: ready.context,
      intent: { kind: "page", direction: "next", pageProgress: menuPageProgress(ready) } } });
    const resume = ready.viewport.document.controls.find((layout) => layout.control.id === "resume");
    if (resume === undefined) throw new Error("Missing Resume");
    const resumePoint = menuDocumentPointToPanel(ready.viewport,
      { x: resume.bounds.x + resume.bounds.width / 2, y: resume.bounds.y + resume.bounds.height / 2 });
    expect(menuActionAt(ready, resumePoint)).toEqual({ kind: "action", action: { type: "menu-control", context: ready.context,
      action: { type: "activate", controlId: "resume" } } });
    const trim = ready.viewport.document.controls.find((layout) => layout.control.id === "trim");
    if (trim === undefined) throw new Error("Missing Trim");
    const progress = (trim.bounds.y + trim.bounds.height / 2) / ready.viewport.maximumOffsetMeters;
    const viewport = menuViewport(ready.viewport.document, progress);
    const rangePoint = menuDocumentPointToPanel(viewport,
      { x: trim.bounds.x + trim.bounds.width / 4, y: trim.bounds.y + trim.bounds.height * 0.75 });
    expect(menuActionAt({ ...ready, viewport }, rangePoint)).toEqual({ kind: "action", action: { type: "menu-control", context: ready.context,
      action: { type: "set-range", controlId: "trim", value: -0.5 } } });
  });

  it("keeps compact Flight controls readable and fully visible without unnecessary page controls", () => {
    const compact: UiPanel = { ...panel, title: "Pause", size: { width: 0.62, height: 0.25 },
      controls: [{ kind: "button", id: "pause", label: "一時停止", enabled: true, rect: normalizedRect(0, 0, 1, 1) }] };
    const prepared = prepareMenuPresentation(new RecordingContext(), compact,
      { scope: { kind: "scene", scene: "Flight", panelId: compact.id, viewKey: "main" }, generation: 1 }, 0,
      configuredViewerFixture(720, 1280), { theme: style.theme, locale: "ja", caption: "Flight status", font: style.font }, new Map());
    if (prepared.kind !== "ready") throw new Error(`Compact Menu ${prepared.kind === "unavailable" ? prepared.reason : prepared.kind}`);
    expect(prepared.viewport.maximumOffsetMeters).toBe(0);
    expect(prepared.viewport.nextBounds.height).toBe(0);
    const control = prepared.viewport.document.controls[0];
    if (control === undefined) throw new Error("Missing Pause");
    expect(menuRectInViewport(prepared.viewport, control.bounds).kind).toBe("visible");
  });

  it("rejects old layout input immediately while invalidation is queued and after the generation changes", () => {
    const context = new RecordingContext();
    const scroll = { scope: { kind: "overlay" as const, scene: "Flight" as const, overlay: "Pause", panelId: panel.id, viewKey: "main" }, generation: 7 };
    const viewer = configuredViewerFixture(720, 1280);
    const first = prepareMenuPresentation(context, panel, scroll, 0, viewer, { theme: style.theme, locale: "ja", caption: "", font: style.font }, new Map());
    if (first.kind !== "ready") throw new Error("Missing layout A");
    const resume = first.viewport.document.controls.find((layout) => layout.control.id === "resume");
    if (resume === undefined) throw new Error("Missing Resume");
    const oldAction = menuActionAt(first, menuDocumentPointToPanel(first.viewport,
      { x: resume.bounds.x + resume.bounds.width / 2, y: resume.bounds.y + resume.bounds.height / 2 }));
    if (oldAction.kind !== "action" || oldAction.action.type !== "menu-control") throw new Error("Missing old action");
    const second = prepareMenuPresentation(context, panel, scroll, 0, viewer,
      { theme: style.theme, locale: "ja", caption: "Long paragraph ".repeat(100), font: style.font }, new Map());
    if (second.kind !== "ready") throw new Error("Missing layout B");
    expect(menuInputGeometryChanged(menuInputGeometry(first), second)).toBe(true);
    const oldState = { kind: "active" as const, ...scroll, progress: 0 };
    expect(menuContextCanInteract({ kind: "pending" }, oldState, oldAction.action.context)).toBe(false);
    const currentState = { ...oldState, generation: 8 };
    const currentMenu = { ...second, context: { ...scroll, generation: 8 } };
    expect(menuContextCanInteract(currentMenu, currentState, oldAction.action.context)).toBe(false);
    expect(menuContextCanInteract(currentMenu, currentState, currentMenu.context)).toBe(true);
    const sameGeometry = prepareMenuPresentation(context, { ...panel, controls: panel.controls.map((control) => control.kind === "status"
      ? { ...control, value: "C\n\nD\n" } : control) }, scroll, 0, viewer, { theme: style.theme, locale: "ja", caption: "", font: style.font }, new Map());
    if (sameGeometry.kind !== "ready") throw new Error("Missing equal geometry");
    expect(menuInputGeometryChanged(menuInputGeometry(first), sameGeometry)).toBe(false);
  });
  it("preserves signed ink bearings and converts horizontal and vertical CSS pixels once despite canvas rounding", () => {
    const context = new RecordingContext();
    const prepared = prepare(context);
    const title = prepared.measurement.measurements[0]?.lines[0];
    if (title === undefined) throw new Error("Missing title");
    const fontPixels = style.fontMeters * prepared.canvas.height / panel.size.height;
    expect(title.leftMeters).toBeCloseTo(-fontPixels * 0.1 * panel.size.width / prepared.canvas.width, 12);
    expect(title.ascentMeters).toBeCloseTo(style.fontMeters * 0.6, 12);
    expect(title.descentMeters).toBeCloseTo(style.fontMeters * 0.2, 12);
    expect(title.advanceMeters).toBeCloseTo(5 * fontPixels * 0.6 * panel.size.width / prepared.canvas.width, 12);
    expect(context.saves).toBe(context.restores);
  });

  it("wraps exact measured strings and preserves explicit and empty paragraphs without passing newline to Canvas", () => {
    const context = new RecordingContext();
    const originalRead = context.read;
    const strings: string[] = [];
    context.read = (value, pixels) => { strings.push(value); return originalRead(value, pixels); };
    const prepared = prepare(context);
    const request = prepared.request.texts.find((text) => text.slot.kind === "control" && text.slot.controlId === "help" && text.slot.role === "value");
    const measurement = prepared.measurement.measurements.find((entry) => entry.identity === request?.identity);
    expect(measurement?.lines.map((line) => [line.paragraphIndex, line.value])).toEqual([[0, "A"], [1, ""], [2, "B"], [3, ""]]);
    expect(strings.every((value) => !value.includes("\n"))).toBe(true);
  });

  it("draws with the same locale/font/baseline/alignment as measurement and derives real ink position from signed metrics", () => {
    const context = new RecordingContext();
    const prepared = prepare(context);
    const viewport = menuViewport(prepared.document, 0);
    drawMeasuredMenu(context, viewport, prepared.canvas.width, prepared.canvas.height);
    expect(context.drawings.every((draw) => draw.locale === "ja" && draw.font.startsWith("normal 600 ") &&
      draw.font.endsWith("system-ui, sans-serif") && draw.baseline === "middle" && draw.align === "left")).toBe(true);
    const titleInk = menuPaintedTextInk(viewport).find((ink) => ink.value === "Pause");
    const titleDraw = context.drawings.find((draw) => draw.value === "Pause");
    if (titleInk === undefined || titleDraw === undefined) throw new Error("Missing title ink");
    expect(titleDraw.x * panel.size.width / prepared.canvas.width).toBeCloseTo(titleInk.anchorX, 12);
    expect(titleDraw.y * panel.size.height / prepared.canvas.height).toBeCloseTo(titleInk.baselineY, 12);
    expect(titleInk.fullRect.x).toBeGreaterThan(titleInk.anchorX);
    expect(context.saves).toBe(context.restores);
  });

  it("uses the same clipped rectangle for a drawn enabled control and independent panel-point hit", () => {
    const context = new RecordingContext();
    const source = { ...panel, size: { width: 0.5, height: 0.45 } };
    const prepared = prepare(context, source);
    const viewport = menuViewport(prepared.document, 0);
    drawMeasuredMenu(context, viewport, prepared.canvas.width, prepared.canvas.height);
    const resume = prepared.document.controls.find((entry) => entry.control.id === "resume");
    if (resume === undefined) throw new Error("Missing Resume");
    const physicalLeft = resume.bounds.x + viewport.contentClip.x;
    const physicalTop = resume.bounds.y + viewport.contentClip.y;
    const point = { x: physicalLeft + resume.bounds.width / 2 - source.size.width / 2,
      y: source.size.height / 2 - physicalTop - resume.bounds.height / 2 };
    const hit = hitMenuViewport(viewport, point);
    expect(hit.kind).toBe("control");
    if (hit.kind !== "control") throw new Error("Missing painted control hit");
    expect(hit.layout.control.id).toBe("resume");
    expect(context.boxes.some((box) => Math.abs(box.x * source.size.width / prepared.canvas.width - physicalLeft) < 1e-12 &&
      Math.abs(box.width * source.size.width / prepared.canvas.width - resume.bounds.width) < 1e-12)).toBe(true);
    expect(context.clips).toBeGreaterThan(0);
  });

  it("checks port-reported actual ink in both eyes without substituting font height", () => {
    const context = new RecordingContext();
    const prepared = prepare(context);
    const viewer = configuredViewerFixture(720, 1280);
    const fit = fitMenuView(viewer, panel.size);
    if (fit.kind !== "ready") throw new Error("Missing fit");
    expect(validateMenuOpeningInk(prepared.document, viewer, fit.headFromMenu).kind).toBe("ready");
    context.read = (value, pixels) => ({ width: value.length * pixels * 0.6, left: 0, right: value.length * pixels * 0.6,
      ascent: value.trim() === "" ? 0 : pixels * 0.03, descent: 0 });
    const small = prepare(context);
    expect(validateMenuOpeningInk(small.document, viewer, fit.headFromMenu)).toEqual({ kind: "unavailable", reason: "insufficient-ink-angle" });
  });

  it("returns typed invalid/unavailable results and restores drawing state on metrics failure", () => {
    const request = requestMenuLayout(panel, panel.size, style);
    const context = new RecordingContext();
    context.read = () => ({ width: Number.NaN, left: 0, right: 0, ascent: 0, descent: 0 });
    expect(measureMenuLayout(context, request, 400, 1400)).toEqual({ kind: "unavailable", reason: "invalid-text-metrics" });
    context.read = () => { throw new Error("Canvas unavailable"); };
    expect(measureMenuLayout(context, request, 400, 1400)).toEqual({ kind: "unavailable", reason: "measurement-unavailable" });
    expect(context.saves).toBe(context.restores);
    expect(() => menuCanvasSize({ width: 0, height: 1 })).toThrow(RangeError);
  });

  it("restores the Canvas clip stack when the drawing API fails", () => {
    const context = new RecordingContext();
    const prepared = prepare(context);
    context.fillText = () => { throw new Error("drawing unavailable"); };
    expect(() => { drawMeasuredMenu(context, menuViewport(prepared.document, 0), prepared.canvas.width, prepared.canvas.height); }).toThrow("drawing unavailable");
    expect(context.saves).toBe(context.restores);
  });

  it("caches measured text by font and layout identity while retaining current semantic controls", () => {
    const context = new RecordingContext();
    const cache: MenuMeasurementCache = new Map();
    const scroll = { scope: { kind: "scene" as const, scene: "Title" as const, panelId: panel.id, viewKey: "main" }, generation: 1 };
    const viewer = configuredViewerFixture();
    const typography = { theme: style.theme, locale: "ja", caption: "", font: style.font };
    const first = prepareMenuPresentation(context, panel, scroll, 0, viewer, typography, cache);
    if (first.kind !== "ready") throw new Error("Missing first Menu");
    context.read = () => { throw new Error("Unexpected repeated text measurement"); };
    const replacement = { ...panel, controls: panel.controls.map((control) => ({ ...control })) };
    const theme = { ...style.theme, panel_background: "#213547", panel_opacity: 0.41 };
    const second = prepareMenuPresentation(context, replacement, { ...scroll, generation: 2 }, 0.5, viewer, { ...typography, theme }, cache);
    if (second.kind !== "ready") throw new Error("Missing cached Menu");
    for (const control of replacement.controls) expect(second.viewport.document.controls.find((layout) => layout.control.id === control.id)?.control).toBe(control);
    expect(second.context.generation).toBe(2);
    expect(second.viewport.progress).toBe(0.5);
    expect(second.viewport.document.request.theme).toBe(theme);
    const dimensions = menuCanvasSize(second.panel.size);
    drawMeasuredMenu(context, second.viewport, dimensions.width, dimensions.height);
    expect(context.boxes[0]).toMatchObject({ color: theme.panel_background, alpha: theme.panel_opacity });
    expect(prepareMenuPresentation(context, replacement, scroll, 0, viewer,
      { ...typography, font: { ...style.font, generation: 1 } }, cache)).toEqual({ kind: "unavailable", reason: "measurement-unavailable" });
  });
});
