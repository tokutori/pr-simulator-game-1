import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createInitialAppModel, gameSessionSnapshot, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppMessage } from "../../web/src/app/app-state.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import type { TailControllerSnapshot } from "../../web/src/game/tail-flight-controller.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const disposals: (() => void)[] = [];
afterEach(() => { for (const dispose of disposals.splice(0)) dispose(); });

function fixture() {
  const facade = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  facade.executeSharedSessionCommand({ kind: "open_setup" });
  facade.executeSharedSessionCommand({ kind: "prepare" });
  facade.executeSharedSessionCommand({ kind: "start_countdown" });
  facade.observeCountdown(facade.captureCountdownToken(), 3, true);
  let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...facade.readGameSessionProjection() }).model;
  const effects: AppEffect[] = [];
  const messages: AppMessage[] = [];
  const dispatch = (message: AppMessage): void => {
    messages.push(message);
    const next = updateApp(model, message);
    model = next.model;
    effects.push(...next.effects);
  };
  let abortNext = false;
  const port = { ...facade.flightPort, advanceFrame: vi.fn<typeof facade.flightPort.advanceFrame>((timestamp, gamepads, available) => {
    const report = facade.flightPort.advanceFrame(timestamp, gamepads, available);
    if (!abortNext || report.advance.kind !== "ticks" || report.advance.count === 0) return report;
    abortNext = false;
    facade.executeSharedSessionCommand({ kind: "abort" });
    const snapshot = facade.readSnapshot();
    return { ...report, snapshot, render_pose: snapshot.render_pose, phase_changed: true };
  }) };
  const input = { readGamepads: () => [], dispose: vi.fn() };
  const renderer = { setFlightPose: vi.fn<(pose: FlightRenderPose | null) => void>() };
  const display = { render: vi.fn<(snapshot: TailControllerSnapshot) => void>(), setVisible: vi.fn<(visible: boolean) => void>() };
  const bindings = new FlightControllerUiBindings();
  const binding = bindings.bindAppDisplay(port, display, () => facade.readGameSessionProjection(), () => controller.currentSnapshot, dispatch);
  const controller = new TailFlightController(port, input, renderer, binding.port, binding.onTerminal);
  dispatch({ type: "flight-controller-ready", identity: binding.identity });
  disposals.push(() => { controller.dispose(); });
  const abortAtNextTick = (): void => { abortNext = true; };
  return { facade, controller, port, input, renderer, display, bindings, binding, effects, messages, dispatch, abortAtNextTick, model: () => model };
}

describe("authoritative Result synchronization across tail adapter failures", () => {
  it("commits the new Retry terminal callback with the new controller binding", () => {
    const trial = fixture();
    trial.abortAtNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    const previousResult = trial.facade.readGameSessionProjection();
    if (previousResult.phase !== "result") throw new Error("Expected the first Rust Result");
    trial.facade.executeSharedSessionCommand({ kind: "retry" });
    trial.facade.executeSharedSessionCommand({ kind: "start_countdown" });
    trial.facade.observeCountdown(trial.facade.captureCountdownToken(), 3, true);
    trial.dispatch({ type: "game-session-synced", ...trial.facade.readGameSessionProjection() });
    const binding = trial.bindings.bindAppDisplay(trial.port, trial.display, () => trial.facade.readGameSessionProjection(),
      () => trial.controller.currentSnapshot, trial.dispatch);
    trial.controller.reset(trial.port.readSnapshot(), binding.port, binding.onTerminal);
    trial.dispatch({ type: "flight-controller-ready", identity: binding.identity });
    const current = trial.model();
    trial.dispatch({ type: "tail-controller-terminal", identity: trial.binding.identity, projection: previousResult });
    expect(trial.model()).toBe(current);
    trial.abortAtNextTick();
    trial.controller.onFrame(20);
    trial.controller.onFrame(30);
    expect(trial.model().gameSession.phase).toBe("result");
    expect(trial.model().flightExecution).toMatchObject({ kind: "ready", identity: binding.identity });
    expect(trial.effects.filter((effect) => effect.type === "persist-flight-record")).toHaveLength(2);
  });

  it("retains the previous binding and callback after a failed reset", () => {
    const trial = fixture();
    const previous = trial.controller.currentSnapshot;
    const callback = vi.fn();
    const brokenHud = { render: () => { throw new Error("Retry HUD reset failed"); }, fail: vi.fn(), setVisible: vi.fn() };
    expect(() => { trial.controller.reset(trial.port.readSnapshot(), brokenHud, callback); }).toThrow("Retry HUD reset failed");
    expect(trial.controller.currentSnapshot.frame).toEqual(previous.frame);
    expect(trial.facade.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: false });
    trial.facade.executeSharedSessionCommand({ kind: "abort" });
    trial.facade.executeSharedSessionCommand({ kind: "retry" });
    trial.facade.executeSharedSessionCommand({ kind: "start_countdown" });
    trial.facade.observeCountdown(trial.facade.captureCountdownToken(), 3, true);
    trial.controller.reset(trial.port.readSnapshot());
    trial.abortAtNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(callback).not.toHaveBeenCalled();
    expect(trial.model().gameSession.phase).toBe("result");
    expect(trial.messages.filter((message) => message.type === "tail-controller-terminal")).toHaveLength(1);
  });

  it.each(["pose", "hud"] as const)("retains Rust phase, record and finalization when %s throws", (site) => {
    const trial = fixture();
    trial.abortAtNextTick();
    const primary = new Error(`Injected ${site} terminal failure`);
    if (site === "pose") trial.renderer.setFlightPose.mockImplementation(pose => {
      if (pose !== null && trial.facade.readLifecycle().phase === "result") throw primary;
    });
    if (site === "hud") trial.display.render.mockImplementation(snapshot => {
      if (snapshot.phase === "result") throw primary;
    });
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    const result = trial.facade.readGameSessionProjection();
    if (result.phase !== "result" || result.display.kind !== "available") throw new Error("Expected the original Rust Result");
    expect(trial.model().gameSession).toMatchObject({ phase: "result" });
    expect(gameSessionSnapshot(trial.model().gameSession)).toEqual(result.display.value);
    expect(trial.model().flightExecution.kind).toBe("ready");
    expect(trial.model().status).toContain(primary.message);
    expect(trial.effects).toEqual([{ type: "persist-flight-record" }]);
    const archive: unknown = JSON.parse(trial.facade.exportRecordJson());
    expect(archive).toMatchObject({ schema_version: 7, finalization: { reason: "manual_abort", terminal_tick: 1, failure: null } });
    const snapshot = trial.controller.currentSnapshot;
    expect(snapshot.phase).toBe("result");
    trial.controller.onFrame(2_000);
    expect(trial.port.advanceFrame).toHaveBeenCalledTimes(2);
    expect(trial.messages.filter((message) => message.type === "flight-controller-stopped")).toEqual([]);
    expect(trial.messages.filter((message) => message.type === "tail-controller-terminal")).toHaveLength(2);
  });

  it("retains a nonterminal history snapshot while the authoritative Rust phase is paused", () => {
    const trial = fixture();
    const retained = trial.controller.currentSnapshot;
    trial.facade.executeSharedSessionCommand({ kind: "pause" });
    trial.dispatch({ type: "game-session-synced", ...trial.facade.readGameSessionProjection() });
    trial.binding.port.fail("Pause presentation failed");
    expect(trial.model().gameSession).toMatchObject({ phase: "flight_paused", display: { value: { phase: "flight_paused" } } });
    expect(trial.model().flightExecution).toMatchObject({ kind: "stopped", snapshot: retained });
  });

  it("rejects terminal callbacks from an old controller and feedback after leaving Result", () => {
    const trial = fixture();
    trial.abortAtNextTick();
    trial.dispatch({ type: "flight-controller-ready", identity: { ...trial.binding.identity, controllerId: trial.binding.identity.controllerId + 1 } });
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(trial.model().gameSession.phase).toBe("flight_running");
    expect(trial.effects).toEqual([]);
    trial.facade.executeSharedSessionCommand({ kind: "retry" });
    trial.dispatch({ type: "game-session-synced", ...trial.facade.readGameSessionProjection() });
    const model = trial.model();
    trial.dispatch({ type: "flight-controller-result-feedback", identity: trial.binding.identity, message: "Late terminal failure" });
    expect(trial.model()).toBe(model);
  });

  it("synchronizes a natural WaterContact finalization once without changing its fractional stamp or score", () => {
    const trial = fixture();
    trial.controller.onFrame(0);
    for (let tick = 1; tick <= 20_000 && trial.facade.readLifecycle().phase === "flight_running"; tick += 1) {
      trial.controller.onFrame(tick * 10);
    }
    const terminal: TailControllerSnapshot = trial.controller.currentSnapshot;
    if (terminal.phase !== "result" || terminal.frame.kind !== "result") throw new Error("Expected a bounded Rust flight to reach Result");
    expect(terminal.frame.finalization.reason).toBe("water_contact");
    expect(trial.model().gameSession).toMatchObject({ phase: "result" });
    const stored: unknown = JSON.parse(trial.facade.exportRecordJson());
    expect(stored).toMatchObject({ finalization: { reason: terminal.frame.finalization.reason,
      terminal_tick: terminal.frame.finalization.terminal_tick, terminal_fraction: terminal.frame.finalization.terminal_fraction } });
    expect(trial.messages.filter((message) => message.type === "tail-controller-terminal")).toHaveLength(1);
    expect(gameSessionSnapshot(trial.model().gameSession)).toEqual(trial.controller.currentSnapshot);
    expect(trial.effects.filter((effect) => effect.type === "persist-flight-record")).toHaveLength(1);
  });

  it("preserves a Rust aerodynamic failure cause when terminal HUD rendering also fails", () => {
    const trial = fixture();
    trial.port.observeFlightKey({ control: "nose_up", pressed: true, repeat: false });
    trial.port.observeFlightKey({ control: "turn_right", pressed: true, repeat: false });
    trial.display.render.mockImplementation((snapshot) => {
      if (snapshot.phase === "result") throw new Error("Injected failure Result HUD error");
    });
    trial.controller.onFrame(0);
    for (let tick = 1; tick <= 20_000 && trial.facade.readLifecycle().phase === "flight_running"; tick += 1) trial.controller.onFrame(tick * 10);
    const terminal = trial.controller.currentSnapshot;
    if (terminal.phase !== "result" || terminal.frame.kind !== "result") throw new Error("Expected a bounded Rust flight to reach Result");
    expect(terminal.frame.finalization.reason).toBe("out_of_valid_envelope");
    expect(terminal.frame.finalization.failure).not.toBeNull();
    expect(trial.model().gameSession.phase).toBe("result");
    expect(gameSessionSnapshot(trial.model().gameSession)).toEqual(trial.controller.currentSnapshot);
    expect(trial.facade.readRecordSummary().context.finalization.value).toEqual(terminal.frame.finalization);
    expect(trial.model().status).toContain("Injected failure Result HUD error");
    expect(trial.effects.filter((effect) => effect.type === "persist-flight-record")).toHaveLength(1);
  });
});
