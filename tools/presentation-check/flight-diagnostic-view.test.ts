import { describe, expect, it, vi } from "vitest";
import { PerspectiveCamera, StereoCamera } from "three";
import { createInitialAppModel, flightDiagnosticNotice, flightRecoveryControlId, gameSessionSnapshot, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel, GameSessionProjection } from "../../web/src/app/app-state.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createFlightFrameViewDraft, finalizeFlightFrameView } from "../../web/src/app/flight-frame-view.js";
import { createCorrelatedFlightHudPort, flightSnapshotForView } from "../../web/src/app/flight-diagnostic-browser.js";
import type { FlightControllerIdentity } from "../../web/src/app/flight-diagnostic-browser.js";
import { FlightController } from "../../web/src/game/flight-controller.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";

const initial = createInitialAppModel();
const raw = new Array<number>(33).fill(0);
raw[4] = 8;
raw[7] = 1;
raw[19] = -1;
raw[20] = 12;
raw[21] = 8;
raw[22] = 9;
raw[31] = 1;
const snapshot = parseFlightSnapshot(raw);
const off = { telemetry: false, attitude: false, wind: false, flightPath: false, angleOfAttack: false, warnings: false };

function projection(phaseCode: number, countdownRemaining = 0): GameSessionProjection {
  return { phaseCode, countdownRemaining, snapshot: phaseCode === 5 || phaseCode === 6 || phaseCode === 7 ? snapshot : null,
    controlModeCode: 0, difficulty: initial.difficulty, configurationMetadata: null, canResume: true };
}

function awaitingLaunch(previous: AppModel = initial): AppModel {
  const briefing = gameSessionState(3, 0, null);
  if (briefing === null) throw new Error("Missing briefing fixture");
  const begun = updateApp({ ...previous, presentation: { type: "ready", mode: "screen" }, gameSession: briefing },
    { type: "ui-action", action: { type: "activate", controlId: "game-briefing-start" } }).model;
  if (begun.pendingGameRequestId === null) throw new Error("Missing launch request");
  const countdown = updateApp(begun, { type: "game-operation-completed", requestId: begun.pendingGameRequestId, ...projection(4, 3) }).model;
  return updateApp(countdown, { type: "game-session-synced", ...projection(4, 0) }).model;
}

function activated(previous: AppModel = initial): AppModel {
  const awaiting = awaitingLaunch(previous);
  if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing expected launch identity");
  return updateApp(awaiting, { type: "flight-controller-activated", launchRequestId: awaiting.expectedLaunchRequestId, projection: projection(5) }).model;
}

function stopped(): AppModel {
  const active = activated();
  if (active.flightRuntime.kind !== "active") throw new Error("Missing active epoch");
  return updateApp(active, { type: "flight-controller-stopped", launchRequestId: active.flightRuntime.launchRequestId, cause: "input security failure", snapshot }).model;
}

function viewer() {
  const camera = new PerspectiveCamera(60, 1280 / 720, 0.05, 100_000);
  const stereo = new StereoCamera();
  stereo.aspect = 0.5;
  camera.updateMatrixWorld(true);
  return captureConfiguredViewerFrame(camera, stereo);
}

describe("Atomic operational-flight failure and shared safe view", () => {
  it("retains the exact countdown identity after operation completion and consumes it only at successful activation", () => {
    const awaiting = awaitingLaunch();
    expect(awaiting.pendingGameRequestId).toBeNull();
    expect(awaiting.expectedLaunchRequestId).not.toBeNull();
    const wrong = updateApp(awaiting, { type: "flight-controller-activated", launchRequestId: awaiting.nextRequestId, projection: projection(5) });
    expect(wrong.model).toBe(awaiting);
    const active = activated();
    expect(active.expectedLaunchRequestId).toBeNull();
    expect(active.flightRuntime.kind).toBe("active");
    if (active.flightRuntime.kind !== "active") throw new Error("Missing active epoch");
    expect(updateApp(active, { type: "flight-controller-activated", launchRequestId: active.flightRuntime.launchRequestId, projection: projection(5) }).model).toBe(active);
  });

  it("preserves the first stop across Pause/Resume and rejects stale old-epoch notifications", () => {
    const failure = stopped();
    if (failure.flightRuntime.kind !== "stopped") throw new Error("Missing stop diagnostic");
    const duplicate = updateApp(failure, { type: "flight-controller-stopped", launchRequestId: failure.flightRuntime.launchRequestId, cause: "duplicate", snapshot });
    expect(duplicate.model).toBe(failure);
    const paused = updateApp(failure, { type: "game-session-synced", ...projection(6) }).model;
    const resume = updateApp(paused, { type: "ui-action", action: { type: "activate", controlId: "game-flight-resume" } });
    expect(resume.model).toBe(paused);
    expect(resume.effects).toEqual([]);
    const next = activated(failure);
    expect(flightDiagnosticNotice(next)).toBeNull();
    expect(updateApp(next, { type: "flight-controller-stopped", launchRequestId: failure.flightRuntime.launchRequestId, cause: "old callback", snapshot }).model).toBe(next);
  });

  it.each(["screen", "phone-vr", "webxr"] as const)("keeps mandatory notice and safe action in %s for every Information and Custom all-off", (mode) => {
    for (const informationCode of [0, 1, 2, 3, 4]) {
      const failure = { ...stopped(), presentation: { type: "ready" as const, mode }, difficulty: { ...initial.difficulty, informationCode, hudProfile: off } };
      const draft = createFlightFrameViewDraft(failure, snapshot, viewer());
      const view = finalizeFlightFrameView(draft, draft.headHud);
      expect(view.description).toContain("input security failure");
      expect(view.panels[0]?.controls.some((control) => control.kind === "button" && control.id === flightRecoveryControlId(failure) && control.enabled)).toBe(true);
      expect(view.panels[0]?.controls.some((control) => control.id === "game-flight-resume")).toBe(false);
      if (mode !== "screen" && informationCode === 4) {
        expect(draft.headHud.kind).toBe("visible");
        if (draft.headHud.kind === "visible") expect(draft.headHud.layer.elements.some((element) => element.id === "head-flight-stop")).toBe(true);
      }
    }
  });

  it("retains the mandatory Menu when Head geometry is unavailable", () => {
    const failure = { ...stopped(), presentation: { type: "ready" as const, mode: "phone-vr" as const } };
    const draft = createFlightFrameViewDraft(failure, snapshot, unavailableViewerFrame("invalid-view-geometry"));
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(view.headHud.kind).toBe("absent");
    expect(view.panels[0]?.controls.some((control) => control.id === "flight-stop-notice")).toBe(true);
    expect(view.panels[0]?.controls.some((control) => control.id === flightRecoveryControlId(failure))).toBe(true);
  });

  it("blocks stale telemetry and normal controls when the post-launch projection getter failed", () => {
    const awaiting = awaitingLaunch(stopped());
    if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing expected epoch");
    const failure = updateApp(awaiting, { type: "flight-launch-failed", launchRequestId: awaiting.expectedLaunchRequestId, cause: "snapshot unavailable", outcome: { kind: "projection-unavailable" } }).model;
    const draft = createFlightFrameViewDraft(failure, snapshot, viewer());
    const view = finalizeFlightFrameView(draft, draft.headHud);
    expect(failure.gameSession.kind).toBe("countdown");
    expect(draft.snapshot).toBeNull();
    expect(draft.hud).toBeNull();
    expect(view.headHud.kind).toBe("absent");
    expect(view.description).toContain("snapshot unavailable");
    expect(view.panels[0]?.controls.some((control) => control.id === flightRecoveryControlId(failure))).toBe(true);
    expect(updateApp(failure, { type: "game-session-synced", ...projection(5) }).model).toBe(failure);
    expect(updateApp(failure, { type: "ui-action", action: { type: "activate", controlId: "game-countdown-cancel" } }).effects).toEqual([]);
  });

  it("keeps fresh initial projection on reset failure and rejects a previous controller snapshot", () => {
    const awaiting = awaitingLaunch(stopped());
    if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing launch");
    const freshRaw = [...raw];
    freshRaw[0] = 15;
    const fresh = parseFlightSnapshot(freshRaw);
    const failure = updateApp(awaiting, { type: "flight-launch-failed", launchRequestId: awaiting.expectedLaunchRequestId, cause: "reset failed",
      outcome: { kind: "launched", projection: { ...projection(5), snapshot: fresh } } }).model;
    const session = {};
    const controller = { currentSnapshot: snapshot };
    expect(flightSnapshotForView(failure.flightRuntime, 5, gameSessionSnapshot(failure.gameSession),
      { session, controller, launchRequestId: awaiting.expectedLaunchRequestId - 1 }, session, controller)).toBe(fresh);
  });

  it("routes a real controller input exception through fail into pure update and reuses the object only after successful reset", () => {
    let model = activated();
    if (model.flightRuntime.kind !== "active") throw new Error("Missing activation");
    const session = { snapshot: () => raw, advance_tick: vi.fn(() => raw), free: vi.fn() };
    let identity: FlightControllerIdentity<typeof session, FlightController> | null = null;
    const messages: AppMessage[] = [];
    const hud = createCorrelatedFlightHudPort({ render: vi.fn(), setVisible: vi.fn(), fail: vi.fn() }, () => identity, (captured, cause) => {
      const message: AppMessage = { type: "flight-controller-stopped", launchRequestId: captured.launchRequestId, cause, snapshot: captured.controller.currentSnapshot };
      messages.push(message);
      model = updateApp(model, message).model;
    });
    let throwInput = true;
    const input = { readIntent: () => { if (throwInput) throw new Error("input failed"); return { roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 }; },
      reset: vi.fn(), resume: vi.fn(), suspend: vi.fn(), dispose: vi.fn() };
    const controller = new FlightController(session, input, { setFlightPose: vi.fn() }, hud, 60, () => []);
    identity = { session, controller, launchRequestId: model.flightRuntime.launchRequestId };
    controller.onFrame(0);
    controller.onFrame(20);
    expect(model.flightRuntime.kind).toBe("stopped");
    expect(flightDiagnosticNotice(model)).toContain("input failed");
    const oldMessage = messages[0];
    if (oldMessage === undefined) throw new Error("Missing controller failure message");
    const awaiting = awaitingLaunch(model);
    if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing next launch");
    throwInput = false;
    controller.reset(raw);
    identity = { session, controller, launchRequestId: awaiting.expectedLaunchRequestId };
    model = updateApp(awaiting, { type: "flight-controller-activated", launchRequestId: identity.launchRequestId, projection: projection(5) }).model;
    expect(updateApp(model, oldMessage).model).toBe(model);
    expect(flightDiagnosticNotice(model)).toBeNull();
    controller.onFrame(30);
    controller.onFrame(50);
    expect(session.advance_tick).toHaveBeenCalled();
  });

  it("preserves the cause on Result and Title without changing their terminal record snapshot", () => {
    const failure = stopped();
    const result = updateApp(failure, { type: "game-session-synced", ...projection(7) }).model;
    expect(gameSessionSnapshot(result.gameSession)).toBe(snapshot);
    expect(createGameViewModel(result, snapshot).panels[0]?.controls.some((control) => control.kind === "status" && control.value.includes("直前の操作飛行"))).toBe(true);
    const title = updateApp(result, { type: "game-session-synced", ...projection(0) }).model;
    expect(flightDiagnosticNotice(title)).toContain("input security failure");
  });

  it("shows a new pre-launch failure alongside the retained previous-run cause", () => {
    const awaiting = awaitingLaunch(stopped());
    if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing expected launch");
    const failed = updateApp(awaiting, { type: "flight-launch-failed", launchRequestId: awaiting.expectedLaunchRequestId,
      cause: "launch operation failed", outcome: { kind: "not-launched" } }).model;
    const status = createGameViewModel(failed, null).panels[0]?.controls.find((control) => control.kind === "status");
    expect(failed.flightRuntime).toBe(awaiting.flightRuntime);
    expect(status?.kind === "status" && status.value).toContain("直前の操作飛行");
    expect(status?.kind === "status" && status.value).toContain("launch operation failed");
  });

  it("keeps a failed safe-recovery query visible without clearing the stop or disabling retry", () => {
    const failure = stopped();
    const controlId = flightRecoveryControlId(failure);
    if (controlId === null) throw new Error("Missing safe recovery control");
    const requested = updateApp(failure, { type: "ui-action", action: { type: "activate", controlId } }).model;
    if (requested.pendingGameRequestId === null || requested.flightRuntime.kind !== "stopped") throw new Error("Missing recovery identity");
    const retry = updateApp(requested, { type: "flight-recovery-failed", requestId: requested.pendingGameRequestId,
      launchRequestId: requested.flightRuntime.launchRequestId, cause: "current Rust phase unavailable" }).model;
    const view = createGameViewModel(retry, snapshot);
    expect(retry.flightRuntime).toBe(failure.flightRuntime);
    expect(view.description).toContain("input security failure");
    expect(view.description).toContain("current Rust phase unavailable");
    expect(view.panels[0]?.controls.some((control) => control.kind === "button" && control.id === controlId && control.enabled)).toBe(true);
  });

  it("accepts only the pending safe-recovery epoch and retains its cause with fresh terminal Result", () => {
    const failure = stopped();
    const controlId = flightRecoveryControlId(failure);
    if (controlId === null || failure.flightRuntime.kind !== "stopped") throw new Error("Missing safe recovery identity");
    const requested = updateApp(failure, { type: "ui-action", action: { type: "activate", controlId } }).model;
    if (requested.pendingGameRequestId === null) throw new Error("Missing recovery request");
    const terminalRaw = [...raw];
    terminalRaw[0] = 8;
    terminalRaw[16] = 4;
    const terminal = parseFlightSnapshot(terminalRaw);
    const message: AppMessage = { type: "flight-recovery-completed", launchRequestId: failure.flightRuntime.launchRequestId,
      requestId: requested.pendingGameRequestId, projection: { ...projection(7), snapshot: terminal } };
    expect(updateApp(requested, { ...message, requestId: message.requestId + 1 }).model).toBe(requested);
    expect(updateApp(requested, { ...message, launchRequestId: message.launchRequestId + 1 }).model).toBe(requested);
    const completed = updateApp(requested, message).model;
    expect(gameSessionSnapshot(completed.gameSession)).toBe(terminal);
    expect(completed.gameSession.kind).toBe("result");
    expect(flightDiagnosticNotice(completed)).toContain("input security failure");
    expect(updateApp(completed, message).model).toBe(completed);
    const next = activated(completed);
    expect(updateApp(next, { type: "ui-action", action: { type: "activate", controlId } }).model).toBe(next);
  });

  it("keeps fresh Rust initial values after a real invalid-snapshot reset and prevents subsequent old-controller ticks", () => {
    const session = { snapshot: () => raw, advance_tick: vi.fn(() => raw), free: vi.fn() };
    const input = { readIntent: () => ({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 }), reset: vi.fn(), resume: vi.fn(), suspend: vi.fn(), dispose: vi.fn() };
    const controller = new FlightController(session, input, { setFlightPose: vi.fn() }, { render: vi.fn(), fail: vi.fn(), setVisible: vi.fn() }, 60, () => []);
    const awaiting = awaitingLaunch(stopped());
    if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing exact next epoch");
    const freshRaw = [...raw];
    freshRaw[0] = 15;
    const fresh = parseFlightSnapshot(freshRaw);
    expect(() => { controller.reset([1, 2]); }).toThrow();
    const failure = updateApp(awaiting, { type: "flight-launch-failed", launchRequestId: awaiting.expectedLaunchRequestId,
      cause: "invalid reset snapshot", outcome: { kind: "launched", projection: { ...projection(5), snapshot: fresh } } }).model;
    expect(flightSnapshotForView(failure.flightRuntime, 5, gameSessionSnapshot(failure.gameSession), null, session, controller)).toBe(fresh);
    expect(controller.currentSnapshot).not.toBe(fresh);
    controller.onFrame(0);
    controller.onFrame(20);
    expect(session.advance_tick).not.toHaveBeenCalled();
    expect(flightDiagnosticNotice(failure)).toContain("invalid reset snapshot");
  });

  it("does not clear a retained stop when the real controller constructor cannot read its new snapshot", () => {
    const awaiting = awaitingLaunch(stopped());
    if (awaiting.expectedLaunchRequestId === null) throw new Error("Missing exact next epoch");
    const session = { snapshot: () => { throw new Error("constructor snapshot failed"); }, advance_tick: vi.fn(() => raw), free: vi.fn() };
    const input = { readIntent: () => ({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 }), reset: vi.fn(), resume: vi.fn(), suspend: vi.fn(), dispose: vi.fn() };
    expect(() => new FlightController(session, input, { setFlightPose: vi.fn() }, { render: vi.fn(), fail: vi.fn(), setVisible: vi.fn() }, 60, () => [])).toThrow("constructor snapshot failed");
    const failure = updateApp(awaiting, { type: "flight-launch-failed", launchRequestId: awaiting.expectedLaunchRequestId,
      cause: "constructor snapshot failed", outcome: { kind: "launched", projection: projection(5) } }).model;
    expect(gameSessionSnapshot(failure.gameSession)).toBe(snapshot);
    expect(failure.flightRuntime).toEqual({ kind: "stopped", launchRequestId: awaiting.expectedLaunchRequestId, cause: "constructor snapshot failed" });
    expect(flightDiagnosticNotice(failure)).not.toBeNull();
  });

  it("adds a same-epoch late stop cause without rewriting the already accepted terminal Result", () => {
    const active = activated();
    if (active.flightRuntime.kind !== "active") throw new Error("Missing active epoch");
    const terminalRaw = [...raw];
    terminalRaw[16] = 4;
    const terminal = parseFlightSnapshot(terminalRaw);
    const result = updateApp(active, { type: "game-session-synced", ...projection(7), snapshot: terminal }).model;
    const failure = updateApp(result, { type: "flight-controller-stopped", launchRequestId: active.flightRuntime.launchRequestId,
      cause: "same-epoch callback failure", snapshot }).model;
    expect(failure.gameSession).toBe(result.gameSession);
    expect(gameSessionSnapshot(failure.gameSession)).toBe(terminal);
    expect({ ...failure, flightRuntime: result.flightRuntime }).toEqual(result);
    expect(flightDiagnosticNotice(failure)).toContain("same-epoch callback failure");
  });

  it("preserves terminal Result when a real onTerminal callback throws after its synchronous update", () => {
    let model = activated();
    if (model.flightRuntime.kind !== "active") throw new Error("Missing activation");
    const terminalRaw = [...raw];
    terminalRaw[16] = 4;
    const terminal = parseFlightSnapshot(terminalRaw);
    const session = { snapshot: () => raw, advance_tick: () => terminalRaw, free: vi.fn() };
    let identity: FlightControllerIdentity<typeof session, FlightController> | null = null;
    const hud = createCorrelatedFlightHudPort({ render: vi.fn(), setVisible: vi.fn(), fail: vi.fn() }, () => identity, (captured, cause) => {
      model = updateApp(model, { type: "flight-controller-stopped", launchRequestId: captured.launchRequestId, cause, snapshot: captured.controller.currentSnapshot }).model;
    });
    const input = { readIntent: () => ({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 }), reset: vi.fn(), resume: vi.fn(), suspend: vi.fn(), dispose: vi.fn() };
    const controller = new FlightController(session, input, { setFlightPose: vi.fn() }, hud, 60, () => [], () => {
      model = updateApp(model, { type: "game-session-synced", ...projection(7), snapshot: terminal }).model;
      throw new Error("terminal view callback failed");
    });
    identity = { session, controller, launchRequestId: model.flightRuntime.launchRequestId };
    controller.onFrame(0);
    controller.onFrame(20);
    expect(model.gameSession.kind).toBe("result");
    expect(gameSessionSnapshot(model.gameSession)).toBe(terminal);
    expect(flightDiagnosticNotice(model)).toContain("terminal view callback failed");
  });
});
