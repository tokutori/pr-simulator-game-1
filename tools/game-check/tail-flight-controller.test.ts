import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import type { TailControllerSnapshot, TailResultSnapshot } from "../../web/src/game/tail-flight-controller.js";
import type { FlightGamepadObservation } from "../../web/src/generated/flight-gamepad.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const disposals: (() => void)[] = [];
afterEach(() => { for (const dispose of disposals.splice(0)) dispose(); });

function fixture(mode = 0) {
  const session = new HybridGameSessionBridge(mode, 11, 12);
  const facade = new TailAppSessionFacade(session, physics_hz());
  facade.executeSharedSessionCommand({ kind: "open_setup" });
  facade.executeSharedSessionCommand({ kind: "prepare" });
  const launch = (): void => {
    facade.executeSharedSessionCommand({ kind: "start_countdown" });
    facade.observeCountdown(facade.captureCountdownToken(), 3, true);
  };
  launch();
  const port = { ...facade.flightPort, advanceFrame: vi.fn(facade.flightPort.advanceFrame.bind(facade.flightPort)),
    failAdapter: vi.fn(facade.flightPort.failAdapter.bind(facade.flightPort)), free: vi.fn(facade.flightPort.free.bind(facade.flightPort)) };
  const input = { readGamepads: vi.fn<() => readonly FlightGamepadObservation[]>(() => []), dispose: vi.fn() };
  const renderer = { setFlightPose: vi.fn<(pose: FlightRenderPose | null) => void>() };
  const hud = { render: vi.fn<(snapshot: TailControllerSnapshot) => void>(), fail: vi.fn<(message: string) => void>(),
    setVisible: vi.fn<(visible: boolean) => void>() };
  const onTerminal = vi.fn<(snapshot: TailResultSnapshot) => void>();
  const onPauseChanged = vi.fn();
  const controller = new TailFlightController(port, input, renderer, hud, onTerminal, onPauseChanged);
  const retry = (): void => {
    if (facade.readLifecycle().phase !== "result") facade.executeSharedSessionCommand({ kind: "abort" });
    facade.executeSharedSessionCommand({ kind: "retry" });
    launch();
  };
  disposals.push(() => { controller.dispose(); });
  return { session, facade, port, input, renderer, hud, onTerminal, onPauseChanged, controller, retry };
}

describe("shared-owner flight display adapter", () => {
  it.each([0, 1, 2])("passes each frame once to Rust and displays the committed two-tail state in mode %s", mode => {
    const trial = fixture(mode);
    trial.port.observeFlightKey({ control: "nose_up", pressed: true, repeat: false });
    trial.port.observeFlightKey({ control: "turn_right", pressed: true, repeat: false });
    trial.port.observeFlightKey({ control: "pilot_increase", pressed: true, repeat: false });
    trial.controller.onFrame(1000);
    trial.controller.onFrame(1035);
    expect(trial.port.advanceFrame).toHaveBeenCalledTimes(2);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(3);
    expect(trial.controller.currentSnapshot.frame.state.pilot_position_target_normalized).toBeCloseTo(0.03, 12);
    expect(trial.hud.render).toHaveBeenLastCalledWith(trial.controller.currentSnapshot);
    expect(trial.renderer.setFlightPose.mock.lastCall?.[0]?.simulationTimeSeconds).toBe(0.025);
    expect(trial.controller.currentSnapshot.render_pose.simulationTimeSeconds).toBe(0.03);
    trial.controller.onFrame(1038);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(3);
    expect(trial.renderer.setFlightPose.mock.lastCall?.[0]?.simulationTimeSeconds).toBeCloseTo(0.028, 12);
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });

  it("projects the shared stall and visibility transitions while explicit Resume starts a new zero-time frame", () => {
    const trial = fixture();
    trial.controller.onFrame(1000);
    trial.controller.onFrame(1010);
    const before = trial.controller.currentSnapshot.frame;
    trial.controller.onFrame(1160.001, false);
    expect(trial.facade.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: false });
    expect(trial.controller.currentSnapshot.frame).toEqual(before);
    trial.controller.onFrame(1170.001, true);
    expect(trial.facade.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: true });
    expect(trial.onPauseChanged).toHaveBeenCalledTimes(2);
    trial.facade.executeSharedSessionCommand({ kind: "resume" });
    trial.controller.synchronizeSnapshot(trial.facade.readSnapshot());
    trial.controller.onFrame(9000);
    expect(trial.controller.currentSnapshot.frame).toEqual(before);
    trial.controller.onFrame(9010);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(2);
  });

  it.each(["input", "pose", "hud"] as const)("stops the shared owner and retains the last valid record after a fatal %s failure", site => {
    const trial = fixture();
    trial.controller.onFrame(0);
    if (site === "input") trial.input.readGamepads.mockImplementationOnce(() => { throw new Error("input failure"); });
    if (site === "pose") trial.renderer.setFlightPose.mockImplementationOnce(() => { throw new Error("pose failure"); });
    if (site === "hud") trial.hud.render.mockImplementationOnce(() => { throw new Error("hud failure"); });
    trial.controller.onFrame(10);
    expect(trial.facade.readLifecycle()).toMatchObject({ phase: "flight_paused", can_resume: false });
    expect(trial.hud.fail).toHaveBeenCalledWith(expect.stringContaining(`${site} failure`));
    const retained = trial.facade.readSnapshot().frame;
    const calls = trial.port.advanceFrame.mock.calls.length;
    trial.controller.onFrame(20);
    expect(trial.port.advanceFrame).toHaveBeenCalledTimes(calls);
    expect(trial.facade.readSnapshot().frame).toEqual(retained);
  });

  it("notifies one Rust Result before display feedback, including a terminal display exception", () => {
    const trial = fixture();
    trial.controller.onFrame(0);
    trial.port.advanceFrame.mockImplementationOnce((timestamp, gamepads, available) => {
      const report = trial.facade.flightPort.advanceFrame(timestamp, gamepads, available);
      trial.facade.executeSharedSessionCommand({ kind: "abort" });
      const snapshot = trial.facade.readSnapshot();
      return { ...report, phase_changed: true, snapshot, render_pose: snapshot.render_pose };
    });
    trial.renderer.setFlightPose.mockImplementationOnce(() => { throw new Error("terminal display"); });
    trial.controller.onFrame(10);
    expect(trial.onTerminal).toHaveBeenCalledOnce();
    expect(trial.onTerminal.mock.calls[0]?.[0].phase).toBe("result");
    expect(trial.facade.readLifecycle().phase).toBe("result");
    expect(trial.hud.fail).toHaveBeenCalledWith(expect.stringContaining("terminal display"));
    trial.controller.onFrame(20);
    expect(trial.onTerminal).toHaveBeenCalledOnce();
  });

  it.each(["reset", "dispose"] as const)("rejects stale frame rendering after a renderer reenters %s and throws", operation => {
    const trial = fixture();
    trial.controller.onFrame(0);
    trial.hud.render.mockClear();
    trial.renderer.setFlightPose.mockImplementationOnce(() => {
      if (operation === "dispose") trial.controller.dispose();
      else { trial.retry(); trial.controller.reset(trial.facade.readSnapshot()); }
      throw new Error("retired renderer");
    });
    trial.controller.onFrame(30);
    expect(trial.hud.fail).not.toHaveBeenCalled();
    if (operation === "dispose") expect(trial.hud.render).not.toHaveBeenCalled();
    else {
      expect(trial.hud.render).toHaveBeenCalledOnce();
      expect(trial.controller.currentSnapshot.frame.state.tick).toBe(0);
      trial.controller.onFrame(1000);
      trial.controller.onFrame(1010);
      expect(trial.controller.currentSnapshot.frame.state.tick).toBe(1);
    }
  });

  it("keeps the previous HUD binding when a prospective reset HUD fails", () => {
    const trial = fixture();
    trial.retry();
    const nextHud = { ...trial.hud, render: vi.fn(() => { throw new Error("prospective HUD"); }), fail: vi.fn() };
    expect(() => { trial.controller.reset(trial.facade.readSnapshot(), nextHud, vi.fn()); }).toThrow("prospective HUD");
    expect(nextHud.fail).not.toHaveBeenCalled();
    expect(trial.hud.fail).toHaveBeenCalledWith(expect.stringContaining("prospective HUD"));
    expect(trial.facade.readLifecycle().can_resume).toBe(false);
  });

  it.each(["input", "session", "pose", "hud"] as const)("attempts every cleanup once and retains the original %s cleanup exception", site => {
    const trial = fixture();
    const attempts: string[] = [];
    const failure = new Error(`cleanup ${site}`);
    const cleanup = (current: string): void => { attempts.push(current); if (current === site) throw failure; };
    trial.input.dispose.mockImplementation(() => { cleanup("input"); });
    trial.port.free.mockImplementation(() => { trial.facade.dispose(); cleanup("session"); });
    trial.renderer.setFlightPose.mockImplementation(pose => { if (pose === null) cleanup("pose"); });
    trial.hud.setVisible.mockImplementation(() => { cleanup("hud"); });
    let caught: unknown;
    try { trial.controller.dispose(); } catch (error: unknown) { caught = error; }
    expect(caught).toBe(failure);
    expect(attempts).toEqual(["input", "session", "pose", "hud"]);
    trial.controller.dispose();
    trial.controller.onFrame(1000);
    expect(attempts).toHaveLength(4);
  });

  it("aggregates cleanup causes in order and closes the display before cleanup reentrancy", () => {
    const trial = fixture();
    const failures = [new Error("input"), new Error("session"), new Error("pose"), new Error("hud")] as const;
    trial.input.dispose.mockImplementation(() => { trial.controller.dispose(); trial.controller.onFrame(0); throw failures[0]; });
    trial.port.free.mockImplementation(() => { trial.facade.dispose(); throw failures[1]; });
    trial.renderer.setFlightPose.mockImplementation(() => { throw failures[2]; });
    trial.hud.setVisible.mockImplementation(() => { throw failures[3]; });
    let caught: unknown;
    try { trial.controller.dispose(); } catch (error: unknown) { caught = error; }
    expect(caught).toBeInstanceOf(AggregateError);
    if (!(caught instanceof AggregateError)) throw new Error("Expected cleanup causes");
    expect(caught.errors).toEqual(failures);
    expect(trial.port.advanceFrame).not.toHaveBeenCalled();
    expect(() => { trial.controller.reset(trial.controller.currentSnapshot); }).toThrow("disposed");
  });
});
