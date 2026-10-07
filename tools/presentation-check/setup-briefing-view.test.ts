import { readFileSync } from "node:fs";
import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createFlightFrameViewDraft, finalizeFlightFrameView } from "../../web/src/app/flight-frame-view.js";
import { NO_HEAD_HUD_VIEW } from "../../web/src/presentation/head-hud-view.js";
import { NO_ENVIRONMENT_BRIEFING } from "../../web/src/game/environment-briefing.js";
import type { EnvironmentBriefingProjection } from "../../web/src/game/environment-briefing.js";
import { requestMenuLayout, layoutMeasuredMenu, menuViewport, menuRectInViewport } from "../../web/src/presentation/menu-layout.js";
import type { MenuTextMeasurement, MenuTextRequest } from "../../web/src/render/contracts/menu-layout.js";
import { uiButtonLabel, uiControlBackground, validateUiViewModel } from "../../web/src/render/contracts/ui.js";
import type { UiViewModel } from "../../web/src/render/contracts/ui.js";
import { configuredViewerFixture } from "./viewer-fixture.js";

const environment: EnvironmentBriefingProjection = Object.freeze({ kind: "available", name: "Declared weather",
  representativePositionNedMeters: [1, 2, -10.5] as const, representativeAltitudeMeters: 10.5,
  representativeWindNedMetersPerSecond: [-0.25, 0.5, -0.1] as const, spatialVariation: "uniform", windBasis: "assumed" });

function preparationModel(phaseCode: number): AppModel {
  const base = createInitialAppModel();
  const gameSession = gameSessionState(phaseCode, 3, null);
  if (gameSession === null) throw new Error("Invalid phase fixture");
  return { ...base, status: "", presentation: { type: "ready", mode: "screen" }, gameSession,
    configurationMetadata: { ...base.difficulty, catalogVersion: 1, scenarioId: 1, scenarioVersion: 1,
      aircraftModelVersion: 1, environmentVersion: 1, controllerProfileVersion: 4, seedHigh: 21930, seedLow: 24472 } };
}

function statusValue(view: UiViewModel, id: string): string {
  const control = view.panels.flatMap((panel) => panel.controls).find((entry) => entry.id === id);
  if (control?.kind !== "status") throw new Error(`Missing status ${id}`);
  return control.value;
}

function measurementFor(request: MenuTextRequest): MenuTextMeasurement {
  const characterWidth = request.fontMeters * 0.55;
  const columns = Math.max(1, Math.floor(request.widthMeters / characterWidth));
  const lines = request.value.split("\n").flatMap((paragraph, paragraphIndex) => {
    const characters = Array.from(paragraph);
    const chunks = characters.length === 0 ? [""] : Array.from({ length: Math.ceil(characters.length / columns) },
      (_textChunkSlot, index) => characters.slice(index * columns, (index + 1) * columns).join(""));
    return chunks.map((value) => ({ value, paragraphIndex, advanceMeters: Array.from(value).length * characterWidth,
      leftMeters: 0, rightMeters: Array.from(value).length * characterWidth,
      ascentMeters: value.trim() === "" ? 0 : request.fontMeters * 0.65,
      descentMeters: value.trim() === "" ? 0 : request.fontMeters * 0.15 }));
  });
  return { identity: request.identity, lines };
}

describe("Flight Setup and Briefing presentation", () => {
  it.each(["screen", "phone-vr", "webxr"] as const)("exposes the same direct candidates and selected values in %s", (mode) => {
    const model: AppModel = { ...preparationModel(1), presentation: { type: "ready", mode } };
    const view = createGameViewModel(model, null, null, NO_HEAD_HUD_VIEW, environment);
    validateUiViewModel(view);
    const controls = view.panels[0]?.controls ?? [];
    const choices = controls.filter((control) => control.kind === "button" && control.presentation?.kind === "choice");
    expect(choices).toHaveLength(18);
    expect(choices.map((choice) => choice.id)).not.toContain("game-setup-select-preset-4");
    expect(controls.map((control) => control.id)).not.toContain("game-state");
    expect(controls.map((control) => control.id)).not.toContain("game-setup-weather");
    expect(statusValue(view, "game-weather-conditions")).toContain("代表水平風速: 0.56 m/s · 風向: 北西から（296.6°）");
    expect(statusValue(view, "game-weather-conditions")).toContain("鉛直流: 上昇 0.10 m/s");
    expect(controls.find((control) => control.id === "game-setup-start")).toMatchObject({ label: "飛行準備へ進む", presentation: { kind: "action", emphasis: "primary" } });
    expect(view.panels[0]?.anchor).toBe("menu");
  });

  it("keeps version and seed in a closed Model-owned disclosure", () => {
    const model = preparationModel(3);
    const closed = createGameViewModel(model, null);
    expect(JSON.stringify(closed)).not.toContain("Catalog:");
    expect(JSON.stringify(closed)).not.toContain("Seed:");
    expect(closed.panels[0]?.controls.find((control) => control.id === "game-briefing-technical")).toMatchObject({ presentation: { expanded: false } });
    const open = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-technical" } });
    expect(open.effects).toEqual([]);
    const view = createGameViewModel(open.model, null);
    expect(statusValue(view, "game-briefing-technical-content")).toContain("Seed: 21930:24472");
    expect(view.panels[0]?.controls.find((control) => control.id === "game-briefing-start")).toMatchObject({ label: "発進カウントダウンを開始" });
    expect(updateApp(open.model, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-technical" } }).model.briefingDetailsOpen).toBe(false);
    const pending = { ...model, pendingGameRequestId: 1 };
    expect(updateApp(pending, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-technical" } }).model).toBe(pending);
    const setup = preparationModel(1);
    expect(updateApp(setup, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-technical" } }).model).toBe(setup);
  });

  it("closes technical information on phase changes and retains it during redraw", () => {
    const model = { ...preparationModel(3), briefingDetailsOpen: true };
    for (const phaseCode of [1, 3, 4]) {
      const synced = updateApp(model, { type: "game-session-synced", phaseCode, controlModeCode: 0,
        difficulty: model.difficulty, configurationMetadata: model.configurationMetadata, countdownRemaining: 3, snapshot: null });
      expect(synced.model.briefingDetailsOpen).toBe(phaseCode === 3);
    }
  });

  it.each([
    [1, "game-setup-select-weather-1", 1],
    [3, "game-briefing-start", 4],
    [4, "game-countdown-cancel", 1]
  ] as const)("retains operation feedback after pending is cleared in phase %i", (phaseCode, controlId, successPhaseCode) => {
    const model = preparationModel(phaseCode);
    const requested = updateApp(model, { type: "ui-action", action: { type: "activate", controlId } });
    const requestId = requested.model.pendingGameRequestId;
    if (requestId === null) throw new Error("Missing operation request");
    for (const message of ["条件を確認する必要がある。", "Operation completed", ""]) {
      for (const includeProjection of [false, true]) {
        const failed = updateApp(requested.model, { type: "game-operation-failed", requestId, message,
          ...(includeProjection ? { currentSession: { phaseCode, controlModeCode: model.controlModeCode,
            difficulty: model.difficulty, configurationMetadata: model.configurationMetadata, countdownRemaining: 3,
            snapshot: null, canResume: false } } : {}) });
        expect(failed.model.pendingGameRequestId).toBeNull();
        expect(failed.model.gameSession.phaseCode).toBe(phaseCode);
        for (const mode of ["screen", "phone-vr", "webxr"] as const) {
          const failedView = createGameViewModel({ ...failed.model, presentation: { type: "ready", mode } }, null);
          const feedback = failedView.panels[0]?.controls.find((control) => control.id === "game-preparation-feedback");
          if (message === "") expect(feedback).toBeUndefined();
          else {
            expect(feedback).toMatchObject({ kind: "status", label: "通知", value: message });
            expect(failedView.panels[0]?.controls.map((control) => control.id)).not.toContain("game-state");
          }
        }
        const retry = updateApp(failed.model, { type: "ui-action", action: { type: "activate", controlId } });
        const retryId = retry.model.pendingGameRequestId;
        if (retryId === null) throw new Error("Missing retry request");
        const completed = updateApp(retry.model, { type: "game-operation-completed", requestId: retryId,
          phaseCode: successPhaseCode, controlModeCode: model.controlModeCode, difficulty: model.difficulty,
          configurationMetadata: model.configurationMetadata, countdownRemaining: 3, snapshot: null });
        expect(completed.model.status).toBe("");
        expect(createGameViewModel(completed.model, null).panels[0]?.controls.map((control) => control.id))
          .not.toContain("game-preparation-feedback");
      }
    }
  });

  it("preserves current feedback when an old operation failure arrives", () => {
    const model = preparationModel(3);
    const requested = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-start" } });
    const requestId = requested.model.pendingGameRequestId;
    if (requestId === null) throw new Error("Missing operation request");
    const failed = updateApp(requested.model, { type: "game-operation-failed", requestId, message: "開始操作を受理できない。" }).model;
    const stale = updateApp(failed, { type: "game-operation-failed", requestId: requestId - 1, message: "古い失敗" });
    expect(stale.model).toBe(failed);
    expect(statusValue(createGameViewModel(stale.model, null), "game-preparation-feedback")).toBe("開始操作を受理できない。");
  });

  it("displays failed Briefing feedback once in its preparation result", () => {
    const model = preparationModel(8);
    const requested = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-retry" } });
    const requestId = requested.model.pendingGameRequestId;
    if (requestId === null) throw new Error("Missing operation request");
    const message = "準備に必要な条件を取得できない。";
    const failed = updateApp(requested.model, { type: "game-operation-failed", requestId, message });
    const view = createGameViewModel(failed.model, null);
    expect(statusValue(view, "game-briefing-readiness")).toBe(message);
    expect(view.panels.flatMap((panel) => panel.controls).filter((control) => control.kind === "status" && control.value === message)).toHaveLength(1);
    expect(view.panels[0]?.controls.map((control) => control.id)).not.toContain("game-preparation-feedback");
  });

  it("disables every Setup interaction while a Rust operation is pending", () => {
    const base = preparationModel(1);
    const model = { ...base, pendingGameRequestId: 1, difficulty: { ...base.difficulty, informationCode: 4 } };
    const view = createGameViewModel(model, null);
    expect(view.panels.flatMap((panel) => panel.controls).filter((control) => control.kind !== "status").every((control) => !control.enabled)).toBe(true);
  });

  it.each([1, 2, 3, 4, 8])("projects preparation progress without a debug-state output in phase %i", (phaseCode) => {
    const model = preparationModel(phaseCode);
    const view = createGameViewModel(model, null);
    expect(view.panels[0]?.controls.map((control) => control.id)).not.toContain("game-state");
    expect(view.panels[0]?.controls.find((control) => control.id === "game-preparation-progress")).toMatchObject({ kind: "status" });
    validateUiViewModel(view);
  });

  it("uses the same immutable Weather projection for the VR frame path", () => {
    const model: AppModel = { ...preparationModel(3), presentation: { type: "ready", mode: "phone-vr" } };
    const draft = createFlightFrameViewDraft(model, null, configuredViewerFixture(), "ja", environment);
    const view = finalizeFlightFrameView(draft, NO_HEAD_HUD_VIEW);
    const direct = createGameViewModel(model, null, null, NO_HEAD_HUD_VIEW, environment);
    expect(view).toEqual(direct);
    expect(statusValue(createGameViewModel(model, null, null, NO_HEAD_HUD_VIEW, NO_ENVIRONMENT_BRIEFING), "game-weather-conditions"))
      .toContain("代表風情報を取得できない");
  });

  it.each([1, 2, 3, 8])("states undefined launch-wind thresholds and preserves Rust preparation phase %i", (phaseCode) => {
    for (const metadata of [environment, NO_ENVIRONMENT_BRIEFING]) {
      const view = createGameViewModel(preparationModel(phaseCode), null, null, NO_HEAD_HUD_VIEW, metadata);
      const weather = statusValue(view, "game-weather-conditions");
      expect(weather).toContain("気象による発進風速の上限・下限: 未定義。");
      expect(weather).not.toContain("準備処理で開始可能かを確認する");
      if (phaseCode === 1) {
        expect(view.panels.flatMap((panel) => panel.controls).some((control) => control.id === "game-briefing-readiness")).toBe(false);
      } else {
        const readiness = statusValue(view, "game-briefing-readiness");
        if (phaseCode === 3) expect(readiness).toBe("✓ 発進準備完了");
        else expect(readiness).not.toContain("発進準備完了");
      }
    }
  });

  it.each([
    [[-1, 0, 0], "北から（0.0°）", "鉛直流なし"],
    [[0, -1, 0.2], "東から（90.0°）", "下降 0.20 m/s"],
    [[1, 0, -0.2], "南から（180.0°）", "上昇 0.20 m/s"],
    [[0, 1, 0], "西から（270.0°）", "鉛直流なし"],
    [[0, 0, -0.2], "方向なし（水平風は無風）", "上昇 0.20 m/s"]
  ] as const)("derives the meteorological origin and vertical direction from %o", (wind, from, vertical) => {
    const view = createGameViewModel(preparationModel(3), null, null, NO_HEAD_HUD_VIEW, {
      ...environment, representativeWindNedMetersPerSecond: wind
    });
    const weather = statusValue(view, "game-weather-conditions");
    expect(weather).toContain(from);
    expect(weather).toContain(vertical);
  });

  it.each([1, 3])("keeps CTA, back and disclosure reachable in a short portrait VR menu in phase %i", (phaseCode) => {
    const model = { ...preparationModel(phaseCode), briefingDetailsOpen: true };
    const view = createGameViewModel(model, null, null, NO_HEAD_HUD_VIEW, environment);
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const request = requestMenuLayout(panel, { width: 0.5, height: 0.6 }, {
      fontMeters: 0.04, font: { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 },
      locale: "ja", previousLabel: "前頁", nextLabel: "次頁", caption: view.description
    });
    const result = layoutMeasuredMenu(request, request.texts.map(measurementFor));
    if (result.kind !== "ready") throw new Error(result.reason);
    const document = result.document;
    const initial = menuViewport(document, 0);
    expect(initial.maximumOffsetMeters).toBeGreaterThan(0);
    const ids = phaseCode === 1 ? ["game-setup-start", "game-setup-back"]
      : ["game-briefing-start", "game-briefing-cancel", "game-briefing-technical"];
    for (const id of ids) {
      const layout = document.controls.find((control) => control.control.id === id);
      if (layout === undefined) throw new Error(`Missing control ${id}`);
      const progress = Math.min(1, Math.max(0, layout.bounds.y / initial.maximumOffsetMeters));
      expect(menuRectInViewport(menuViewport(document, progress), layout.bounds).kind, id).toBe("visible");
    }
    expect(request.texts.some((text) => text.value.includes("● Full\n利用可能な計器"))).toBe(phaseCode === 1);
  });

  it("projects selection, emphasis and disclosure semantics into the DOM while preserving focus and scroll", async () => {
    const window = new Window({ width: 320, height: 240 });
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    let model = preparationModel(1);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    try {
      adapter.render(createGameViewModel(model, null));
      expect(root.querySelectorAll("fieldset")).toHaveLength(4);
      const selected = root.querySelector<HTMLButtonElement>('[data-control-id="game-setup-select-information-0"]');
      const shell = root.querySelector<HTMLElement>(".screen-ui-shell");
      if (selected === null || shell === null) throw new Error("Missing Setup DOM");
      expect(selected.getAttribute("aria-pressed")).toBe("true");
      expect(selected.textContent).toContain("利用可能な計器");
      selected.focus();
      shell.scrollTop = 40;
      adapter.render(createGameViewModel({ ...model, difficulty: { ...model.difficulty, informationCode: 2 } }, null));
      expect(root.querySelector('[data-control-id="game-setup-select-information-0"]')).toBe(selected);
      expect(documentRef.activeElement).toBe(selected);
      expect(shell.scrollTop).toBe(40);
      expect(root.querySelector('[data-control-id="game-setup-start"]')?.getAttribute("data-emphasis")).toBe("primary");
      expect(root.querySelector('[data-control-id="game-setup-back"]')?.getAttribute("data-emphasis")).toBe("secondary");
      selected.click();
      expect(actions).toEqual([{ type: "activate", controlId: "game-setup-select-information-0" }]);
      model = preparationModel(3);
      adapter.render(createGameViewModel(model, null));
      const disclosure = root.querySelector<HTMLButtonElement>('[data-control-id="game-briefing-technical"]');
      if (disclosure === null) throw new Error("Missing disclosure");
      disclosure.focus();
      expect(disclosure.getAttribute("aria-expanded")).toBe("false");
      expect(root.textContent).not.toContain("Seed:");
      model = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-briefing-technical" } }).model;
      adapter.render(createGameViewModel(model, null));
      expect(root.querySelector('[data-control-id="game-briefing-technical"]')).toBe(disclosure);
      expect(documentRef.activeElement).toBe(disclosure);
      expect(disclosure.getAttribute("aria-expanded")).toBe("true");
      expect(root.querySelector("#game-briefing-technical-content")?.textContent).toContain("Seed: 21930:24472");
      const stylesheet = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
      expect(stylesheet).toContain("max-height: calc(100dvh - 2rem)");
      expect(stylesheet).toContain("overflow-y: auto");
      expect(stylesheet).toContain("repeat(auto-fit, minmax(min(100%, 12rem), 1fr))");
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it("uses a common selected marker and visual hierarchy for VR painting", () => {
    const controls = createGameViewModel(preparationModel(1), null).panels[0]?.controls ?? [];
    const choice = controls.find((control) => control.id === "game-setup-select-information-0");
    const primary = controls.find((control) => control.id === "game-setup-start");
    const secondary = controls.find((control) => control.id === "game-setup-back");
    if (choice?.kind !== "button" || primary === undefined || secondary === undefined) throw new Error("Missing controls");
    expect(uiButtonLabel(choice)).toContain("● Full\n利用可能な計器");
    expect(uiControlBackground(primary)).not.toBe(uiControlBackground(secondary));
  });
});
