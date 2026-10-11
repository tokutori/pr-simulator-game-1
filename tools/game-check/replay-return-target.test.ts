import { readFileSync } from "node:fs";
import { describe, expect, expectTypeOf, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { createInitialAppModel, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel, GameSessionUiState, ReplayReturnTarget, TailGameSessionProjection } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { validateUiViewModel } from "../../web/src/render/contracts/ui.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function launch(session: AppSessionFacade): void {
  session.executeOperation("open-setup"); session.executeOperation("prepare"); session.executeOperation("start-flight");
  session.advanceCountdown(); session.advanceCountdown(); session.advanceCountdown(); session.launch();
}

function verifyReturnViews(session: TailAppSessionFacade, expected: ReplayReturnTarget): void {
  const projection = readRuntimeSessionProjection(session);
  if (projection.phaseCode !== 9) throw new Error("Expected Rust Replay projection");
  expect(projection.returnTarget).toBe(expected);
  expect(Object.isFrozen(projection)).toBe(true);
  const observed = updateApp(createInitialAppModel(), { type: "game-session-synced", ...projection }).model;
  expect(observed.gameSession).toMatchObject({ kind: "replay", returnTarget: expected });
  const data = session.readAnalysisDataset({ kind: "unavailable", reason: "not_requested" });
  verifyUiReturnViews({ ...observed, flightAnalysis: data }, expected);
}

function verifyUiReturnViews(observed: AppModel, expected: ReplayReturnTarget): void {
  for (const mode of ["screen", "phone-vr", "webxr"] as const) {
    for (const replayViewMode of ["analysis", "cinematic", "telemetry"] as const) {
      const model = { ...observed, presentation: { type: "ready", mode } as const, replayViewMode };
      const view = createGameViewModel(model, null);
      validateUiViewModel(view);
      const controls = view.panels.flatMap((panel) => panel.controls).filter((control) => control.id === "game-replay-return");
      expect(controls).toHaveLength(1);
      expect(controls[0]).toMatchObject({ kind: "button", enabled: true, label: expected === "title" ? "Titleへ戻る" : "Resultへ戻る" });
      const action = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-replay-return" } });
      expect(action.effects).toContainEqual({ type: "game-session-operation", operation: "leave-replay", requestId: model.nextRequestId });
    }
  }
}

describe("Rust-owned Replay exit projection", () => {
  it("requires a return target only in the immutable Replay state", () => {
    expectTypeOf<Extract<GameSessionUiState, { kind: "replay" }>["returnTarget"]>().toEqualTypeOf<ReplayReturnTarget>();
    expectTypeOf<Extract<TailGameSessionProjection, { phaseCode: 9 }>["returnTarget"]>().toEqualTypeOf<ReplayReturnTarget>();
    expectTypeOf<Extract<GameSessionUiState, { kind: "title" }>>().not.toHaveProperty("returnTarget");
    for (const layout of ["tail_incidence"] as const) {
      expect(gameSessionState(9, 0, null, false, null, layout)).toBeNull();
      expect(gameSessionState(9, 0, null, false, null, layout, "invalid" as ReplayReturnTarget)).toBeNull();
    }
  });

  it("labels current Replay for Result and imported archives for Title in every presentation and view", () => {
    const session = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      launch(session); session.executeOperation("abort");
      const modernRecord = session.exportRecordJson();
      session.executeOperation("enter-replay");
      verifyReturnViews(session, "result");
      session.executeOperation("leave-replay");
      expect(session.readLifecycle().phaseCode).toBe(7);
      session.openArchive(modernRecord);
      verifyReturnViews(session, "title");
      session.executeOperation("leave-replay");
      expect(session.readLifecycle().phaseCode).toBe(0);
      const unknownRecord = JSON.parse(modernRecord) as { header: { environment_version: number } };
      unknownRecord.header.environment_version = 99;
      session.openArchive(JSON.stringify(unknownRecord));
      verifyReturnViews(session, "title");
      session.executeOperation("leave-replay");
      expect(session.readLifecycle().phaseCode).toBe(0);
    } finally { session.dispose(); }
  });


  it("rejects invalid owner boundaries and stale observations without an archive flag", () => {
    const bridge = new HybridGameSessionBridge(0, 21, 22);
    const session = new TailAppSessionFacade(bridge, physics_hz());
    try {
      expect(() => session.readReplayReturnTarget()).toThrow("Replay phase");
      launch(session); session.executeOperation("abort"); session.executeOperation("enter-replay");
      const owner = vi.spyOn(bridge, "is_archived_replay");
      owner.mockReturnValueOnce("true" as unknown as boolean);
      expect(() => session.readReplayReturnTarget()).toThrow("boolean");
      expect(session.readReplayReturnTarget()).toBe("result");
      owner.mockImplementationOnce(() => { session.executeOperation("leave-replay"); return false; });
      expect(() => session.readReplayReturnTarget()).toThrow("ownership changed");
      owner.mockRestore();
      expect(session.readLifecycle().phaseCode).toBe(7);
      session.executeOperation("enter-replay");
      expect(session.readReplayReturnTarget()).toBe("result");
    } finally { session.dispose(); }
  });
});
