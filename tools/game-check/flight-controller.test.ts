import { describe, expect, it, vi } from "vitest";
import { FlightController } from "../../web/src/game/flight-controller.js";
import { FLIGHT_SNAPSHOT_LENGTH } from "../../web/src/game/flight-snapshot.js";
import type { FlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";

describe("FlightController reset transaction", () => {
  it.each(["length", "nan", "infinity"] as const)("rejects malformed snapshot %s without reopening a failed controller", (invalid) => {
    const trial = fixture();
    trial.failDuringFlight();
    const retained = trial.controller.currentSnapshot;
    const suspends = trial.input.suspend.mock.calls.length;
    const values = snapshotValues(0.12);
    if (invalid === "length") values.pop();
    else values[11] = invalid === "nan" ? Number.NaN : Number.POSITIVE_INFINITY;

    expect(() => { trial.controller.reset(values); }).toThrow(RangeError);
    expect(trial.input.suspend).toHaveBeenCalledTimes(suspends + 1);
    expectStopped(trial, retained);
    expectRecovery(trial);
    trial.controller.dispose();
  });

  it.each(["input-reset", "input-resume", "input-suspend", "pose", "hud"] as const)(
    "keeps reset failure in %s closed and preserves the committed snapshot", (port) => {
      const trial = fixture();
      trial.failDuringFlight();
      const retained = trial.controller.currentSnapshot;
      const suspends = trial.input.suspend.mock.calls.length;
      const failure = new Error(`Injected ${port} failure`);
      const fail = (): never => { throw failure; };
      switch (port) {
        case "input-reset": trial.input.reset.mockImplementationOnce(fail); break;
        case "input-resume": trial.input.resume.mockImplementationOnce(fail); break;
        case "input-suspend": trial.input.suspend.mockImplementationOnce(fail); break;
        case "pose": trial.renderer.setFlightPose.mockImplementationOnce(fail); break;
        case "hud": trial.hud.render.mockImplementationOnce(fail); break;
      }

      expect(() => { trial.controller.reset(snapshotValues(0.12, 0, port === "input-suspend" ? 4 : 0)); }).toThrow(failure);
      expect(trial.input.suspend).toHaveBeenCalledTimes(suspends + (port === "input-suspend" ? 2 : 1));
      expectStopped(trial, retained);
      trial.controller.renderCurrentSnapshot();
      expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(expect.objectContaining({ initialPilotPositionMeters: 0.05 }));
      expectRecovery(trial);
      trial.controller.dispose();
    }
  );

  it.each(["snapshot", "input-reset", "input-resume", "pose", "hud"] as const)("also stops an active controller when reset fails at %s", (port) => {
    const trial = fixture();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    const retained = trial.controller.currentSnapshot;
    const fail = (): never => { throw new Error("Reset failed while active"); };
    switch (port) {
      case "snapshot": break;
      case "input-reset": trial.input.reset.mockImplementationOnce(fail); break;
      case "input-resume": trial.input.resume.mockImplementationOnce(fail); break;
      case "pose": trial.renderer.setFlightPose.mockImplementationOnce(fail); break;
      case "hud": trial.hud.render.mockImplementationOnce(fail); break;
    }
    expect(() => { trial.controller.reset(port === "snapshot" ? [] : snapshotValues(0.12)); }).toThrow();
    expect(trial.input.suspend).toHaveBeenCalledTimes(1);
    expectStopped(trial, retained);
    expectRecovery(trial);
    trial.controller.dispose();
  });

  it.each([new Error("Primary reset failure"), "non-Error reset failure"])("retains the primary failure when input cleanup also throws: %s", (primary: unknown) => {
    const trial = fixture();
    trial.failDuringFlight();
    const retained = trial.controller.currentSnapshot;
    const cleanup = new Error("Input cleanup failed");
    trial.hud.render.mockImplementationOnce(() => { throw primary; });
    trial.input.suspend.mockImplementationOnce(() => { throw cleanup; });
    let caught: unknown;
    try {
      trial.controller.reset(snapshotValues(0.12));
    } catch (error: unknown) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(AggregateError);
    if (!(caught instanceof AggregateError)) throw new Error("Expected both reset and cleanup failures");
    expect(caught.errors).toEqual([primary, cleanup]);
    expect(caught.cause).toBe(primary);
    expectStopped(trial, retained);
    expectRecovery(trial);
    trial.controller.dispose();
  });

  it("keeps the committed snapshot and execution gate closed during reset port calls", () => {
    const trial = fixture();
    const retained = trial.controller.currentSnapshot;
    trial.renderer.setFlightPose.mockImplementationOnce(() => {
      expect(trial.controller.currentSnapshot).toBe(retained);
      const resumes = trial.input.resume.mock.calls.length;
      trial.controller.resume();
      trial.controller.onFrame(100);
      trial.controller.onFrame(110);
      expect(trial.input.resume).toHaveBeenCalledTimes(resumes);
      expect(trial.session.advance_tick).not.toHaveBeenCalled();
      expect(trial.readGamepads).not.toHaveBeenCalled();
    });
    expectRecovery(trial);
    trial.controller.dispose();
  });

  it("clears terminal notification only on successful reset and reports each new terminal once", () => {
    const trial = fixture();
    trial.endNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    const terminal = trial.controller.currentSnapshot;
    expect(() => { trial.controller.reset([]); }).toThrow(RangeError);
    expectStopped(trial, terminal);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);

    trial.clearTerminal();
    expectRecovery(trial);
    trial.endNextTick();
    trial.controller.onFrame(10_020);
    trial.controller.onFrame(20_000);
    trial.controller.resume();
    trial.controller.onFrame(30_000);
    expect(trial.onTerminal).toHaveBeenCalledTimes(2);
    expect(trial.hud.fail).not.toHaveBeenCalled();
    trial.controller.dispose();
  });

  it("preserves terminal reset and idempotent disposal without resuming input", () => {
    const trial = fixture();
    const resumes = trial.input.resume.mock.calls.length;
    trial.controller.reset(snapshotValues(0.1, 9, 4));
    expect(trial.controller.currentSnapshot.terminal).toBe("manual-abort");
    expect(trial.input.reset).toHaveBeenLastCalledWith(0.1);
    expect(trial.input.resume).toHaveBeenCalledTimes(resumes);
    expectStopped(trial, trial.controller.currentSnapshot);
    expect(trial.onTerminal).not.toHaveBeenCalled();
    trial.controller.dispose();
    trial.controller.dispose();
    const resetCalls = trial.input.reset.mock.calls.length;
    expect(() => { trial.controller.reset(snapshotValues()); }).toThrow("Cannot reset a disposed flight controller");
    trial.controller.onFrame(10_000);
    trial.controller.resume();
    trial.controller.renderCurrentSnapshot();
    expect(trial.input.reset).toHaveBeenCalledTimes(resetCalls);
    expect(trial.session.free).toHaveBeenCalledTimes(1);
    expect(trial.input.dispose).toHaveBeenCalledTimes(1);
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
    expect(trial.hud.setVisible).toHaveBeenLastCalledWith(false);
    expect(trial.session.advance_tick).not.toHaveBeenCalled();
  });

  it.each(["pose", "hud", "input-suspend"] as const)("delivers terminal once when %s fails", (port) => {
    const trial = fixture();
    const failure = new Error(`Terminal ${port} failed`);
    const fail = (): never => { throw failure; };
    trial.endNextTick();
    if (port === "pose") trial.renderer.setFlightPose.mockImplementationOnce(fail);
    if (port === "hud") trial.hud.render.mockImplementationOnce(fail);
    if (port === "input-suspend") trial.input.suspend.mockImplementationOnce(fail);
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    const terminal = trial.controller.currentSnapshot;
    expect(terminal.terminal).toBe("water-contact");
    expect(trial.onTerminal).toHaveBeenCalledExactlyOnceWith(terminal);
    expect(trial.hud.fail).toHaveBeenCalledWith(failure.message);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(1);
    expectStopped(trial, terminal);
    trial.controller.dispose();
  });

  it("does not resume catch-up or apply old cleanup after a terminal callback resets the controller", () => {
    const trial = fixture();
    trial.endNextTick();
    trial.onTerminal.mockImplementationOnce(() => {
      trial.clearTerminal();
      trial.controller.reset(snapshotValues(0.12));
    });
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(1);
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(expect.objectContaining({ pilotPositionMeters: 0.12 }));
    trial.controller.onFrame(110);
    trial.controller.onFrame(120);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(2);
    expect(trial.hud.fail).not.toHaveBeenCalled();
    trial.controller.dispose();
  });

  it("does not apply terminal presentation or input cleanup after callback disposal", () => {
    const trial = fixture();
    trial.endNextTick();
    trial.onTerminal.mockImplementationOnce(() => { trial.controller.dispose(); });
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
    expect(trial.session.free).toHaveBeenCalledTimes(1);
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });

  it("attempts terminal delivery once even when the callback throws", () => {
    const trial = fixture();
    trial.endNextTick();
    trial.onTerminal.mockImplementationOnce(() => { throw new Error("Terminal observer failed"); });
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.input.suspend).toHaveBeenCalled();
    expect(trial.hud.fail).toHaveBeenCalledWith("Terminal observer failed");
    expectStopped(trial, trial.controller.currentSnapshot);
    trial.controller.dispose();
  });

  it("preserves the terminal presentation failure when reporting also throws", () => {
    const trial = fixture();
    const primary = new Error("Terminal renderer failed");
    const diagnostic = new Error("Failure reporting failed");
    trial.endNextTick();
    trial.renderer.setFlightPose.mockImplementationOnce(() => { throw primary; });
    trial.hud.fail.mockImplementationOnce(() => { throw diagnostic; });
    trial.controller.onFrame(0);
    let caught: unknown;
    try {
      trial.controller.onFrame(100);
    } catch (error: unknown) {
      caught = error;
    }
    if (!(caught instanceof AggregateError)) throw new Error("Expected primary and diagnostic failures");
    expect(caught.errors).toEqual([primary, diagnostic]);
    expect(caught.cause).toBe(primary);
    expect(trial.onTerminal).toHaveBeenCalledExactlyOnceWith(trial.controller.currentSnapshot);
    expectStopped(trial, trial.controller.currentSnapshot);
    trial.controller.dispose();
  });

  it("does not deliver old cleanup diagnostics to a replacement HUD after cleanup resets the controller", () => {
    const trial = fixture();
    const replacementHud = { render: vi.fn(), fail: vi.fn(), setVisible: vi.fn() };
    trial.endNextTick();
    trial.input.suspend.mockImplementationOnce(() => { throw new Error("Terminal input cleanup failed"); })
      .mockImplementationOnce(() => {
        trial.clearTerminal();
        trial.controller.reset(snapshotValues(0.12), replacementHud);
      });
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.controller.currentSnapshot.terminal).toBe("airborne");
    expect(trial.hud.fail).not.toHaveBeenCalled();
    expect(replacementHud.fail).not.toHaveBeenCalled();
    trial.controller.onFrame(110);
    trial.controller.onFrame(120);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(2);
    expect(replacementHud.render).toHaveBeenLastCalledWith(trial.controller.currentSnapshot);
    trial.controller.dispose();
  });

  it("preserves catch-up ticks while presenting the latest live snapshot after clock bookkeeping", () => {
    const trial = fixture();
    trial.renderer.setFlightPose.mockClear();
    trial.hud.render.mockClear();
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(10);
    expect(trial.input.readIntent).toHaveBeenCalledTimes(10);
    expect(trial.controller.currentSnapshot.tick).toBe(10);
    expect(trial.renderer.setFlightPose).toHaveBeenCalledTimes(1);
    expect(trial.hud.render).toHaveBeenCalledExactlyOnceWith(trial.controller.currentSnapshot);
    trial.controller.dispose();
  });

  it.each(["pose", "hud"] as const)("preserves the new execution clock after live %s resets the controller", (port) => {
    const trial = fixture();
    const replacementHud = { render: vi.fn(), fail: vi.fn(), setVisible: vi.fn() };
    const reset = (): void => { trial.controller.reset(snapshotValues(0.12), replacementHud); };
    trial.hud.render.mockClear();
    if (port === "pose") trial.renderer.setFlightPose.mockImplementationOnce(reset);
    else trial.hud.render.mockImplementationOnce(reset);
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(10);
    expect(trial.controller.currentSnapshot.pilotPositionMeters).toBe(0.12);
    expect(trial.hud.render).toHaveBeenCalledTimes(port === "pose" ? 0 : 1);
    expect(replacementHud.render).toHaveBeenCalledExactlyOnceWith(trial.controller.currentSnapshot);
    expect(trial.input.suspend).not.toHaveBeenCalled();
    trial.controller.onFrame(110);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(10);
    trial.controller.onFrame(120);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(11);
    expect(replacementHud.render).toHaveBeenLastCalledWith(trial.controller.currentSnapshot);
    expect(trial.hud.fail).not.toHaveBeenCalled();
    expect(replacementHud.fail).not.toHaveBeenCalled();
    trial.controller.dispose();
  });

  it.each(["pose", "hud"] as const)("stops the old frame when live %s disposes the controller", (port) => {
    const trial = fixture();
    const dispose = (): void => { trial.controller.dispose(); };
    trial.hud.render.mockClear();
    if (port === "pose") trial.renderer.setFlightPose.mockImplementationOnce(dispose);
    else trial.hud.render.mockImplementationOnce(dispose);
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(10);
    expect(trial.hud.render).toHaveBeenCalledTimes(port === "pose" ? 0 : 1);
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
    expect(trial.session.free).toHaveBeenCalledTimes(1);
    trial.controller.onFrame(110);
    trial.controller.onFrame(120);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(10);
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });
});

function fixture() {
  let pilotPosition = 0.05;
  let tick = 0;
  let terminal = 0;
  const input = {
    reset: vi.fn((initialPilotPositionMeters: number) => { pilotPosition = initialPilotPositionMeters; }),
    resume: vi.fn<() => void>(),
    suspend: vi.fn<() => void>(),
    dispose: vi.fn<() => void>(),
    readIntent: vi.fn(() => ({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: pilotPosition }))
  };
  const session = {
    snapshot: () => snapshotValues(pilotPosition),
    advance_tick: vi.fn((_roll: number, _pitch: number, _yaw: number, command: number) => {
      tick++;
      return snapshotValues(command, tick, terminal);
    }),
    free: vi.fn<() => void>()
  };
  const renderer = { setFlightPose: vi.fn<(pose: FlightRenderPose | null) => void>() };
  const hud = {
    render: vi.fn<(snapshot: FlightSnapshot) => void>(),
    fail: vi.fn<(message: string) => void>(),
    setVisible: vi.fn<(visible: boolean) => void>()
  };
  const readGamepads = vi.fn(() => []);
  const onTerminal = vi.fn<(snapshot: FlightSnapshot) => void>();
  const controller = new FlightController(session, input, renderer, hud, 100, readGamepads, onTerminal);
  return {
    controller, input, session, renderer, hud, readGamepads, onTerminal,
    endNextTick: () => { terminal = 1; },
    clearTerminal: () => { terminal = 0; },
    failDuringFlight: () => {
      controller.onFrame(0);
      controller.onFrame(10);
      input.readIntent.mockImplementationOnce(() => { throw new Error("Temporary input read failure"); });
      controller.onFrame(20);
      expect(hud.fail).toHaveBeenLastCalledWith("Temporary input read failure");
    }
  };
}

function expectStopped(trial: ReturnType<typeof fixture>, retained: FlightSnapshot): void {
  const ticks = trial.session.advance_tick.mock.calls.length;
  const polls = trial.readGamepads.mock.calls.length;
  const reads = trial.input.readIntent.mock.calls.length;
  const resumes = trial.input.resume.mock.calls.length;
  trial.controller.resume();
  trial.controller.onFrame(1_000);
  trial.controller.onFrame(1_010);
  trial.controller.onFrame(5_000);
  expect(trial.controller.currentSnapshot).toBe(retained);
  expect(trial.session.advance_tick).toHaveBeenCalledTimes(ticks);
  expect(trial.readGamepads).toHaveBeenCalledTimes(polls);
  expect(trial.input.readIntent).toHaveBeenCalledTimes(reads);
  expect(trial.input.resume).toHaveBeenCalledTimes(resumes);
}

function expectRecovery(trial: ReturnType<typeof fixture>): void {
  const ticks = trial.session.advance_tick.mock.calls.length;
  trial.controller.reset(snapshotValues(0.12));
  expect(trial.controller.currentSnapshot.pilotPositionMeters).toBe(0.12);
  expect(trial.input.reset).toHaveBeenLastCalledWith(0.12);
  expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(expect.objectContaining({ initialPilotPositionMeters: 0.12 }));
  trial.controller.onFrame(10_000);
  expect(trial.session.advance_tick).toHaveBeenCalledTimes(ticks);
  trial.controller.onFrame(10_010);
  expect(trial.session.advance_tick).toHaveBeenCalledTimes(ticks + 1);
  expect(trial.session.advance_tick).toHaveBeenLastCalledWith(0, 0, 0, 0.12);
}

function snapshotValues(pilotPosition = 0.05, tick = 0, terminal = 0): number[] {
  const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
  values[0] = tick;
  values[7] = 1;
  values[11] = pilotPosition;
  values[16] = terminal;
  values[19] = terminal === 1 ? 0.5 : -1;
  return values;
}
