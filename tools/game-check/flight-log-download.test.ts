import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserFlightLogDownload, sanitizeFlightLogFilename } from "../../web/src/app/browser-flight-log-download.js";
import { createInitialAppModel, flightLogDownloadNotice, updateApp } from "../../web/src/app/app-state.js";
import type { SessionPhase } from "../../web/src/app/session-snapshot.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { readFileSync } from "node:fs";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { withSharedGameUiSession } from "./shared-game-ui-fixture.js";
import type { SharedGameUiFixtureOptions } from "./shared-game-ui-fixture.js";
import type { GameUiAction } from "../../web/src/generated/game-ui-action.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function readyModel(phase: SessionPhase, options: SharedGameUiFixtureOptions = {}): AppModel {
  if (phase === "result") options = { ...options, actions: [{ kind: "toggle_technical" }, ...options.actions ?? []] };
  return withSharedGameUiSession(phase, options, (session, platform) => {
    const observed = updateApp({ ...createInitialAppModel(), presentation: { type: "ready", mode: "screen" } as const },
      { type: "game-session-synced", ...readRuntimeSessionProjection(session) }).model;
    return updateApp(observed, { type: "shared-game-ui-observed", view: session.readSharedUi(platform) }).model;
  });
}

function viewForModel(model: AppModel, viewport?: GameUiPlatformInput["viewport"]) {
  const phase = model.gameSession.phase;
  if (phase === "boot") throw new Error("Record controls require a Rust session");
  const currentMode = model.presentation.type === "ready" ? model.presentation.mode : "screen";
  const actions: GameUiAction[] = phase === "result" ? [{ kind: "toggle_technical" }] : [];
  if (phase === "result" && model.resultTab === "analysis") actions.push({ kind: "set_result_tab", tab: "analysis" });
  if (phase === "replay") actions.push({ kind: "set_replay_view", view: model.replayViewMode });
  if ((phase === "result" && model.resultTab === "analysis" || phase === "replay" && model.replayViewMode === "analysis") && model.analysisChart !== "map") {
    actions.push({ kind: "set_analysis_chart", chart: model.analysisChart });
  }
  const shared = withSharedGameUiSession(phase, { actions, platform: {
    ...(viewport === undefined ? {} : { viewport }),
    presentation_mode: currentMode === "phone-vr" ? "phone_vr" : currentMode,
    notification: flightLogDownloadNotice(model.flightLogDownload)
  } }, (session, platform) => session.readSharedUi({ ...platform,
    operation_pending: model.recordOpen.kind === "loading"
  }));
  return createGameViewModel(shared);
}

function beginDownload(model: AppModel, format: "csv" | "json" = "csv") {
  const admission = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: `game-flight-log-${format}` } });
  const request = admission.effects[0];
  if (request?.type !== "update-shared-game-ui") throw new Error("Missing widget request");
  const phase = model.gameSession.phase;
  if (phase === "boot") throw new Error("Missing session");
  const effectFromRust = withSharedGameUiSession(phase, { actions: phase === "result" ? [{ kind: "toggle_technical" }] : [] },
    (session, platform) => session.updateSharedUi(request.action, platform).effects[0]);
  if (effectFromRust === undefined) throw new Error("Missing Rust export effect");
  const requested = updateApp(model, { type: "shared-game-ui-effect", effect: effectFromRust });
  const effect = requested.effects[0];
  if (effect?.type !== "download-flight-log") throw new Error("Missing download effect");
  return { ...requested, effect };
}

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

describe("flight log download state and common presentation", () => {
  it.each([
    { label: "Result Summary", phase: "result", resultTab: "summary", replayViewMode: "cinematic" },
    { label: "Replay Analysis", phase: "replay", resultTab: "summary", replayViewMode: "analysis" },
    { label: "Replay Telemetry", phase: "replay", resultTab: "summary", replayViewMode: "telemetry" },
    { label: "Replay Cinematic", phase: "replay", resultTab: "summary", replayViewMode: "cinematic" }
  ] as const)("preserves shared ordered download sections and navigation across backends in $label", ({ phase, replayViewMode }) => {
    for (const mode of ["screen", "phone-vr", "webxr"] as const) {
      for (const analysisChart of ["map", "altitude", "speed"] as const) {
        const actions: GameUiAction[] = phase === "result"
          ? []
          : [{ kind: "set_replay_view", view: replayViewMode }];
        if (phase === "replay" && replayViewMode === "analysis" && analysisChart !== "map") {
          actions.push({ kind: "set_analysis_chart", chart: analysisChart });
        }
        const model: AppModel = {
          ...readyModel(phase, { actions }), presentation: { type: "ready", mode }
        };
        const panel = viewForModel(model).panels[0];
        if (panel?.sections === undefined) throw new Error("Shared download panel sections are unavailable");
        expect(panel.anchor).toBe("menu");
        expect(panel.sections.length).toBeGreaterThan(0);
        const exports = panel.controls.filter((control) => control.id.startsWith("game-flight-log-"));
        const analysisFlow = phase === "replay" && replayViewMode === "analysis";
        expect(exports.map((control) => control.id)).toEqual(["game-flight-log-csv", "game-flight-log-json", "game-flight-log-notice"]);
        expect(new Set(panel.sections.map((section) => section.id)).size).toBe(panel.sections.length);
        expect(panel.sections.find((section) => section.id === "flight-log-actions")?.layout).toBe("row");
        if (analysisFlow) {
          expect(panel.controls.some((control) => control.kind === "chart")).toBe(true);
          expect(panel.controls.some((control) => control.kind === "range")).toBe(true);
        }
        expect(exports.map((control) => control.section)).toEqual(["flight-log-actions", "flight-log-actions", "flight-log-notice"]);
        if (phase === "replay") {
          expect(panel.controls.find((control) => control.id === "game-replay-technical")).toMatchObject({
            kind: "button", section: "replay-technical", presentation: { kind: "disclosure", expanded: false }
          });
          expect(panel.controls.find((control) => control.id === "game-replay-technical-content")).toBeUndefined();
        }
        expect(panel.controls.map((control) => control.id)).toEqual(panel.sections.flatMap((section) =>
          panel.controls.filter((control) => control.section === section.id).map((control) => control.id)));
        expect([panel.size.width, panel.size.height].every(Number.isFinite)).toBe(true);
        expect(panel.size.width).toBeGreaterThan(0);
        expect(panel.size.height).toBeGreaterThan(0);
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
        const navigation = phase === "result"
          ? ["game-result-replay", "game-result-retry", "game-result-setup", "game-result-title", "game-result-open-analysis"]
          : ["game-replay-return", "game-replay-camera", "game-replay-view-mode", "game-replay-play-pause", ...(replayViewMode === "cinematic" ? []
            : ["game-replay-cursor"])];
        for (const id of [...navigation, "game-flight-log-csv", "game-flight-log-json"]) {
          expect(panel.controls.find((control) => control.id === id)?.enabled, id).toBe(true);
        }
      }
    }
  });

  it.each([
    { label: "Result Summary", phase: "result", resultTab: "summary" },
    { label: "Replay Analysis", phase: "replay", resultTab: "summary" }
  ] as const)("keeps Screen $label downloads and exits in nonabsolute flow", async ({ phase }) => {
    const browser = new BrowserWindow({ width: 1280, height: 720 });
    vi.stubGlobal("window", browser);
    vi.stubGlobal("document", browser.document);
    const stylesheet = browser.document.createElement("style");
    stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
    browser.document.head.append(stylesheet);
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = browser.document.createElement("main");
    browser.document.body.append(root);
    const dispatch = vi.fn<(action: UiAction) => void>();
    const screen = new ScreenUiAdapter(root as unknown as HTMLElement, dispatch);
    try {
      const model = readyModel(phase, { platform: { viewport: { width: 1280, height: 720, root_font_size: 16 } }, actions: phase === "result"
        ? []
        : [{ kind: "set_replay_view", view: "analysis" }] });
      const view = viewForModel(model, { width: 1280, height: 720, root_font_size: 16 });
      const panel = view.panels[0];
      if (panel?.sections === undefined || view.sharedPresentation === undefined) throw new Error("Missing shared flow layout");
      screen.render(view);
      const shell = root.querySelector(".screen-ui-shell");
      if (!(shell instanceof browser.HTMLElement)) throw new Error("Missing flow shell");
      expect(shell.style.overflowY).toBe("auto");
      expect(shell.dataset.scrollY).toBe("true");
      expect(Array.from(root.querySelectorAll(".screen-ui-section"), (section) => section.getAttribute("data-section-id")))
        .toEqual(panel.sections.map((section) => section.id));
      for (const section of panel.sections) {
        const element = root.querySelector(`[data-section-id="${section.id}"]`);
        const content = element?.querySelector(".screen-ui-section-controls");
        if (!(element instanceof browser.HTMLElement) || !(content instanceof browser.HTMLElement)) throw new Error("Missing flow section");
        const computed = browser.getComputedStyle(content);
        expect(computed.display).toBe(section.layout === "row" ? "flex" : "grid");
        if (section.layout === "row") expect(computed.flexWrap).toBe("wrap");
        const controls = panel.controls.filter((candidate) => candidate.section === section.id);
        expect(content.children.length).toBe(controls.length);
        for (const [index, control] of controls.entries()) {
          const node = content.children[index];
          if (!(node instanceof browser.HTMLElement)) throw new Error(`Missing flow control ${control.id}`);
          expect(node.style.position).toBe("");
          expect(browser.getComputedStyle(node).position).not.toBe("absolute");
          if (control.kind !== "button") continue;
          if (!(node instanceof browser.HTMLButtonElement)) throw new Error("Missing flow button");
          expect(node.dataset.controlId).toBe(control.id);
          expect(node.disabled).toBe(!control.enabled);
          const minimumHeight = control.presentation?.kind === "action" && control.presentation.emphasis === "primary"
            ? view.sharedPresentation.theme.primary_min_height_px : view.sharedPresentation.theme.button_min_height_px;
          expect(browser.getComputedStyle(node).minHeight).toBe(`${String(minimumHeight)}px`);
          expect(minimumHeight).toBeGreaterThanOrEqual(44);
          if (control.enabled) {
            node.focus();
            expect(browser.document.activeElement).toBe(node);
            node.click();
          }
        }
      }
      expect(dispatch.mock.calls.map(([action]) => action)).toEqual(panel.controls.filter((control) => control.kind === "button" && control.enabled)
        .map((control) => ({ type: "activate", controlId: control.id })));
      for (const format of ["csv", "json"] as const) {
        expect(beginDownload(model, format).effect.format).toBe(format);
      }
    } finally {
      await browser.happyDOM.abort();
    }
  });

  it.each(["result", "replay"] as const)("exposes explicit CSV/JSON choices in phase %s for Screen and VR", async (phase) => {
    const browser = new BrowserWindow();
    vi.stubGlobal("window", browser);
    vi.stubGlobal("document", browser.document);
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const root = browser.document.createElement("div");
    const dispatch = vi.fn();
    const screen = new ScreenUiAdapter(root as unknown as HTMLElement, dispatch);
    for (const mode of ["screen", "phone-vr", "webxr"] as const) {
      const model: AppModel = { ...readyModel(phase), presentation: { type: "ready", mode } };
      const view = viewForModel(model);
      const panel = view.panels[0];
      expect(panel?.anchor).toBe("menu");
      const controls = panel?.controls.filter((control) => control.id === "game-flight-log-csv" || control.id === "game-flight-log-json");
      expect(controls).toHaveLength(2);
      screen.render(view);
      const button = root.querySelector('[data-control-id="game-flight-log-csv"]');
      if (!(button instanceof browser.HTMLButtonElement)) throw new Error("Required download button is unavailable");
      button.click();
      expect(dispatch).toHaveBeenLastCalledWith({ type: "activate", controlId: "game-flight-log-csv" });
      expect(root.textContent).toContain("速度の差から推定した加速度");
    }
    await browser.happyDOM.abort();
  });

  it.each(["title", "flight_setup", "briefing_preparing", "briefing_ready", "countdown", "briefing_failed", "attract"] as const)("rejects export commands outside valid record phases: %s", (phase) => {
    withSharedGameUiSession(phase, {}, (session, platform) => {
      const observed = updateApp({ ...createInitialAppModel(), presentation: { type: "ready", mode: "screen" } as const },
        { type: "game-session-synced", ...readRuntimeSessionProjection(session) }).model;
      const view = session.readSharedUi(platform);
      const model = updateApp(observed, { type: "shared-game-ui-observed", view }).model;
      const request = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-flight-log-json" } }).effects[0];
      if (request?.type !== "update-shared-game-ui") throw new Error("Missing widget request");
      expect(() => session.updateSharedUi(request.action, platform)).toThrow(/ActionUnavailable/);
      expect(view.panels.flatMap(panel => panel.controls).some(control => control.id === "game-flight-log-json")).toBe(false);
    });
  });

  it("reports synchronous output failures and allows another command immediately", () => {
    const initial = readyModel("result");
    const requested = beginDownload(initial);
    expect(requested.model).toBe(initial);
    expect(requested.effect).toEqual({ type: "download-flight-log", format: "csv" });
    const failed = updateApp(requested.model, { type: "flight-log-download-failed", message: "Injected export failure" });
    expect(failed.model.flightLogDownload).toEqual({ kind: "failed", message: "Injected export failure" });
    expect(flightLogDownloadNotice(failed.model.flightLogDownload)).toContain("Injected export failure");
    const retried = beginDownload(failed.model, "json");
    expect(retried.effect).toEqual({ type: "download-flight-log", format: "json" });
    const completed = updateApp(retried.model, { type: "flight-log-download-requested" });
    expect(completed.model.flightLogDownload).toEqual({ kind: "requested" });
    expect(viewForModel(completed.model).panels[0]?.controls.find(control => control.id === "game-flight-log-csv")?.enabled).toBe(true);
    expect(flightLogDownloadNotice(completed.model.flightLogDownload)).toContain("ダウンロードを要求した");
  });

  it("clears output feedback when leaving or replacing the record context", () => {
    const requested = updateApp(readyModel("result"), { type: "flight-log-download-requested" }).model;
    const retry = updateApp(requested, { type: "shared-game-ui-effect", effect: { kind: "session", command: { kind: "retry" } } });
    expect(retry.model.flightLogDownload).toEqual({ kind: "idle" });
    const hidden = updateApp(requested, { type: "page-hidden" });
    expect(hidden.model.flightLogDownload).toEqual({ kind: "idle" });
    expect(updateApp(readyModel("title"), { type: "flight-log-download-requested" }).model.flightLogDownload).toEqual({ kind: "idle" });
  });

  it("disables output while an actual record read is pending", () => {
    const busy: AppModel = { ...readyModel("result"), recordOpen: { kind: "loading", requestId: 4, id: 1 } };
    expect(updateApp(busy, { type: "shared-game-ui-effect", effect: { kind: "platform", command: { kind: "download_record_json" } } }).effects).toEqual([]);
    expect(viewForModel(busy).panels[0]?.controls.find(control => control.id === "game-flight-log-csv")?.enabled).toBe(false);
  });
});

describe("browser download side-effect boundary", () => {
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
