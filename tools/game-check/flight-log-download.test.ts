import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserFlightLogDownload, sanitizeFlightLogFilename } from "../../web/src/app/browser-flight-log-download.js";
import { createInitialAppModel, gameSessionState, isCurrentFlightLogDownload, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { actionForControl } from "../../web/src/presentation/panel-interaction.js";
import { readFlightLog } from "../../web/src/game/flight-log-export.js";
import { readFileSync } from "node:fs";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function readyModel(phaseCode: number): AppModel {
  const initial = createInitialAppModel();
  const gameSession = gameSessionState(phaseCode, 0, null, false, null, "tail_incidence", phaseCode === 9 ? "result" : undefined);
  if (gameSession === null) throw new Error("Invalid download fixture phase");
  return { ...initial, presentation: { type: "ready", mode: "screen" }, gameSession };
}

function beginDownload(model: AppModel, format: "csv" | "json" = "csv") {
  const requested = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: `game-flight-log-${format}` } });
  const effect = requested.effects[0];
  if (effect?.type !== "download-flight-log") throw new Error("Missing download effect");
  return { ...requested, effect };
}

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

const analysisFixture = (() => {
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  try {
    launchCurrentSession(session);
    session.flightPort.advance_tick_json(neutralTailInput);
    session.executeOperation("abort");
    return session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
  } finally { session.dispose(); }
})();

describe("flight log download state and common presentation", () => {
  it.each([
    { label: "Result Summary", phaseCode: 7, resultTab: "summary", replayViewMode: "cinematic" },
    { label: "Result Analysis", phaseCode: 7, resultTab: "analysis", replayViewMode: "cinematic" },
    { label: "Replay Analysis", phaseCode: 9, resultTab: "summary", replayViewMode: "analysis" },
    { label: "Replay Telemetry", phaseCode: 9, resultTab: "summary", replayViewMode: "telemetry" },
    { label: "Replay Cinematic", phaseCode: 9, resultTab: "summary", replayViewMode: "cinematic" }
  ] as const)("reserves a nonintersecting download footer and preserves navigation in $label", ({ phaseCode, resultTab, replayViewMode }) => {
    for (const mode of ["screen", "phone-vr", "webxr"] as const) {
      for (const analysisChart of ["map", "altitude", "speed"] as const) {
        const model: AppModel = {
          ...readyModel(phaseCode), presentation: { type: "ready", mode },
          resultTab, replayViewMode, analysisChart, flightAnalysis: analysisFixture,
          analysisCursorSample: analysisFixture.samples[1] === undefined ? null
            : { ...analysisFixture.samples[1], kind: "named_record", context: analysisFixture.context }
        };
        const panel = createGameViewModel(model, null).panels[0];
        if (panel === undefined) throw new Error("Download panel is unavailable");
        expect(panel.anchor).toBe("menu");
        const exports = panel.controls.filter((control) => control.id.startsWith("game-flight-log-"));
        expect(exports.map((control) => control.id)).toEqual(["game-flight-log-csv", "game-flight-log-json", "game-flight-log-notice"]);
        for (const control of panel.controls) {
          expect(control.rect.x).toBeGreaterThanOrEqual(0);
          expect(control.rect.y).toBeGreaterThanOrEqual(0);
          expect(control.rect.width).toBeGreaterThan(0);
          expect(control.rect.height).toBeGreaterThan(0);
          expect(control.rect.x + control.rect.width).toBeLessThanOrEqual(1);
          expect(control.rect.y + control.rect.height).toBeLessThanOrEqual(1);
        }
        for (const exported of exports) {
          for (const control of panel.controls) {
            if (control.id === exported.id) continue;
            const horizontal = Math.min(exported.rect.x + exported.rect.width, control.rect.x + control.rect.width)
              - Math.max(exported.rect.x, control.rect.x);
            const vertical = Math.min(exported.rect.y + exported.rect.height, control.rect.y + control.rect.height)
              - Math.max(exported.rect.y, control.rect.y);
            expect(horizontal > 1e-12 && vertical > 1e-12, `${exported.id} overlaps ${control.id}`).toBe(false);
          }
        }
        const navigation = phaseCode === 7
          ? ["game-result-replay", "game-result-retry", "game-result-setup", "game-result-title", resultTab === "summary" ? "game-result-open-analysis" : "game-result-open-summary"]
          : ["game-replay-return", "game-replay-camera", "game-replay-view-mode", "game-replay-play-pause", ...(replayViewMode === "cinematic" ? [] : ["game-replay-cursor"])];
        for (const id of navigation) expect(panel.controls.some((control) => control.id === id)).toBe(true);
      }
    }
  });

  it.each([7, 9])("exposes explicit CSV/JSON choices in phase %s for Screen and VR", async (phaseCode) => {
    const browser = new BrowserWindow();
    vi.stubGlobal("window", browser);
    vi.stubGlobal("document", browser.document);
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = browser.document.createElement("div");
    const dispatch = vi.fn();
    const screen = new ScreenUiAdapter(root as unknown as HTMLElement, dispatch);
    for (const mode of ["screen", "phone-vr", "webxr"] as const) {
      const model: AppModel = { ...readyModel(phaseCode), presentation: { type: "ready", mode } };
      const view = createGameViewModel(model, null);
      const panel = view.panels[0];
      expect(panel?.anchor).toBe("menu");
      const controls = panel?.controls.filter((control) => control.id === "game-flight-log-csv" || control.id === "game-flight-log-json");
      expect(controls).toHaveLength(2);
      expect(controls?.map(actionForControl)).toEqual([
        { type: "activate", controlId: "game-flight-log-csv" }, { type: "activate", controlId: "game-flight-log-json" }
      ]);
      screen.render(view);
      const button = root.querySelector('[data-control-id="game-flight-log-csv"]');
      if (!(button instanceof browser.HTMLButtonElement)) throw new Error("Required download button is unavailable");
      button.click();
      expect(dispatch).toHaveBeenLastCalledWith({ type: "activate", controlId: "game-flight-log-csv" });
      expect(root.textContent).toContain("finite-difference推定値");
    }
  });

  it.each([0, 1, 2, 3, 4, 8, 10])("rejects export commands outside valid record phases: %s", (phaseCode) => {
    const model = readyModel(phaseCode);
    expect(updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-log-json" } }).effects).toEqual([]);
    expect(createGameViewModel(model, null).panels.flatMap((panel) => panel.controls).some((control) => control.id === "game-flight-log-json")).toBe(false);
  });

  it("retains typed failure and clears it on a successful request without claiming saved completion", () => {
    const pending = beginDownload(readyModel(7));
    expect(updateApp(pending.model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-log-json" } }).effects).toEqual([]);
    const failed = updateApp(pending.model, { type: "flight-log-download-failed", requestId: pending.effect.requestId, source: pending.effect.source, message: "Injected export failure" });
    expect(failed.model.flightLogDownload).toEqual({ kind: "failed", message: "飛行ログのダウンロード要求に失敗した: Injected export failure" });
    const retried = beginDownload(failed.model, "json");
    const completed = updateApp(retried.model, { type: "flight-log-download-requested", requestId: retried.effect.requestId, source: retried.effect.source });
    expect(completed.model.flightLogDownload.kind).toBe("requested");
    const notice = createGameViewModel(completed.model, null).panels[0]?.controls.find((control) => control.id === "game-flight-log-notice");
    if (notice?.kind !== "status") throw new Error("Download notification is unavailable");
    expect(notice.value).toContain("ダウンロードを要求した");
  });

  it("rejects prior request and same-phase source-operation completions", () => {
    const pending = beginDownload(readyModel(7));
    const changed = updateApp(pending.model, { type: "ui-action", action: { type: "activate", controlId: "game-result-retry" } });
    expect(changed.model.flightRecordSourceRevision).toBeGreaterThan(pending.effect.source.revision);
    expect(isCurrentFlightLogDownload(changed.model, pending.effect.requestId, pending.effect.source)).toBe(false);
    expect(updateApp(changed.model, { type: "flight-log-download-requested", requestId: pending.effect.requestId, source: pending.effect.source }).model).toBe(changed.model);
    expect(updateApp(pending.model, { type: "flight-log-download-failed", requestId: pending.effect.requestId + 1, source: pending.effect.source, message: "old request" }).model).toBe(pending.model);
  });

  it("invalidates pending download on page teardown and disables buttons during source operations", () => {
    const pending = beginDownload(readyModel(9));
    const hidden = updateApp(pending.model, { type: "page-hidden" });
    expect(isCurrentFlightLogDownload(hidden.model, pending.effect.requestId, pending.effect.source)).toBe(false);
    const busy = { ...readyModel(7), pendingGameRequestId: 4 };
    expect(createGameViewModel(busy, null).panels[0]?.controls.find((control) => control.id === "game-flight-log-csv")).toMatchObject({ enabled: false });
  });
});

describe("browser download side-effect boundary", () => {
  it("passes original query text without numeric or JSON reconstruction", () => {
    const port = { export_flight_log_csv: vi.fn(() => "raw,csv\r\n1,2"), export_current_flight_record_json: vi.fn(() => '{ "original": true }') };
    expect(readFlightLog(port, "csv")).toBe("raw,csv\r\n1,2");
    expect(readFlightLog(port, "json")).toBe('{ "original": true }');
    expect(port.export_flight_log_csv).toHaveBeenCalledTimes(1);
    expect(port.export_current_flight_record_json).toHaveBeenCalledTimes(1);
  });

  it("retains successful UTF-8 Blob URLs through later requests until teardown and removes download anchors", async () => {
    const browser = new BrowserWindow();
    const blobs: Blob[] = [];
    const urls = { createObjectURL: vi.fn((blob: Blob) => { blobs.push(blob); return `blob:fixture-${String(blobs.length)}`; }), revokeObjectURL: vi.fn() };
    const clicked: { filename: string; href: string }[] = [];
    const click = vi.spyOn(browser.HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) { clicked.push({ filename: this.download, href: this.href }); });
    const callbacks: (() => void)[] = [];
    const cancel = vi.fn();
    const scheduler = { schedule: vi.fn((callback: () => void, delayMilliseconds: number) => {
      expect(delayMilliseconds).toBe(60_000);
      callbacks.push(callback);
      return cancel;
    }) };
    const adapter = new BrowserFlightLogDownload(browser.document as unknown as Document, urls, scheduler);
    adapter.download({ text: "高度,速度\n10,9.7", format: "csv", filename: "../unsafe:flight.csv" });
    expect(blobs[0]?.type).toBe("text/csv;charset=utf-8");
    expect(await blobs[0]?.text()).toBe("高度,速度\n10,9.7");
    expect(clicked[0]).toEqual({ filename: "---unsafe-flight.csv", href: "blob:fixture-1" });
    expect(urls.revokeObjectURL).not.toHaveBeenCalled();
    expect(browser.document.querySelector("a")).toBeNull();
    adapter.download({ text: '{ "record": "原文" }', format: "json", filename: "flight.json" });
    expect(await blobs[1]?.text()).toBe('{ "record": "原文" }');
    expect(urls.revokeObjectURL).not.toHaveBeenCalled();
    adapter.dispose();
    adapter.dispose();
    for (const callback of callbacks) callback();
    expect(urls.revokeObjectURL.mock.calls).toEqual([["blob:fixture-1"], ["blob:fixture-2"]]);
    expect(cancel).toHaveBeenCalledTimes(2);
    expect(() => { adapter.download({ text: "", format: "csv", filename: "flight.csv" }); }).toThrow("disposed");
    click.mockRestore();
  });

  it("releases a failed request URL and preserves the original browser exception", () => {
    const browser = new BrowserWindow();
    const failure = new Error("Injected anchor activation failure");
    const click = vi.spyOn(browser.HTMLAnchorElement.prototype, "click").mockImplementation(() => { throw failure; });
    const urls = { createObjectURL: vi.fn(() => "blob:failed"), revokeObjectURL: vi.fn() };
    const adapter = new BrowserFlightLogDownload(browser.document as unknown as Document, urls);
    expect(() => { adapter.download({ text: "csv", format: "csv", filename: "flight.csv" }); }).toThrow(failure);
    expect(urls.revokeObjectURL).toHaveBeenCalledExactlyOnceWith("blob:failed");
    expect(browser.document.querySelector("a")).toBeNull();
    adapter.dispose();
    expect(urls.revokeObjectURL).toHaveBeenCalledTimes(1);
    click.mockRestore();
  });

  it("restricts the filename extension and removes path and control characters", () => {
    expect(sanitizeFlightLogFilename(".json", "csv")).toBe("flight-log.csv");
    expect(sanitizeFlightLogFilename("a\n/b\\c:<>.exe", "json")).toBe("a--b-c---.json");
  });

  it("bounds outstanding URLs before Blob creation and expires each URL once after the grace period", () => {
    const browser = new BrowserWindow();
    const click = vi.spyOn(browser.HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const callbacks: (() => void)[] = [];
    const cancel = vi.fn();
    const scheduler = { schedule: vi.fn((callback: () => void, delayMilliseconds: number) => {
      expect(delayMilliseconds).toBe(60_000);
      callbacks.push(callback);
      return cancel;
    }) };
    let count = 0;
    const urls = { createObjectURL: vi.fn(() => `blob:bounded-${String(++count)}`), revokeObjectURL: vi.fn() };
    const adapter = new BrowserFlightLogDownload(browser.document as unknown as Document, urls, scheduler);
    const log = { text: "log", format: "csv", filename: "flight.csv" } as const;
    for (let index = 0; index < 8; index += 1) adapter.download(log);
    expect(() => { adapter.download(log); }).toThrow("上限8件");
    expect(urls.createObjectURL).toHaveBeenCalledTimes(8);
    expect(urls.revokeObjectURL).not.toHaveBeenCalled();
    callbacks[0]?.();
    callbacks[0]?.();
    expect(urls.revokeObjectURL).toHaveBeenCalledExactlyOnceWith("blob:bounded-1");
    adapter.download(log);
    expect(urls.createObjectURL).toHaveBeenCalledTimes(9);
    adapter.dispose();
    for (const callback of callbacks) callback();
    expect(urls.revokeObjectURL).toHaveBeenCalledTimes(9);
    expect(cancel).toHaveBeenCalledTimes(8);
    click.mockRestore();
  });
});
