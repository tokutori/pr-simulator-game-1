import { describe, expect, it } from "vitest";
import { IDENTITY_POSE, pose, vec3 } from "../../web/src/render/contracts/math.js";
import type { MenuDocument, MenuLayoutRequest, MenuTextMeasurement, MenuTextRequest } from "../../web/src/render/contracts/menu-layout.js";
import { MENU_MINIMUM_INK_HEIGHT_DEGREES } from "../../web/src/render/contracts/menu-layout.js";
import { normalizedRect } from "../../web/src/render/contracts/ui.js";
import type { UiChart, UiControl, UiPanel } from "../../web/src/render/contracts/ui.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import { hitMenuViewport, layoutMeasuredMenu, menuDocumentPointToPanel, menuPanelPointToDocument,
  menuRangeAction, menuRectInViewport, menuViewport, requestMenuLayout, validateMenuOpeningInk } from "../../web/src/presentation/menu-layout.js";
import { fitMenuView } from "../../web/src/presentation/menu-view-fit.js";
import { configuredViewerFixture } from "./viewer-fixture.js";

const font = Object.freeze({ family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 } as const);
const style = Object.freeze({ fontMeters: 0.04, font, locale: "ja", previousLabel: "前頁", nextLabel: "次頁", caption: "Software fixture" });
const controls: readonly UiControl[] = Object.freeze([
  { kind: "button", id: "resume", label: "飛行を再開する", enabled: true, rect: normalizedRect(0.04, 0.2, 0.44, 0.08) },
  { kind: "button", id: "settings", label: "設定", enabled: false, rect: normalizedRect(0.52, 0.2, 0.44, 0.08) },
  { kind: "status", id: "help", label: "操作方法", value: "Roll A/D · Pitch ↑/↓ · 重心 J/L\n入力と姿勢を確認する。", enabled: false, rect: normalizedRect(0.04, 0.32, 0.92, 0.22) },
  { kind: "range", id: "time", label: "Flight time (s)", value: 5, minimum: 0, maximum: 20, step: 0.5, enabled: true, rect: normalizedRect(0.04, 0.62, 0.92, 0.08) },
  { kind: "toggle", id: "toggle", label: "表示", value: true, enabled: true, rect: normalizedRect(0.04, 0.76, 0.92, 0.08) }
]);
const panel: UiPanel = Object.freeze({ id: "pause", title: "Pause", anchor: "menu", localPose: IDENTITY_POSE,
  size: Object.freeze({ width: 2.4, height: 1.8 }), controls });

function measurementFor(request: MenuTextRequest): MenuTextMeasurement {
  const characterWidth = request.fontMeters * 0.55;
  const columns = Math.max(1, Math.floor(request.widthMeters / characterWidth));
  const lines = request.value.split("\n").flatMap((paragraph, paragraphIndex) => {
    const characters = Array.from(paragraph);
    if (characters.length === 0) return [{ value: "", paragraphIndex }];
    const result: string[] = [];
    for (let offset = 0; offset < characters.length; offset += columns) result.push(characters.slice(offset, offset + columns).join(""));
    return result.map((value) => ({ value, paragraphIndex }));
  }).map(({ value, paragraphIndex }) => Object.freeze({ value, paragraphIndex, advanceMeters: Array.from(value).length * characterWidth,
    leftMeters: 0, rightMeters: Array.from(value).length * characterWidth,
    ascentMeters: value.trim() === "" ? 0 : request.fontMeters * 0.65,
    descentMeters: value.trim() === "" ? 0 : request.fontMeters * 0.15 }));
  return Object.freeze({ identity: request.identity, lines: Object.freeze(lines) });
}

function documentFor(request: MenuLayoutRequest): MenuDocument {
  const result = layoutMeasuredMenu(request, request.texts.map(measurementFor));
  if (result.kind !== "ready") throw new Error(result.reason);
  return result.document;
}

function chart(): UiChart {
  return Object.freeze({ kind: "chart", id: "chart", label: "Altitude", xAxisLabel: "Time (s)", yAxisLabel: "Height (m)",
    xMinimum: 0, xMaximum: 20, yMinimum: 0, yMaximum: 20, equalAxisScale: true, enabled: false,
    rect: normalizedRect(0.04, 0.12, 0.92, 0.5), series: [{ label: "Recorded altitude", color: "#ffffff", points: [{ x: 0, y: 10 }, { x: 20, y: 0 }] }],
    vectors: [{ label: "Wind", color: "#ffffff", start: { x: 2, y: 3 }, end: { x: 4, y: 3 } }],
    markers: [{ label: "Launch", color: "#ffffff", point: { x: 0, y: 10 } }],
    timeMarkers: [{ label: "5 seconds", color: "#ffffff", point: { x: 5, y: 7 } }],
    referenceLines: [{ value: 0, label: "Water", color: "#ffffff" }], cursorX: 5, cursorPoints: [{ x: 5, y: 7 }] });
}

describe("Pure measured Menu document and shared viewport", () => {
  it("preserves every semantic control and uses source rows in landscape and a single column in portrait", () => {
    const landscape = requestMenuLayout(panel, panel.size, style);
    const portrait = requestMenuLayout(panel, { width: 0.5, height: 1.8 }, style);
    expect(landscape.rows[0]?.controls.map((item) => item.control.id)).toEqual(["resume", "settings"]);
    expect(portrait.rows.every((row) => row.controls.length === 1)).toBe(true);
    for (const request of [landscape, portrait]) {
      const document = documentFor(request);
      expect(document.controls.map((entry) => entry.control)).toEqual(controls);
      expect(document.controls.find((entry) => entry.control.id === "settings")?.control.enabled).toBe(false);
      expect(document.controls.find((entry) => entry.control.id === "time")?.control).toBe(controls[3]);
      expect(Object.isFrozen(document)).toBe(true);
      expect(Object.isFrozen(document.controls)).toBe(true);
    }
  });

  it("retains long wrapped text and extends the document instead of dropping controls or reducing ink size", () => {
    const long = { ...panel, controls: [...controls, { kind: "status", id: "long", label: "記録", value: "内容を保持する。".repeat(150),
      enabled: false, rect: normalizedRect(0.04, 0.88, 0.92, 0.08) } satisfies UiControl] };
    const document = documentFor(requestMenuLayout(long, { width: 0.5, height: 1.8 }, style));
    expect(document.contentHeightMeters).toBeGreaterThan(document.request.surfaceSize.height * 2);
    const text = document.controls.find((entry) => entry.control.id === "long")?.texts.find((entry) => entry.request.slot.role === "value");
    expect(text?.lines.map((line) => line.value).join("")).toBe("内容を保持する。".repeat(150));
    expect(document.request.fontMeters).toBe(style.fontMeters);
  });

  it("distinguishes text/font/locale/surface/owner/role identities and rejects missing, duplicate and stale responses", () => {
    const request = requestMenuLayout(panel, panel.size, style);
    const measurements = request.texts.map(measurementFor);
    const first = measurements[0];
    if (first === undefined) throw new Error("Missing fixture");
    expect(layoutMeasuredMenu(request, measurements.slice(1))).toEqual({ kind: "unavailable", reason: "missing-text-metrics" });
    expect(layoutMeasuredMenu(request, [...measurements, first])).toEqual({ kind: "unavailable", reason: "stale-text-metrics" });
    for (const alternative of [requestMenuLayout(panel, panel.size, { ...style, locale: "en" }),
      requestMenuLayout(panel, panel.size, { ...style, fontMeters: 0.05 }), requestMenuLayout(panel, { width: 1.8, height: 1.8 }, style),
      requestMenuLayout(panel, panel.size, { ...style, font: { ...font, family: "monospace" } }),
      requestMenuLayout(panel, panel.size, { ...style, font: { ...font, style: "italic" } }),
      requestMenuLayout(panel, panel.size, { ...style, font: { ...font, weight: 700 } }),
      requestMenuLayout(panel, panel.size, { ...style, font: { ...font, generation: 1 } }),
      requestMenuLayout({ ...panel, title: "変更" }, panel.size, style)]) {
      expect(layoutMeasuredMenu(alternative, measurements)).toEqual({ kind: "unavailable", reason: "stale-text-metrics" });
    }
    expect(new Set(request.texts.map((text) => text.identity)).size).toBe(request.texts.length);
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, -1])("rejects invalid metrics %s as a typed result", (invalid) => {
    const request = requestMenuLayout(panel, panel.size, style);
    const measurements = request.texts.map(measurementFor);
    const first = measurements[0];
    const line = first?.lines[0];
    if (first === undefined || line === undefined) throw new Error("Missing fixture");
    expect(layoutMeasuredMenu(request, [{ ...first, lines: [{ ...line, advanceMeters: invalid }] }, ...measurements.slice(1)]))
      .toEqual({ kind: "unavailable", reason: "invalid-text-metrics" });
  });

  it("rejects omitted characters and too-tall ink while retaining signed ink bearings separately from advance", () => {
    const request = requestMenuLayout(panel, panel.size, style);
    const measurements = request.texts.map(measurementFor);
    const first = measurements[0];
    const line = first?.lines[0];
    if (first === undefined || line === undefined) throw new Error("Missing fixture");
    for (const changed of [{ ...line, value: "wrong" }, { ...line, ascentMeters: request.lineHeightMeters * 2 }]) {
      expect(layoutMeasuredMenu(request, [{ ...first, lines: [changed] }, ...measurements.slice(1)]))
        .toEqual({ kind: "unavailable", reason: "invalid-text-metrics" });
    }
    const bearing = { ...line, leftMeters: -0.005, rightMeters: line.advanceMeters + 0.005 };
    const result = layoutMeasuredMenu(request, [{ ...first, lines: [bearing] }, ...measurements.slice(1)]);
    expect(result.kind).toBe("ready");
    if (result.kind !== "ready") throw new Error("Missing result");
    expect(result.document.title.lines[0]).toEqual(bearing);
  });

  it("maps the same full-document point to painting and hit testing at every scroll position", () => {
    const document = documentFor(requestMenuLayout(panel, { width: 0.5, height: 0.65 }, style));
    for (const progress of [0, 0.5, 1]) {
      const viewport = menuViewport(document, progress);
      const center = { x: viewport.contentClip.width / 2, y: viewport.offsetMeters + viewport.contentClip.height / 2 };
      const point = menuDocumentPointToPanel(viewport, center);
      const restored = menuPanelPointToDocument(viewport, point);
      if (restored.kind !== "inside") throw new Error("Missing content point");
      expect(restored.point.x).toBeCloseTo(center.x, 12);
      expect(restored.point.y).toBeCloseTo(center.y, 12);
      expect(viewport.offsetMeters).toBe(progress * viewport.maximumOffsetMeters);
      for (const layout of document.controls) {
        const visible = menuRectInViewport(viewport, layout.bounds);
        if (visible.kind === "outside" || !layout.control.enabled) continue;
        const panelPoint = { x: visible.rect.x + visible.rect.width / 2 - document.request.surfaceSize.width / 2,
          y: document.request.surfaceSize.height / 2 - visible.rect.y - visible.rect.height / 2 };
        const hit = hitMenuViewport(viewport, panelPoint);
        expect(hit.kind).toBe("control");
        if (hit.kind !== "control") throw new Error("Missing visible hit");
        expect(hit.layout.control.id).toBe(layout.control.id);
      }
    }
  });

  it("keeps footer hit testing exclusive, stops paging at the limits and rejects points outside the shared content clip", () => {
    const document = documentFor(requestMenuLayout(panel, { width: 0.5, height: 0.65 }, style));
    const first = menuViewport(document, 0);
    const last = menuViewport(document, 1);
    const panelPoint = (bounds: typeof first.previousBounds) => ({ x: bounds.x + bounds.width / 2 - 0.25, y: 0.325 - bounds.y - bounds.height / 2 });
    expect(hitMenuViewport(first, panelPoint(first.previousBounds))).toEqual({ kind: "none" });
    expect(hitMenuViewport(first, panelPoint(first.nextBounds))).toEqual({ kind: "page", direction: "next" });
    expect(hitMenuViewport(last, panelPoint(last.previousBounds))).toEqual({ kind: "page", direction: "previous" });
    expect(hitMenuViewport(last, panelPoint(last.nextBounds))).toEqual({ kind: "none" });
    expect(menuPanelPointToDocument(first, panelPoint(first.nextBounds))).toEqual({ kind: "outside" });
    expect(hitMenuViewport(first, { x: 0.251, y: 0 })).toEqual({ kind: "none" });
    expect(hitMenuViewport(first, { x: Number.NaN, y: 0 })).toEqual({ kind: "none" });
  });

  it("maps a partly clipped range by its full document width instead of the visible rectangle", () => {
    const extended = { ...panel, controls: [...controls, { kind: "status", id: "tail", label: "Record",
      value: "FlightRecord metadata ".repeat(50), enabled: false, rect: normalizedRect(0.04, 0.9, 0.92, 0.08) } satisfies UiControl] };
    const document = documentFor(requestMenuLayout(extended, { width: 0.5, height: 0.65 }, style));
    const range = document.controls.find((entry) => entry.control.kind === "range");
    if (range === undefined) throw new Error("Missing range");
    const initial = menuViewport(document, 0);
    const offset = range.bounds.y + range.bounds.height / 2;
    const viewport = menuViewport(document, offset / initial.maximumOffsetMeters);
    const point = menuDocumentPointToPanel(viewport, { x: range.bounds.x + range.bounds.width * 0.75, y: offset + range.bounds.height * 0.1 });
    const clipped = menuRectInViewport(viewport, range.bounds);
    if (clipped.kind !== "visible") throw new Error("Missing clipped range");
    expect(clipped.rect.height).toBeLessThan(range.bounds.height);
    expect(menuRangeAction(viewport, point)).toEqual({ kind: "action", controlId: "time", value: 15 });
    expect(menuRangeAction(viewport, { x: 0.3, y: 0 })).toEqual({ kind: "absent" });
  });

  it("measures every chart label, axis, tick, legend and annotation and keeps Rust query arrays and square scaling", () => {
    const original = chart();
    const request = requestMenuLayout({ ...panel, controls: [original] }, panel.size, style);
    const texts = request.texts.filter((text) => text.slot.kind === "control" && text.slot.controlId === "chart");
    expect(texts.filter((text) => text.slot.role === "x-tick")).toHaveLength(5);
    expect(texts.filter((text) => text.slot.role === "y-tick")).toHaveLength(5);
    expect(texts.map((text) => text.value)).toEqual(expect.arrayContaining(["Altitude", "Time (s)", "Height (m)", "Recorded altitude", "Wind", "Launch", "5 seconds", "Water"]));
    const document = documentFor(request);
    const layout = document.controls[0];
    if (layout?.kind !== "chart") throw new Error("Missing chart");
    expect(layout.control).toBe(original);
    expect(layout.control.series).toBe(original.series);
    expect(layout.control.cursorPoints).toBe(original.cursorPoints);
    expect(layout.plot.width).toBe(layout.plot.height);
    for (const text of layout.texts) {
      expect(text.bounds.x).toBeGreaterThanOrEqual(layout.bounds.x);
      expect(text.bounds.x + text.bounds.width).toBeLessThanOrEqual(layout.bounds.x + layout.bounds.width);
      expect(text.bounds.y).toBeGreaterThanOrEqual(layout.bounds.y);
      expect(text.bounds.y + text.bounds.height).toBeLessThanOrEqual(layout.bounds.y + layout.bounds.height);
    }
  });

  it("includes actual ink at panel edges and both eyes rather than using nominal central font height", () => {
    const request = requestMenuLayout(panel, panel.size, { ...style, fontMeters: 0.021 });
    const document = documentFor(request);
    const viewer = configuredViewerFixture();
    const fit = fitMenuView(viewer, panel.size);
    if (fit.kind !== "ready") throw new Error("Missing fit");
    const nominalCenter = 2 * Math.atan(request.fontMeters * 0.8 / (2 * fit.distanceMeters)) * 180 / Math.PI;
    expect(nominalCenter).toBeGreaterThan(MENU_MINIMUM_INK_HEIGHT_DEGREES);
    expect(validateMenuOpeningInk(document, viewer, fit.headFromMenu)).toEqual({ kind: "unavailable", reason: "insufficient-ink-angle" });
  });

  it.each([[1280, 720, 2.4], [720, 1280, 0.5]])("accepts a readable opening at %i × %i with reflow width %s", (width, height, surfaceWidth) => {
    const request = requestMenuLayout(panel, { width: surfaceWidth, height: 1.8 }, style);
    const viewer = configuredViewerFixture(width, height);
    const fit = fitMenuView(viewer, request.surfaceSize);
    if (fit.kind !== "ready") throw new Error("Missing fit");
    const result = validateMenuOpeningInk(documentFor(request), viewer, fit.headFromMenu);
    expect(result.kind).toBe("ready");
    if (result.kind !== "ready") throw new Error("Unreadable fixture");
    expect(result.minimumInkAngleDegrees).toBeGreaterThanOrEqual(0.35);
  });

  it("keeps missing/invalid viewer and uncontained opening distinct from insufficient ink", () => {
    const document = documentFor(requestMenuLayout(panel, panel.size, style));
    expect(validateMenuOpeningInk(document, unavailableViewerFrame("viewer-unavailable"), IDENTITY_POSE))
      .toEqual({ kind: "unavailable", reason: "viewer-unavailable" });
    expect(validateMenuOpeningInk(document, configuredViewerFixture(), pose(vec3(0, 0, -0.5), IDENTITY_POSE.orientation)))
      .toEqual({ kind: "unavailable", reason: "insufficient-view-area" });
  });

  it("validates public layout dimensions and normalized scroll values", () => {
    for (const dimensions of [{ width: 0, height: 1 }, { width: Number.POSITIVE_INFINITY, height: 1 }, { width: 0.01, height: 1 }]) {
      expect(() => requestMenuLayout(panel, dimensions, style)).toThrow(RangeError);
    }
    expect(() => requestMenuLayout(panel, panel.size, { ...style, locale: " " })).toThrow(RangeError);
    const document = documentFor(requestMenuLayout(panel, panel.size, style));
    for (const progress of [-0.1, 1.1, Number.NaN]) expect(() => menuViewport(document, progress)).toThrow(RangeError);
    expect(() => menuDocumentPointToPanel(menuViewport(document, 0), { x: Number.POSITIVE_INFINITY, y: 0 })).toThrow(RangeError);
  });

  it("retains explicit paragraph order and empty paragraphs and rejects merged or omitted lines", () => {
    const source = { ...panel, title: "A\n\nB\n" };
    const request = requestMenuLayout(source, source.size, style);
    const measurements = request.texts.map(measurementFor);
    const title = measurements[0];
    if (title === undefined) throw new Error("Missing title");
    expect(title.lines.map((line) => [line.paragraphIndex, line.value])).toEqual([[0, "A"], [1, ""], [2, "B"], [3, ""]]);
    expect(layoutMeasuredMenu(request, measurements).kind).toBe("ready");
    const first = title.lines[0];
    if (first === undefined) throw new Error("Missing first paragraph");
    const merged = { ...first, value: "AB", advanceMeters: style.fontMeters * 1.1, rightMeters: style.fontMeters * 1.1 };
    for (const changed of [{ ...title, lines: [merged] }, { ...title, lines: title.lines.filter((line) => line.value !== "") },
      { ...title, lines: [...title.lines].reverse() }]) {
      expect(layoutMeasuredMenu(request, [changed, ...measurements.slice(1)])).toEqual({ kind: "unavailable", reason: "invalid-text-metrics" });
    }
  });
});
