import { describe, expect, it } from "vitest";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import type { UiPanel } from "../../web/src/render/contracts/ui.js";
import { normalizedRect } from "../../web/src/render/contracts/ui.js";
import { drawMeasuredMenu, measureMenuLayout, menuCanvasSize, menuPaintedTextInk } from "../../web/src/presentation/menu-canvas.js";
import type { HeadHudDrawingContext, HeadHudTextMetrics } from "../../web/src/presentation/head-hud-canvas.js";
import { hitMenuViewport, layoutMeasuredMenu, menuViewport, requestMenuLayout, validateMenuOpeningInk } from "../../web/src/presentation/menu-layout.js";
import { fitMenuView } from "../../web/src/presentation/menu-view-fit.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { createAllSceneFixtures, createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { prepareMenuPresentation } from "../../web/src/presentation/menu-preparation.js";
import type { MenuMeasurementCache } from "../../web/src/presentation/menu-preparation.js";
import { menuActionAt, menuContextCanInteract, menuInputGeometry, menuInputGeometryChanged, menuPageProgress } from "../../web/src/presentation/menu-interaction.js";
import { menuDocumentPointToPanel, menuRectInViewport } from "../../web/src/presentation/menu-layout.js";

class RecordingContext implements HeadHudDrawingContext {
  font = "";
  locale = "";
  baseline = "";
  align = "";
  saves = 0;
  restores = 0;
  clips = 0;
  readonly drawings: { value: string; x: number; y: number; font: string; locale: string; baseline: string; align: string }[] = [];
  readonly boxes: { x: number; y: number; width: number; height: number }[] = [];
  readonly paths: { x: number; y: number }[] = [];
  read: (value: string, pixels: number) => HeadHudTextMetrics = (value, pixels) => {
    const width = Array.from(value).length * pixels * 0.6;
    const visible = value.trim() !== "";
    return { width, left: visible ? -pixels * 0.1 : 0, right: visible ? width + pixels * 0.1 : width,
      ascent: visible ? pixels * 0.6 : 0, descent: visible ? pixels * 0.2 : 0 };
  };
  clearRect(): void {}
  fillRect(coordinateX: number, coordinateY: number, width: number, height: number): void { this.boxes.push({ x: coordinateX, y: coordinateY, width, height }); }
  fillText(value: string, coordinateX: number, coordinateY: number): void { this.drawings.push({ value, x: coordinateX, y: coordinateY,
    font: this.font, locale: this.locale, baseline: this.baseline, align: this.align }); }
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
  setFillStyle(): void {}
  setStrokeStyle(): void {}
  setFont(value: string): void { this.font = value; }
  setTextBaseline(value: "middle"): void { this.baseline = value; }
  setLineWidth(): void {}
  setGlobalAlpha(): void {}
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
const style = { fontMeters: 0.04, font: { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 } as const,
  locale: "ja", previousLabel: "前頁", nextLabel: "次頁", caption: "Build" };

function prepare(context: HeadHudDrawingContext, source = panel) {
  const request = requestMenuLayout(source, source.size, style);
  const canvas = menuCanvasSize(source.size);
  const measurement = measureMenuLayout(context, request, canvas.width, canvas.height);
  if (measurement.kind !== "ready") throw new Error(measurement.reason);
  const result = layoutMeasuredMenu(request, measurement.measurements);
  if (result.kind !== "ready") throw new Error(result.reason);
  return { request, canvas, measurement, document: result.document };
}

describe("Canvas measurement boundary and shared Menu painting", () => {
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
          { locale: "ja", caption: view.description, font: style.font }, new Map());
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
      { kind: "range", id: "trim", label: "Trim", value: 0, minimum: -1, maximum: 1, step: 0.1, enabled: true, rect: normalizedRect(0.04, 0.7, 0.92, 0.08) }] };
    const ready = prepareMenuPresentation(context, rangePanel,
      { scope: { kind: "overlay", scene: "Flight", overlay: "Pause", panelId: panel.id, viewKey: "main" }, generation: 7 }, 0,
      configuredViewerFixture(720, 1280), { locale: "ja", caption: "Long note ".repeat(80), font: style.font }, new Map());
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
      configuredViewerFixture(720, 1280), { locale: "ja", caption: "Flight status", font: style.font }, new Map());
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
    const first = prepareMenuPresentation(context, panel, scroll, 0, viewer, { locale: "ja", caption: "", font: style.font }, new Map());
    if (first.kind !== "ready") throw new Error("Missing layout A");
    const resume = first.viewport.document.controls.find((layout) => layout.control.id === "resume");
    if (resume === undefined) throw new Error("Missing Resume");
    const oldAction = menuActionAt(first, menuDocumentPointToPanel(first.viewport,
      { x: resume.bounds.x + resume.bounds.width / 2, y: resume.bounds.y + resume.bounds.height / 2 }));
    if (oldAction.kind !== "action" || oldAction.action.type !== "menu-control") throw new Error("Missing old action");
    const second = prepareMenuPresentation(context, panel, scroll, 0, viewer,
      { locale: "ja", caption: "Long paragraph ".repeat(100), font: style.font }, new Map());
    if (second.kind !== "ready") throw new Error("Missing layout B");
    expect(menuInputGeometryChanged(menuInputGeometry(first), second)).toBe(true);
    const oldState = { kind: "active" as const, ...scroll, progress: 0 };
    expect(menuContextCanInteract({ kind: "pending" }, oldState, oldAction.action.context)).toBe(false);
    const currentState = { ...oldState, generation: 8 };
    const currentMenu = { ...second, context: { ...scroll, generation: 8 } };
    expect(menuContextCanInteract(currentMenu, currentState, oldAction.action.context)).toBe(false);
    expect(menuContextCanInteract(currentMenu, currentState, currentMenu.context)).toBe(true);
    const sameGeometry = prepareMenuPresentation(context, { ...panel, controls: panel.controls.map((control) => control.kind === "status"
      ? { ...control, value: "C\n\nD\n" } : control) }, scroll, 0, viewer, { locale: "ja", caption: "", font: style.font }, new Map());
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
    const typography = { locale: "ja", caption: "", font: style.font };
    const first = prepareMenuPresentation(context, panel, scroll, 0, viewer, typography, cache);
    if (first.kind !== "ready") throw new Error("Missing first Menu");
    context.read = () => { throw new Error("Unexpected repeated text measurement"); };
    const replacement = { ...panel, controls: panel.controls.map((control) => ({ ...control })) };
    const second = prepareMenuPresentation(context, replacement, { ...scroll, generation: 2 }, 0.5, viewer, typography, cache);
    if (second.kind !== "ready") throw new Error("Missing cached Menu");
    for (const control of replacement.controls) expect(second.viewport.document.controls.find((layout) => layout.control.id === control.id)?.control).toBe(control);
    expect(second.context.generation).toBe(2);
    expect(second.viewport.progress).toBe(0.5);
    expect(prepareMenuPresentation(context, replacement, scroll, 0, viewer,
      { ...typography, font: { ...style.font, generation: 1 } }, cache)).toEqual({ kind: "unavailable", reason: "measurement-unavailable" });
  });
});
