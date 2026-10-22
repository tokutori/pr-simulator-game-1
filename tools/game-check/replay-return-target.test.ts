import { readFileSync } from "node:fs";
import { describe, expect, expectTypeOf, it } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { GameSessionUiState } from "../../web/src/app/app-state.js";
import type { ReplayReturnTarget, TailGameSessionProjection } from "../../web/src/app/session-snapshot.js";
import { selectSharedReplayView, sharedGameUiPlatform } from "./shared-game-ui-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { validateUiViewModel } from "../../web/src/render/contracts/ui.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function launch(session: AppSessionFacade): void {
  session.executeSharedSessionCommand({ kind: "open_setup" }); session.executeSharedSessionCommand({ kind: "prepare" }); session.executeSharedSessionCommand({ kind: "start_countdown" });
  session.observeCountdown(session.captureCountdownToken(), 1, true); session.observeCountdown(session.captureCountdownToken(), 1, true); session.observeCountdown(session.captureCountdownToken(), 1, true);
}

function verifyReturnViews(session: TailAppSessionFacade, expected: ReplayReturnTarget): void {
  const projection = readRuntimeSessionProjection(session);
  if (projection.phase !== "replay") throw new Error("Expected Rust Replay projection");
  expect(projection.returnTarget).toBe(expected);
  const observed = updateApp(createInitialAppModel(), { type: "game-session-synced", ...projection }).model;
  expect(observed.gameSession).toMatchObject({ phase: "replay", returnTarget: expected });
  verifyUiReturnViews(session, expected);
}

function verifyUiReturnViews(session: TailAppSessionFacade, expected: ReplayReturnTarget): void {
  for (const mode of ["screen", "phone-vr", "webxr"] as const) {
    for (const replayViewMode of ["analysis", "cinematic", "telemetry"] as const) {
      const platform = sharedGameUiPlatform({ presentation_mode: mode === "phone-vr" ? "phone_vr" : mode });
      selectSharedReplayView(session, platform, replayViewMode);
      const shared = session.readSharedUi(platform);
      const model = updateApp({ ...createInitialAppModel(), presentation: { type: "ready", mode } as const },
        { type: "shared-game-ui-observed", view: shared }).model;
      const view = createGameViewModel(shared);
      validateUiViewModel(view);
      const controls = view.panels.flatMap((panel) => panel.controls).filter((control) => control.id === "game-replay-return");
      expect(controls).toHaveLength(1);
      expect(controls[0]).toMatchObject({ kind: "button", enabled: true, label: expected === "title" ? "Titleへ戻る" : "Resultへ戻る" });
      const action = updateApp(model, { type: "ui-action", action: { type: "activate", controlId: "game-replay-return" } });
      const request = action.effects[0];
      if (request?.type !== "update-shared-game-ui") throw new Error("Missing widget request");
      expect(session.updateSharedUi(request.action, platform).effects)
        .toEqual([{ kind: "session", command: { kind: "leave_replay" } }]);
    }
  }
}

describe("Rust-owned Replay exit projection", () => {
  it("requires a return target only in the immutable Replay state", () => {
    expectTypeOf<Extract<GameSessionUiState, { phase: "replay" }>["returnTarget"]>().toEqualTypeOf<ReplayReturnTarget>();
    expectTypeOf<Extract<TailGameSessionProjection, { phase: "replay" }>["returnTarget"]>().toEqualTypeOf<ReplayReturnTarget>();
    expectTypeOf<Exclude<GameSessionUiState, { phase: "replay" }>>().not.toHaveProperty("returnTarget");
  });

  it("labels current Replay for Result and imported archives for Title in every presentation and view", () => {
    const session = new TailAppSessionFacade(new HybridGameSessionBridge(0, 21, 22), physics_hz());
    try {
      launch(session); session.executeSharedSessionCommand({ kind: "abort" });
      const modernRecord = session.exportRecordJson();
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      verifyReturnViews(session, "result");
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(session.readLifecycle().phase).toBe("result");
      session.openArchive(modernRecord);
      verifyReturnViews(session, "title");
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(session.readLifecycle().phase).toBe("title");
      const unknownRecord = JSON.parse(modernRecord) as { header: { environment_version: number } };
      unknownRecord.header.environment_version = 99;
      session.openArchive(JSON.stringify(unknownRecord));
      verifyReturnViews(session, "title");
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      expect(session.readLifecycle().phase).toBe("title");
    } finally { session.dispose(); }
  });


});
