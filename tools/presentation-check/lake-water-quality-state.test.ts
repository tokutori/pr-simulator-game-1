import { describe, expect, it } from "vitest";
import { canSelectLakeWaterQuality, createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel, AppTransition } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import type { LakeWaterQuality } from "../../web/src/render/contracts/lake-water.js";
import type { PresentationMode } from "../../web/src/render/contracts/runtime.js";
import { validateUiViewModel } from "../../web/src/render/contracts/ui.js";

const values = Array.from({ length: 33 }, () => 0);
values[7] = 1;
values[19] = -1;
const snapshot = parseFlightSnapshot(values);

describe("manual lake water quality ownership", () => {
  it("keeps one immutable applied/requested state and emits a presentation-only effect", () => {
    const initial = readyModel();
    const requested = select(initial, "low");
    expect(initial.lakeWaterQuality).toEqual({ kind: "ready", applied: "high", cleanup: { kind: "complete" } });
    expect(requested.model.lakeWaterQuality).toEqual({ kind: "applying", applied: "high", requested: "low", requestId: initial.nextRequestId });
    expect(requested.effects).toEqual([{ type: "apply-lake-water-quality", requestId: initial.nextRequestId, quality: "low" }]);
    expect(Object.isFrozen(requested.model.lakeWaterQuality)).toBe(true);
    expect(requested.model.gameSession).toBe(initial.gameSession);
    expect(requested.model.difficulty).toBe(initial.difficulty);
    expect(requested.model.configurationMetadata).toBe(initial.configurationMetadata);
    expect(requested.model.status).toBe(initial.status);
    expect(requested.model.pendingGameRequestId).toBeNull();
    expect(select(requested.model, "medium").model).toBe(requested.model);
  });

  it("accepts only the matching completion after the requested resource swap", () => {
    const initial = readyModel();
    const requested = select(initial, "low").model;
    expect(updateApp(requested, { type: "lake-water-quality-applied", requestId: initial.nextRequestId - 1, quality: "low", cleanup: { kind: "complete" } }).model).toBe(requested);
    expect(updateApp(requested, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "high", cleanup: { kind: "complete" } }).model).toBe(requested);
    const completed = updateApp(requested, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "low", cleanup: { kind: "complete" } });
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
    const completed = updateApp(retried, { type: "lake-water-quality-applied", requestId: failed.nextRequestId, quality: "low", cleanup: { kind: "complete" } });
    expect(completed.model.lakeWaterQuality).toEqual({ kind: "ready", applied: "low", cleanup: { kind: "complete" } });
  });

  it("permits the same shared controls only in Setup or paused Settings", () => {
    const setup = readyModel();
    const paused = readyModel(6);
    expect(canSelectLakeWaterQuality(setup)).toBe(true);
    expect(canSelectLakeWaterQuality(paused)).toBe(true);
    for (const phase of [0, 2, 3, 4, 5, 7, 8, 9, 10]) {
      const model = readyModel(phase);
      expect(select(model, "low").model).toBe(model);
    }
    if (paused.gameSession.kind !== "paused-flight") throw new Error("Missing paused session");
    for (const overlay of ["menu", "help"] as const) {
      const model: AppModel = { ...paused, gameSession: { ...paused.gameSession, overlay: { kind: overlay } } };
      expect(select(model, "low").model).toBe(model);
    }
    const pending: AppModel = { ...setup, pendingGameRequestId: 9 };
    expect(select(pending, "low").model).toBe(pending);
    const exhausted: AppModel = { ...setup, nextRequestId: Number.MAX_SAFE_INTEGER };
    expect(select(exhausted, "low").model).toBe(exhausted);
    expect(updateApp(setup, { type: "ui-action", action: { type: "activate", controlId: "presentation-water-quality-auto" } }).model).toBe(setup);
  });

  it("retains quality across Rust phase synchronization, Retry and backend switching", () => {
    const initial = readyModel();
    const requested = select(initial, "medium").model;
    const selected = updateApp(requested, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "medium", cleanup: { kind: "complete" } }).model;
    const synchronized = updateApp(selected, {
      type: "game-session-synced", phaseCode: 3, controlModeCode: 0, difficulty: selected.difficulty,
      configurationMetadata: null, countdownRemaining: 0, snapshot: null
    }).model;
    expect(synchronized.lakeWaterQuality).toBe(selected.lakeWaterQuality);
    const transition = updateApp({ ...selected, presentation: { type: "ready", mode: "phone-vr" } }, {
      type: "backend-ended", mode: "phone-vr", message: "tracking lost"
    });
    expect(transition.model.lakeWaterQuality).toBe(selected.lakeWaterQuality);
    const terminal: AppModel = { ...readyModel(7), lakeWaterQuality: selected.lakeWaterQuality };
    const retry = updateApp(terminal, { type: "ui-action", action: { type: "activate", controlId: "game-result-retry" } });
    expect(retry.effects.some((effect) => effect.type === "game-session-operation" && effect.operation === "retry")).toBe(true);
    expect(retry.model.lakeWaterQuality).toBe(selected.lakeWaterQuality);
  });

  it("preserves an in-flight quality completion through BFCache suspension", () => {
    const initial = readyModel();
    const requested = select(initial, "low").model;
    const suspended = updateApp(requested, { type: "page-suspended" }).model;
    const completed = updateApp(suspended, { type: "lake-water-quality-applied", requestId: initial.nextRequestId, quality: "low", cleanup: { kind: "complete" } }).model;
    expect(completed.presentation.type).toBe("cached");
    expect(completed.lakeWaterQuality).toEqual({ kind: "ready", applied: "low", cleanup: { kind: "complete" } });
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("projects applied and pending quality into %s Setup and Settings", (mode) => {
    for (const phase of [1, 6]) {
      const initial = readyModel(phase, mode);
      const view = createGameViewModel(initial, phase === 6 ? snapshot : null);
      validateUiViewModel(view);
      const controls = view.panels[0]?.controls.filter((control) => control.id.startsWith("presentation-water-quality-")) ?? [];
      expect(controls).toHaveLength(4);
      const choices = controls.filter((control) => control.kind === "button");
      expect(choices.map((control) => control.label)).toEqual(["Low", "Medium", "High"]);
      expect(choices.every((control) => control.enabled)).toBe(true);
      expect(choices.map((control) => control.presentation?.kind === "choice" && control.presentation.selected)).toEqual([false, false, true]);
      const requested = select(initial, "low").model;
      const pending = createGameViewModel(requested, phase === 6 ? snapshot : null).panels[0]?.controls ?? [];
      expect(pending.filter((control) => control.id.startsWith("presentation-water-quality-") && control.kind === "button").every((control) => !control.enabled)).toBe(true);
      expect(pending.find((control) => control.id === "presentation-water-quality-status")).toMatchObject({ value: "High · Lowを適用中" });
    }
  });
});

function select(model: AppModel, quality: LakeWaterQuality): AppTransition {
  return updateApp(model, { type: "ui-action", action: { type: "activate", controlId: `presentation-water-quality-${quality}` } });
}

function readyModel(phase = 1, mode: PresentationMode = "screen"): AppModel {
  const initialized = updateApp(createInitialAppModel(), { type: "initialize" });
  const ready = updateApp(initialized.model, {
    type: "presentation-initialized", requestId: 1, activeMode: mode,
    webXrAvailable: true, phoneVrAvailable: true, status: "Ready"
  }).model;
  const phaseSnapshot = phase === 5 || phase === 6 ? snapshot : phase === 7 ? parseFlightSnapshot(values.map((value, index) => index === 16 ? 4 : value)) : null;
  const session = gameSessionState(phase, 0, phaseSnapshot, true, null, "legacy_three_axis", phase === 9 ? "result" : undefined);
  if (session === null) throw new Error("Invalid quality fixture session");
  return { ...ready, gameSession: session.kind === "paused-flight" ? { ...session, overlay: { kind: "settings" } } : session };
}
