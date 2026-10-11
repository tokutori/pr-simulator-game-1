import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createInitialAppModel, gameSessionSnapshot, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppMessage } from "../../web/src/app/app-state.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import type { TailControllerSnapshot } from "../../web/src/game/tail-flight-controller.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const disposals: (() => void)[] = [];
afterEach(() => { for (const dispose of disposals.splice(0)) dispose(); });

function fixture() {
  const facade = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  facade.executeOperation("open-setup");
  facade.executeOperation("prepare");
  facade.executeOperation("start-flight");
  while (facade.advanceCountdown() > 0) continue;
  facade.launch();
  let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...facade.readGameSessionProjection() }).model;
  const effects: AppEffect[] = [];
  const messages: AppMessage[] = [];
  const dispatch = (message: AppMessage): void => {
    messages.push(message);
    const next = updateApp(model, message);
    model = next.model;
    effects.push(...next.effects);
  };
  const port = { snapshot_json: () => facade.flightPort.snapshot_json(), control_profile_json: () => facade.flightPort.control_profile_json(),
    advance_tick_json: vi.fn((json: string) => facade.flightPort.advance_tick_json(json)), free: () => { facade.dispose(); } };
  const input = { readDemand: vi.fn(() => ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0,
    pilotPositionCommand: { kind: "hold" as const } })), reset: vi.fn(), suspend: vi.fn(), resume: vi.fn(), dispose: vi.fn() };
  const renderer = { setFlightPose: vi.fn<(pose: FlightRenderPose | null) => void>() };
  const display = { render: vi.fn<(snapshot: FlightDisplaySnapshot) => void>(), setVisible: vi.fn<(visible: boolean) => void>() };
  const bindings = new FlightControllerUiBindings();
  const binding = bindings.bindAppDisplay(port, display, () => facade.readGameSessionProjection(), () => controller.currentDisplaySnapshot, dispatch);
  const controller = new TailFlightController(port, input, renderer, binding.port, physics_hz(), () => [], binding.onTerminal);
  dispatch({ type: "flight-controller-ready", identity: binding.identity });
  disposals.push(() => { controller.dispose(); });
  const abortAtNextTick = (): void => {
    port.advance_tick_json.mockImplementationOnce(() => {
      facade.executeOperation("abort");
      return facade.flightPort.snapshot_json();
    });
  };
  return { facade, controller, port, input, renderer, display, bindings, binding, effects, messages, dispatch, abortAtNextTick, model: () => model };
}

describe("authoritative Result synchronization across tail adapter failures", () => {
  it("commits the new Retry terminal callback with the new controller binding", () => {
    const trial = fixture();
    trial.abortAtNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    const previousResult = trial.facade.readGameSessionProjection();
    if (previousResult.phaseCode !== 7) throw new Error("Expected the first Rust Result");
    trial.facade.executeOperation("retry");
    trial.facade.executeOperation("start-flight");
    while (trial.facade.advanceCountdown() > 0) continue;
    trial.facade.launch();
    trial.dispatch({ type: "game-session-synced", ...trial.facade.readGameSessionProjection() });
    const binding = trial.bindings.bindAppDisplay(trial.port, trial.display, () => trial.facade.readGameSessionProjection(),
      () => trial.controller.currentDisplaySnapshot, trial.dispatch);
    trial.controller.reset(trial.port.snapshot_json(), binding.port, binding.onTerminal);
    trial.dispatch({ type: "flight-controller-ready", identity: binding.identity });
    const current = trial.model();
    trial.dispatch({ type: "tail-controller-terminal", identity: trial.binding.identity, projection: previousResult });
    expect(trial.model()).toBe(current);
    trial.abortAtNextTick();
    trial.controller.onFrame(20);
    trial.controller.onFrame(30);
    expect(trial.model().gameSession.kind).toBe("result");
    expect(trial.model().flightExecution).toMatchObject({ kind: "ready", identity: binding.identity });
    expect(trial.effects.filter((effect) => effect.type === "persist-flight-record")).toHaveLength(2);
  });

  it("retains the previous binding and callback after a failed reset", () => {
    const trial = fixture();
    const previous = trial.controller.currentSnapshot;
    const callback = vi.fn();
    const brokenHud = { render: () => { throw new Error("Retry HUD reset failed"); }, fail: vi.fn(), setVisible: vi.fn() };
    expect(() => { trial.controller.reset(trial.port.snapshot_json(), brokenHud, callback); }).toThrow("Retry HUD reset failed");
    expect(trial.controller.currentSnapshot).toBe(previous);
    trial.controller.reset(trial.port.snapshot_json());
    trial.abortAtNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(callback).not.toHaveBeenCalled();
    expect(trial.model().gameSession.kind).toBe("result");
    expect(trial.messages.filter((message) => message.type === "tail-controller-terminal")).toHaveLength(1);
  });

  it.each(["pose", "hud", "input", "pose-and-cleanup"] as const)("retains Rust phase, record and finalization when %s throws", (site) => {
    const trial = fixture();
    trial.abortAtNextTick();
    const primary = new Error(`Injected ${site} terminal failure`);
    if (site === "pose" || site === "pose-and-cleanup") trial.renderer.setFlightPose.mockImplementationOnce(() => { throw primary; });
    if (site === "hud") trial.display.render.mockImplementationOnce(() => { throw primary; });
    if (site === "input") trial.input.suspend.mockImplementation(() => { throw primary; });
    if (site === "pose-and-cleanup") trial.input.suspend.mockImplementation(() => { throw new Error("Injected cleanup failure"); });
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    const result = trial.facade.readGameSessionProjection();
    if (result.phaseCode !== 7 || result.display.kind !== "available") throw new Error("Expected the original Rust Result");
    expect(trial.model().gameSession).toMatchObject({ kind: "result", phaseCode: 7 });
    expect(gameSessionSnapshot(trial.model().gameSession)).toEqual(result.display.value);
    expect(trial.model().flightExecution.kind).toBe("ready");
    expect(trial.model().status).toContain(primary.message);
    if (site === "pose-and-cleanup") expect(trial.model().status).toContain("Injected cleanup failure");
    expect(trial.effects).toEqual([{ type: "persist-flight-record" }, { type: "load-flight-analysis", requestId: 1 }]);
    expect(trial.model().pendingAnalysisRequestId).toBe(1);
    const archive: unknown = JSON.parse(trial.facade.exportRecordJson());
    expect(archive).toMatchObject({ schema_version: 6, finalization: { reason: "manual_abort", terminal_tick: 0, failure: null } });
    const snapshot = trial.controller.currentSnapshot;
    expect(snapshot.phaseCode).toBe(7);
    trial.controller.onFrame(2_000);
    trial.controller.resume();
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
    expect(trial.messages.filter((message) => message.type === "flight-controller-stopped")).toEqual([]);
    expect(trial.messages.filter((message) => message.type === "tail-controller-terminal")).toHaveLength(2);
  });

  it("retains a nonterminal history snapshot while the authoritative Rust phase is paused", () => {
    const trial = fixture();
    const retained = trial.controller.currentDisplaySnapshot;
    trial.facade.executeOperation("pause");
    trial.dispatch({ type: "game-session-synced", ...trial.facade.readGameSessionProjection() });
    trial.binding.port.fail("Pause presentation failed");
    expect(trial.model().gameSession).toMatchObject({ kind: "paused-flight", snapshot: { phaseCode: 6 } });
    expect(trial.model().flightExecution).toMatchObject({ kind: "stopped", snapshot: retained });
  });

  it("rejects terminal callbacks from an old controller and feedback after leaving Result", () => {
    const trial = fixture();
    trial.abortAtNextTick();
    trial.dispatch({ type: "flight-controller-ready", identity: { ...trial.binding.identity, controllerId: trial.binding.identity.controllerId + 1 } });
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(trial.model().gameSession.kind).toBe("flight");
    expect(trial.effects).toEqual([]);
    trial.facade.executeOperation("retry");
    trial.dispatch({ type: "game-session-synced", ...trial.facade.readGameSessionProjection() });
    const model = trial.model();
    trial.dispatch({ type: "flight-controller-result-feedback", identity: trial.binding.identity, message: "Late terminal failure" });
    expect(trial.model()).toBe(model);
  });

  it("synchronizes a natural WaterContact finalization once without changing its fractional stamp or score", () => {
    const trial = fixture();
    trial.controller.onFrame(0);
    for (let tick = 1; tick <= 20_000 && trial.facade.readLifecycle().phaseCode === 5; tick += 1) {
      trial.controller.onFrame(tick * 10);
    }
    const terminal: TailControllerSnapshot = trial.controller.currentSnapshot;
    if (terminal.phaseCode !== 7) throw new Error("Expected a bounded Rust flight to reach Result");
    expect(terminal.frame.finalization.reason).toBe("water_contact");
    expect(trial.model().gameSession).toMatchObject({ kind: "result" });
    const stored: unknown = JSON.parse(trial.facade.exportRecordJson());
    expect(stored).toMatchObject({ finalization: { reason: terminal.frame.finalization.reason,
      terminal_tick: terminal.frame.finalization.terminalTick, terminal_fraction: terminal.frame.finalization.terminalFraction } });
    expect(trial.messages.filter((message) => message.type === "tail-controller-terminal")).toHaveLength(1);
    expect(gameSessionSnapshot(trial.model().gameSession)).toEqual(trial.controller.currentDisplaySnapshot);
    expect(trial.effects.filter((effect) => effect.type === "persist-flight-record")).toHaveLength(1);
  });

  it("preserves a Rust aerodynamic failure cause when terminal HUD rendering also fails", () => {
    const trial = fixture();
    trial.input.readDemand.mockReturnValue({ controlLayout: "tail_incidence", noseUp: 1, turnRight: 1, pilotPositionCommand: { kind: "hold" } });
    trial.display.render.mockImplementation((snapshot) => {
      if (snapshot.kind === "tail_result") throw new Error("Injected failure Result HUD error");
    });
    trial.controller.onFrame(0);
    for (let tick = 1; tick <= 20_000 && trial.facade.readLifecycle().phaseCode === 5; tick += 1) trial.controller.onFrame(tick * 10);
    const terminal = trial.controller.currentSnapshot;
    if (terminal.phaseCode !== 7) throw new Error("Expected a bounded Rust flight to reach Result");
    expect(terminal.frame.finalization.reason).toBe("out_of_valid_envelope");
    expect(terminal.frame.finalization.failure).not.toBeNull();
    expect(trial.model().gameSession.kind).toBe("result");
    expect(gameSessionSnapshot(trial.model().gameSession)).toEqual(trial.controller.currentDisplaySnapshot);
    expect(trial.facade.readRecordSummary().context.finalization).toEqual(terminal.frame.finalization);
    expect(trial.model().status).toContain("Injected failure Result HUD error");
    expect(trial.effects.filter((effect) => effect.type === "persist-flight-record")).toHaveLength(1);
  });
});
