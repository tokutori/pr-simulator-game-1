import { readFileSync } from "node:fs";
import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { canSelectLakeWaterQuality, createInitialAppModel, gameSessionPhase, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel, AppTransition } from "../../web/src/app/app-state.js";
import { createGameViewModel, sharedGameUiAction } from "../../web/src/app/game-view.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { SessionPhase } from "../../web/src/app/session-snapshot.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import type { LakeWaterQuality } from "../../web/src/render/contracts/lake-water.js";
import type { PresentationMode } from "../../web/src/render/contracts/runtime.js";
import { validateUiViewModel } from "../../web/src/render/contracts/ui.js";
import { currentGameSessionProjectionFixture, currentSharedGameUiFixture, sharedGameUiPlatform, sharedScreenRenderSnapshot, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import type { SharedGameUiFixtureOptions } from "../game-check/shared-game-ui-fixture.js";
import type { GameUiView } from "../../web/src/generated/game-ui-view.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";


describe("manual lake water quality ownership", () => {
  it("keeps one immutable applied/requested state and emits a presentation-only effect", () => {
    const initial = readyModel();
    const requested = select(initial, "low");
    expect(initial.lakeWaterQuality).toEqual({ kind: "ready", applied: "high", cleanup: { kind: "complete" } });
    expect(requested.model.lakeWaterQuality).toEqual({ kind: "applying", applied: "high", requested: "low", requestId: initial.nextRequestId });
    expect(requested.effects).toEqual([{ type: "apply-lake-water-quality", requestId: initial.nextRequestId, quality: "low" }]);
    expect(Object.isFrozen(requested.model.lakeWaterQuality)).toBe(true);
    expect(requested.model.gameSession).toBe(initial.gameSession);
    expect(requested.model.status).toBe(initial.status);
    expect(requested.model.recordOpen).toEqual({ kind: "idle" });
    expect(select(requested.model, "medium").model).toBe(requested.model);
  });

  it("accepts only the matching completion after the requested resource swap", () => {
    const initial = readyModel();
    const requested = select(initial, "low").model;
    expect(updateApp(requested, { type: "lake-water-quality-applied", requestId: initial.nextRequestId - 1, quality: "low", cleanup: { kind: "complete" } }).model).toBe(requested);
    expect(updateApp(requested, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "high", cleanup: { kind: "complete" } }).model).toBe(requested);
    const completed = completeQuality(requested, "low", initial.nextRequestId);
    expect(completed.model.lakeWaterQuality).toEqual({ kind: "ready", applied: "low", cleanup: { kind: "complete" } });
    expect(completed.effects).toEqual([]);
    expect(select(completed.model, "low").model).toBe(completed.model);
    expect(updateApp(completed.model, { type: "lake-water-quality-failed", requestId: initial.nextRequestId, message: "late failure" }).model).toBe(completed.model);
  });

  it("retains the applied quality on failure and rejects results from an earlier retry", () => {
    const initial = readyModel();
    const requested = select(initial, "low").model;
    const failed = updateApp(requested, { type: "lake-water-quality-failed", requestId: initial.nextRequestId, message: "allocation failed" }).model;
    expect(failed.lakeWaterQuality).toEqual({ kind: "failed", applied: "high", requested: "low", message: "allocation failed" });
    expect(canSelectLakeWaterQuality(failed)).toBe(true);
    const retried = select(failed, "low").model;
    expect(updateApp(retried, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "low", cleanup: { kind: "complete" } }).model).toBe(retried);
    expect(updateApp(retried, { type: "lake-water-quality-failed", requestId: initial.nextRequestId, message: "late failure" }).model).toBe(retried);
    const completed = completeQuality(retried, "low", failed.nextRequestId);
    expect(completed.model.lakeWaterQuality).toEqual({ kind: "ready", applied: "low", cleanup: { kind: "complete" } });
  });

  it("permits the same shared controls only in Setup or paused Settings", () => {
    const setup = readyModel();
    const paused = readyModel("flight_paused");
    expect(canSelectLakeWaterQuality(setup)).toBe(true);
    expect(canSelectLakeWaterQuality(paused)).toBe(true);
    for (const phase of ["title", "briefing_preparing", "briefing_ready", "countdown", "flight_running", "result", "briefing_failed", "replay", "attract"] as const) {
      const model = readyModel(phase);
      expect(select(model, "low").model).toBe(model);
    }
    for (const overlay of ["menu", "help"] as const) {
      const model: AppModel = { ...paused, sharedGameUi: currentSharedGameUiFixture("flight_paused",
        overlay === "menu" ? {} : { actions: [{ kind: "set_pause_overlay", overlay }] }) };
      expect(select(model, "low").model).toBe(model);
    }
    const pending: AppModel = { ...setup, recordOpen: { kind: "loading" as const, requestId: 9, id: 1 } };
    expect(select(pending, "low").model).toBe(pending);
    const exhausted: AppModel = { ...setup, nextRequestId: Number.MAX_SAFE_INTEGER };
    expect(select(exhausted, "low").model).toBe(exhausted);
    expect(updateApp(setup, { type: "ui-action", action: { type: "activate", controlId: "presentation-water-quality-auto" } }).model).toBe(setup);
  });

  it("retains quality across Rust phase synchronization, Retry and backend switching", () => {
    const initial = readyModel();
    const requested = select(initial, "medium").model;
    const selected = completeQuality(requested, "medium", initial.nextRequestId).model;
    const synchronized = updateApp(selected, {
      type: "game-session-synced", ...currentGameSessionProjectionFixture("briefing_ready")
    }).model;
    expect(synchronized.lakeWaterQuality).toBe(selected.lakeWaterQuality);
    const transition = updateApp({ ...selected, presentation: { type: "ready", mode: "phone-vr" } }, {
      type: "backend-ended", mode: "phone-vr", message: "tracking lost"
    });
    expect(transition.model.lakeWaterQuality).toBe(selected.lakeWaterQuality);
    const terminal: AppModel = { ...readyModel("result"), lakeWaterQuality: selected.lakeWaterQuality };
    const retry = updateApp(terminal, { type: "ui-action", action: { type: "activate", controlId: "game-result-retry" } });
    expect(retry.effects.some((effect) => effect.type === "update-shared-game-ui" && effect.action.kind === "session" && effect.action.command.kind === "retry")).toBe(true);
    expect(retry.model.lakeWaterQuality).toBe(selected.lakeWaterQuality);
  });

  it("preserves an in-flight quality completion through BFCache suspension", () => {
    const initial = readyModel();
    const requested = select(initial, "low").model;
    const suspended = updateApp(requested, { type: "page-suspended" }).model;
    const completed = completeQuality(suspended, "low", initial.nextRequestId).model;
    expect(completed.presentation.type).toBe("cached");
    expect(completed.lakeWaterQuality).toEqual({ kind: "ready", applied: "low", cleanup: { kind: "complete" } });
  });

  it("projects Auto-applied water during Running without changing the retained upper quality or domain state", () => {
    const initial = readyModel("flight_running");
    const snapshot = { ...sharedScreenRenderSnapshot("50", "low"),
      selection: { kind: "auto" as const, manual_scale: "100" as const, manual_shadow: "full" as const, manual_water_quality: "high" as const } };
    const observed = updateApp(initial, { type: "screen-render-observed", snapshot });
    expect(observed.model.lakeWaterQuality.applied).toBe("low");
    expect(observed.model.screenRenderScale?.selection).toMatchObject({ manual_water_quality: "high" });
    expect(observed.model.gameSession).toBe(initial.gameSession);
    expect(observed.effects).toEqual([]);
    const paused = { ...observed.model, gameSession: readyModel("flight_paused").gameSession, sharedGameUi: readyModel("flight_paused").sharedGameUi };
    const requested = select(paused, "low");
    expect(requested.effects).toEqual([{ type: "apply-lake-water-quality", requestId: paused.nextRequestId, quality: "low" }]);
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("projects applied and pending quality into %s Setup and Settings", (mode) => {
    for (const phase of ["flight_setup", "flight_paused"] as const) {
      const initial = readyModel(phase, mode);
      const view = createGameViewModel(qualityView(initial));
      validateUiViewModel(view);
      const controls = view.panels[0]?.controls.filter((control) => control.id.startsWith("presentation-water-quality-")) ?? [];
      expect(controls).toHaveLength(4);
      const choices = controls.filter((control) => control.kind === "button");
      expect(choices.map((control) => control.label)).toEqual(["Low", "Medium", "High"]);
      expect(choices.every((control) => control.enabled)).toBe(true);
      expect(choices.map((control) => control.presentation?.kind === "choice" && control.presentation.selected)).toEqual([false, false, true]);
      const requested = select(initial, "low").model;
      const pending = createGameViewModel(qualityView(requested)).panels[0]?.controls ?? [];
      expect(pending.filter((control) => control.id.startsWith("presentation-water-quality-") && control.kind === "button").every((control) => !control.enabled)).toBe(true);
      expect(pending.find((control) => control.id === "presentation-water-quality-status")).toMatchObject({ value: "High · Lowを適用中" });
    }
  });

  it("keeps paused Settings quality choices and back focus inside the shared scrollable panel", async () => {
    const window = new Window({ width: 320, height: 240 });
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const stylesheet = documentRef.createElement("style");
    stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
    documentRef.head.append(stylesheet);
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    try {
      const initial = readyModel("flight_paused");
      const initialView = qualityView(initial);
      adapter.render(createGameViewModel(initialView));
      const shell = root.querySelector<HTMLElement>('.screen-ui-shell[data-scene="Flight"][data-overlay="PauseSettings"]');
      const back = root.querySelector<HTMLButtonElement>('[data-control-id="game-pause-settings-back"]');
      if (shell === null || back === null) throw new Error("Missing paused Settings card");
      const qualitySection = root.querySelector<HTMLElement>('[data-section-id="water-quality"]');
      if (qualitySection === null) throw new Error("Missing shared water quality section");
      expect(qualitySection.dataset.layout).toBe("choice_grid");
      const choices = Array.from(qualitySection.querySelectorAll<HTMLButtonElement>("button"));
      expect(choices).toHaveLength(3);
      expect(choices.map((choice) => choice.dataset.controlId)).toEqual([
        "presentation-water-quality-low", "presentation-water-quality-medium", "presentation-water-quality-high"
      ]);
      expect(choices.map((choice) => choice.getAttribute("aria-pressed"))).toEqual(["false", "false", "true"]);
      expect(shell.dataset.shell).toBe(initialView.layout.shell);
      expect(shell.style.maxHeight).toBe(`${String(initialView.layout.maximum_height_px)}px`);
      expect(window.getComputedStyle(shell as never).overflowY).toBe("auto");
      expect(window.getComputedStyle(shell as never).pointerEvents).toBe("auto");
      expect(back.disabled).toBe(false);
      back.focus();
      shell.scrollTop = 40;
      const requested = select(initial, "low").model;
      if (requested.lakeWaterQuality.kind !== "applying") throw new Error("Missing paused quality request");
      adapter.render(createGameViewModel(qualityView(requested)));
      expect(root.querySelector('[data-control-id="game-pause-settings-back"]')).toBe(back);
      expect(documentRef.activeElement).toBe(back);
      expect(shell.scrollTop).toBe(40);
      const completed = completeQuality(requested, "low", requested.lakeWaterQuality.requestId).model;
      adapter.render(createGameViewModel(qualityView(completed)));
      expect(documentRef.activeElement).toBe(back);
      expect(shell.scrollTop).toBe(40);
      expect(root.querySelector('[data-control-id="presentation-water-quality-low"]')?.getAttribute("aria-pressed")).toBe("true");
      back.click();
      expect(actions).toEqual([{ type: "activate", controlId: "game-pause-settings-back" }]);
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });
});

describe("Screen render scale preference", () => {
  function selectScale(model: AppModel, scale: "100" | "75" | "50"): AppTransition {
    return updateApp(model, { type: "shared-game-ui-effect", effect: {
      kind: "platform", command: { kind: "set_screen_render_scale", scale }
    } });
  }

  it("keeps one preference while preserving domain state and other render settings", () => {
    const initial = readyModel();
    expect(initial.screenRenderScale).toEqual(sharedScreenRenderSnapshot());
    let selected = initial;
    for (const scale of ["75", "50", "100"] as const) {
      const updated = selectScale(selected, scale);
      expect(updated.model.screenRenderScale).toBe(selected.screenRenderScale);
      expect(updated.effects).toEqual([{ type: "select-screen-render-scale", scale }]);
      expect(updated.model.gameSession).toBe(initial.gameSession);
      expect(updated.model.lakeWaterQuality).toBe(initial.lakeWaterQuality);
      selected = updateApp(updated.model, { type: "screen-render-observed", snapshot: sharedScreenRenderSnapshot(scale) }).model;
    }
    expect(selectScale(readyModel("flight_paused"), "50").effects).toEqual([{ type: "select-screen-render-scale", scale: "50" }]);
  });

  it("rejects stale changes outside Screen settings and retains the preference across VR exit", () => {
    const selected = updateApp(readyModel(), { type: "screen-render-observed", snapshot: sharedScreenRenderSnapshot("75") }).model;
    for (const phase of ["title", "briefing_preparing", "briefing_ready", "countdown", "flight_running", "result", "briefing_failed", "replay", "attract"] as const) {
      const model = { ...readyModel(phase), screenRenderScale: selected.screenRenderScale };
      expect(selectScale(model, "50").model).toBe(model);
    }
    for (const mode of ["phone-vr", "webxr"] as const) {
      const model = { ...readyModel("flight_setup", mode), screenRenderScale: selected.screenRenderScale };
      expect(selectScale(model, "50").model).toBe(model);
      const ended = updateApp(model, { type: "backend-ended", mode, message: "session ended" }).model;
      expect(ended.screenRenderScale).toEqual(sharedScreenRenderSnapshot("75"));
    }
    const switching = updateApp(selected, { type: "shared-game-ui-effect", effect: {
      kind: "platform", command: { kind: "set_presentation", mode: "phone_vr" }
    } }).model;
    expect(selectScale(switching, "50").model).toBe(switching);
    expect(switching.screenRenderScale).toEqual(sharedScreenRenderSnapshot("75"));
  });
});

function select(model: AppModel, quality: LakeWaterQuality): AppTransition {
  const transition = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: `presentation-water-quality-${quality}` } });
  const requested = transition.effects.find((effect) => effect.type === "update-shared-game-ui");
  if (requested?.type !== "update-shared-game-ui") return transition;
  return withQualitySession(model, (session, platform) => {
    const admitted = sharedGameUiAction(session.readSharedUi(platform), {
      type: "activate", controlId: `presentation-water-quality-${quality}`
    });
    if (admitted === null) return { model, effects: [] };
    let result: AppTransition = { model: transition.model, effects: [] };
    for (const effect of session.updateSharedUi(admitted, platform).effects) {
      const next = updateApp(result.model, { type: "shared-game-ui-effect", effect });
      result = { model: next.model, effects: [...result.effects, ...next.effects] };
    }
    return result;
  });
}

function completeQuality(model: AppModel, quality: LakeWaterQuality, requestId: number): AppTransition {
  const observed = updateApp(model, { type: "screen-render-observed", snapshot: sharedScreenRenderSnapshot("100", quality) }).model;
  return updateApp(observed, { type: "lake-water-quality-applied", requestId, quality, cleanup: { kind: "complete" } });
}

function qualityOptions(model: AppModel): SharedGameUiFixtureOptions {
  const phase = gameSessionPhase(model.gameSession);
  const mode = model.presentation.type === "ready" ? model.presentation.mode : "screen";
  const overlay = model.sharedGameUi?.selections.pause_overlay ?? "menu";
  const quality = model.lakeWaterQuality;
  const operation = quality.kind === "applying" ? { kind: "applying" as const, requested: quality.requested }
    : quality.kind === "failed" ? { kind: "failed" as const, requested: quality.requested, cause: quality.message }
    : quality.cleanup.kind === "failed" ? { kind: "cleanup_failed" as const, cause: quality.cleanup.message }
    : { kind: "idle" as const };
  return { ...(phase === "flight_paused" && overlay !== "menu" ? { actions: [{ kind: "set_pause_overlay", overlay }] } : {}),
    platform: { presentation_mode: mode === "phone-vr" ? "phone_vr" : mode,
      operation_pending: model.recordOpen.kind === "loading",
      ...(model.screenRenderScale === null ? {} : { screen_render_scale: model.screenRenderScale }),
      water_quality: quality.applied, water_quality_operation: operation } };
}

function qualityView(model: AppModel): GameUiView {
  return withQualitySession(model, (session, platform) => session.readSharedUi(platform));
}

function withQualitySession<Value>(model: AppModel,
  observe: (session: TailAppSessionFacade, platform: GameUiPlatformInput) => Value): Value {
  const options = qualityOptions(model);
  const phase = gameSessionPhase(model.gameSession);
  if (phase === "boot") throw new Error("Quality fixture requires a Rust session");
  return withSharedGameUiSession(phase, {
    ...options, platform: { ...options.platform, operation_pending: false, water_quality_operation: { kind: "idle" } }
  }, (session) => observe(session, sharedGameUiPlatform(options.platform)));
}

function readyModel(phase: SessionPhase = "flight_setup", mode: PresentationMode = "screen"): AppModel {
  const initialized = updateApp(createInitialAppModel(), { type: "initialize" });
  const ready = updateApp(initialized.model, {
    type: "presentation-initialized", requestId: 1, activeMode: mode,
    webXrAvailable: true, phoneVrAvailable: true, status: "Ready"
  }).model;
  return withSharedGameUiSession(phase, {
    ...(phase === "flight_paused" ? { actions: [{ kind: "set_pause_overlay", overlay: "settings" }] } : {}),
    platform: { presentation_mode: mode === "phone-vr" ? "phone_vr" : mode } }, (session, platform) => ({
    ...ready, screenRenderScale: sharedScreenRenderSnapshot(), gameSession: readRuntimeSessionProjection(session),
    sharedGameUi: session.readSharedUi(platform) }));
}
