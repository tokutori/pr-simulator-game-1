import { readFileSync } from "node:fs";
import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { BrowserTailPilotInput, DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION } from "../../web/src/game/browser-tail-input.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import type { TailResultSnapshot } from "../../web/src/game/tail-flight-controller.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  vi.unstubAllGlobals();
});
const hold = { controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" as const } };

function fixture(mode = 0, initialTarget = 0) {
  const browser = new BrowserWindow();
  vi.stubGlobal("HTMLElement", browser.HTMLElement);
  const session = new HybridGameSessionBridge(mode, 11, 12);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
  if (initialTarget !== 0) session.advance_tick_json(encodeTailLogicalInput({ ...hold, pilotPositionCommand: { kind: "set", normalized: initialTarget } }));
  const browserInput = new BrowserTailPilotInput(browser as unknown as Window, { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION,
    keyboard: { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION.keyboard, physicsHz: physics_hz() } });
  const input = { readDemand: vi.fn(browserInput.readDemand.bind(browserInput)), reset: vi.fn(browserInput.reset.bind(browserInput)),
    suspend: vi.fn(browserInput.suspend.bind(browserInput)), resume: vi.fn(browserInput.resume.bind(browserInput)), dispose: vi.fn(browserInput.dispose.bind(browserInput)) };
  const port = { snapshot_json: vi.fn(() => session.snapshot_json()), control_profile_json: vi.fn(() => session.control_profile_json()),
    advance_tick_json: vi.fn((json: string) => session.advance_tick_json(json)), free: vi.fn(() => { session.free(); }) };
  const renderer = { setFlightPose: vi.fn<(pose: FlightRenderPose | null) => void>() };
  const hud = { render: vi.fn<(snapshot: FlightDisplaySnapshot) => void>(), fail: vi.fn<(message: string) => void>(), setVisible: vi.fn<(visible: boolean) => void>() };
  const onTerminal = vi.fn<(snapshot: TailResultSnapshot) => void>();
  const controller = new TailFlightController(port, input, renderer, hud, physics_hz(), () => [], onTerminal);
  disposals.push(() => { controller.dispose(); });
  const key = (type: string, code: string) => browser.dispatchEvent(new browser.KeyboardEvent(type, { code, cancelable: true }));
  return { session, controller, port, input, renderer, hud, onTerminal, key };
}

type CleanupSite = "input" | "session" | "pose" | "hud";
const cleanupSites: readonly CleanupSite[] = ["input", "session", "pose", "hud"];

function observeCleanup(trial: ReturnType<typeof fixture>, onCleanup: (site: CleanupSite) => void): void {
  const disposeInput = trial.input.dispose.getMockImplementation();
  const freeSession = trial.port.free.getMockImplementation();
  if (disposeInput === undefined || freeSession === undefined) throw new Error("Expected real input and WASM cleanup implementations");
  trial.input.dispose.mockImplementation(() => { disposeInput(); onCleanup("input"); });
  trial.port.free.mockImplementation(() => { freeSession(); onCleanup("session"); });
  trial.renderer.setFlightPose.mockImplementation(() => { onCleanup("pose"); });
  trial.hud.setVisible.mockImplementation(() => { onCleanup("hud"); });
}

describe("two-tail controller through the Rust WASM port", () => {
  it.each([0, 1, 2])("connects keyboard intent, sealed profile and two physical tails in mode %s", (mode) => {
    const trial = fixture(mode);
    trial.key("keydown", "ArrowUp");
    trial.key("keydown", "ArrowRight");
    trial.key("keydown", "KeyL");
    trial.key("keydown", "KeyA");
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    const profile: unknown = JSON.parse(trial.session.control_profile_json());
    const limits = (profile as { desired_body_rate_limit_rad_s: { pitch: number; yaw: number } }).desired_body_rate_limit_rad_s;
    const payload = trial.port.advance_tick_json.mock.calls[0]?.[0];
    if (payload === undefined) throw new Error("Expected a Rust input document");
    expect(JSON.parse(payload)).toEqual({ schema_version: 2, control_layout: "tail_incidence", nose_up: 1, turn_right: 1,
      desired_pitch_rate_rad_s: limits.pitch, desired_yaw_rate_rad_s: limits.yaw, pilot_position_command: { kind: "set", normalized: 0.01 } });
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(1);
    expect(trial.controller.currentSnapshot.frame.state.physicalIncidence.horizontalTailRadians).toBeLessThan(0);
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(expect.objectContaining({ controls: { layout: "tail_incidence",
      physicalIncidence: trial.controller.currentSnapshot.frame.state.physicalIncidence } }));
    expect(trial.hud.render).toHaveBeenLastCalledWith(trial.controller.currentDisplaySnapshot);
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });

  it("initializes from the held target rather than current moving mass position and preserves it on Hold/resume", () => {
    const trial = fixture(0, 0.5);
    const initial = trial.controller.currentSnapshot.frame.state;
    expect(initial.pilotPositionMeters).not.toBe(initial.pilotPositionTargetMeters);
    expect(trial.input.reset).toHaveBeenCalledWith(initial.pilotPositionTargetNormalized);
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(trial.controller.currentSnapshot.frame.state.pilotPositionTargetNormalized).toBe(0.5);
    trial.session.pause(0);
    trial.controller.synchronizeSnapshot(trial.session.snapshot_json());
    trial.controller.resume();
    expect(trial.controller.currentSnapshot.phaseCode).toBe(6);
    const polls = trial.input.readDemand.mock.calls.length;
    trial.controller.onFrame(1_000);
    expect(trial.input.readDemand).toHaveBeenCalledTimes(polls);
    trial.session.clear_pause_reason(0);
    trial.session.resume();
    trial.controller.resume();
    trial.controller.onFrame(2_000);
    trial.controller.onFrame(2_010);
    expect(trial.controller.currentSnapshot.frame.state.pilotPositionTargetNormalized).toBe(0.5);
    for (const [json] of trial.port.advance_tick_json.mock.calls) {
      const decoded: unknown = JSON.parse(json);
      expect(decoded).toMatchObject({ pilot_position_command: { kind: "hold" } });
    }
  });

  it.each([30, 60, 120])("retains 100 Hz simulation independently of %s FPS frame delivery", (fps) => {
    const trial = fixture();
    trial.controller.onFrame(0);
    for (let frame = 1; frame <= fps / 5; frame += 1) trial.controller.onFrame(frame * 1_000 / fps);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(20);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(20);
  });

  it("stops at a Rust Result, preserves its snapshot/cause, and reports it once", () => {
    const trial = fixture();
    trial.port.advance_tick_json.mockImplementationOnce(() => trial.session.abort());
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(2_000);
    trial.controller.resume();
    const terminal = trial.controller.currentSnapshot;
    expect(terminal.phaseCode).toBe(7);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.onTerminal).toHaveBeenCalledWith(terminal);
    expect(trial.hud.render).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "tail_result" }));
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
    const saved = trial.session.export_flight_record_json();
    const archive: unknown = JSON.parse(saved);
    expect(archive).toMatchObject({ schema_version: 6, finalization: { reason: "manual_abort", terminal_tick: 0 } });
    trial.session.retry();
    trial.session.start_countdown(1);
    trial.session.advance_countdown();
    trial.controller.reset(trial.session.launch());
    expect(trial.controller.currentSnapshot.phaseCode).toBe(5);
    expect(trial.controller.currentSnapshot.frame.state.pilotPositionTargetNormalized).toBe(0);
  });

  it("rejects stale normalized/schema payloads and seals a failed reset while retaining the committed snapshot", () => {
    const trial = fixture(0, 0.5);
    const retained = trial.controller.currentSnapshot;
    const payload: unknown = JSON.parse(trial.session.snapshot_json());
    const document = payload as { frame: { state: Record<string, unknown> } };
    for (const invalid of [-1.01, 1.01, null]) {
      document.frame.state.pilot_position_target_normalized = invalid;
      expect(() => { trial.controller.reset(JSON.stringify(document)); }).toThrow(RangeError);
      trial.controller.resume();
      trial.controller.onFrame(100);
      trial.controller.onFrame(110);
      expect(trial.controller.currentSnapshot).toBe(retained);
      expect(trial.port.advance_tick_json).not.toHaveBeenCalled();
    }
    delete document.frame.state.pilot_position_target_normalized;
    expect(() => parseTailSessionSnapshot(JSON.stringify(document))).toThrow(RangeError);
    trial.controller.reset(trial.session.snapshot_json());
    expect(trial.input.reset).toHaveBeenLastCalledWith(0.5);
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(1_010);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
  });

  it("keeps frame/identity failures closed and disposes each resource once", () => {
    const trial = fixture();
    const other = new HybridGameSessionBridge(0, 99, 100);
    try {
      other.open_setup(); other.prepare(); other.mark_briefing_ready(); other.start_countdown(1); other.advance_countdown(); other.launch();
      const retained = trial.controller.currentSnapshot;
      trial.port.advance_tick_json.mockImplementationOnce(() => other.snapshot_json());
      trial.controller.onFrame(0);
      trial.controller.onFrame(10);
      expect(trial.controller.currentSnapshot).toBe(retained);
      expect(trial.hud.fail).toHaveBeenCalledWith("Tail control profile belongs to another sealed session");
      trial.controller.resume();
      trial.controller.onFrame(100);
      trial.controller.onFrame(110);
      expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
      trial.controller.dispose();
      trial.controller.dispose();
      trial.controller.onFrame(1_000);
      expect(trial.port.free).toHaveBeenCalledTimes(1);
      expect(trial.input.dispose).toHaveBeenCalledTimes(1);
      expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
    } finally {
      other.free();
    }
  });

  it.each(cleanupSites)("tries every independent cleanup once after a %s failure and retains the original exception", (site) => {
    const trial = fixture();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    const retained = trial.controller.currentSnapshot;
    const primary = new Error(`Injected ${site} cleanup failure`);
    const attempts: CleanupSite[] = [];
    observeCleanup(trial, (current) => {
      attempts.push(current);
      if (current === site) throw primary;
    });
    let caught: unknown;
    try {
      trial.controller.dispose();
    } catch (error: unknown) {
      caught = error;
    }
    expect(caught).toBe(primary);
    expect(attempts).toEqual(cleanupSites);
    expect(trial.input.dispose).toHaveBeenCalledTimes(1);
    expect(trial.port.free).toHaveBeenCalledTimes(1);
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
    expect(trial.hud.setVisible).toHaveBeenCalledWith(false);
    const queries = trial.port.snapshot_json.mock.calls.length;
    const renders = trial.renderer.setFlightPose.mock.calls.length;
    trial.controller.dispose();
    trial.controller.resume();
    trial.controller.suspend();
    trial.controller.synchronizeSnapshot("not a snapshot");
    trial.controller.renderCurrentSnapshot();
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(2_000);
    expect(() => { trial.controller.reset("not a snapshot"); }).toThrow("Cannot reset a disposed tail controller");
    expect(attempts).toEqual(cleanupSites);
    expect(trial.port.snapshot_json).toHaveBeenCalledTimes(queries);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
    expect(trial.input.readDemand).toHaveBeenCalledTimes(1);
    expect(trial.renderer.setFlightPose).toHaveBeenCalledTimes(renders);
    expect(trial.controller.currentSnapshot).toBe(retained);
    expect(trial.onTerminal).not.toHaveBeenCalled();
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });

  it("preserves an undefined throw value and still attempts the remaining cleanup", () => {
    const trial = fixture();
    const attempts: CleanupSite[] = [];
    const failure: unknown = undefined;
    observeCleanup(trial, (site) => {
      attempts.push(site);
      if (site === "input") throw failure;
    });
    let thrown = false;
    try {
      trial.controller.dispose();
    } catch (error: unknown) {
      thrown = true;
      expect(error).toBeUndefined();
    }
    expect(thrown).toBe(true);
    expect(attempts).toEqual(cleanupSites);
    trial.controller.dispose();
    expect(trial.port.free).toHaveBeenCalledTimes(1);
  });

  it("aggregates all cleanup failures in attempt order without converting their causes", () => {
    const trial = fixture();
    const failures: Readonly<Record<CleanupSite, unknown>> = { input: new Error("Input cleanup failed"), session: { resource: "session" }, pose: undefined, hud: "HUD cleanup failed" };
    const attempts: CleanupSite[] = [];
    observeCleanup(trial, (site) => { attempts.push(site); throw failures[site]; });
    let caught: unknown;
    try {
      trial.controller.dispose();
    } catch (error: unknown) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(AggregateError);
    if (!(caught instanceof AggregateError)) throw new Error("Expected all cleanup failures");
    expect(caught.message).toBe("Tail controller cleanup failed");
    expect(caught.cause).toBe(failures.input);
    expect(caught.errors).toEqual(cleanupSites.map((site) => failures[site]));
    expect(attempts).toEqual(cleanupSites);
    trial.controller.dispose();
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(attempts).toEqual(cleanupSites);
    expect(trial.input.dispose).toHaveBeenCalledTimes(1);
    expect(trial.port.free).toHaveBeenCalledTimes(1);
    expect(trial.port.advance_tick_json).not.toHaveBeenCalled();
  });

  it.each(cleanupSites)("prevents %s cleanup from reentrantly disposing or restarting the controller", (site) => {
    const trial = fixture();
    const attempts: CleanupSite[] = [];
    const queries = trial.port.snapshot_json.mock.calls.length;
    observeCleanup(trial, (current) => {
      attempts.push(current);
      if (current !== site) return;
      trial.controller.dispose();
      trial.controller.resume();
      trial.controller.renderCurrentSnapshot();
      trial.controller.onFrame(0);
      trial.controller.onFrame(100);
    });
    trial.controller.dispose();
    expect(attempts).toEqual(cleanupSites);
    expect(trial.input.dispose).toHaveBeenCalledTimes(1);
    expect(trial.port.free).toHaveBeenCalledTimes(1);
    expect(trial.port.snapshot_json).toHaveBeenCalledTimes(queries);
    expect(trial.port.advance_tick_json).not.toHaveBeenCalled();
    expect(trial.input.readDemand).not.toHaveBeenCalled();
    expect(trial.renderer.setFlightPose).toHaveBeenCalledTimes(2);
    expect(trial.hud.render).toHaveBeenCalledTimes(1);
    expect(trial.hud.setVisible).toHaveBeenCalledTimes(1);
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });

  it("completes cleanup failures during terminal callback disposal without reviving the old frame", () => {
    const trial = fixture();
    trial.port.advance_tick_json.mockImplementationOnce(() => trial.session.abort());
    const attempts: CleanupSite[] = [];
    observeCleanup(trial, (site) => {
      attempts.push(site);
      if (site === "input" || site === "pose") throw new Error(`Injected ${site} terminal cleanup failure`);
    });
    trial.onTerminal.mockImplementation(() => { trial.controller.dispose(); });
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(2_000);
    expect(attempts).toEqual(cleanupSites);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
    expect(trial.port.free).toHaveBeenCalledTimes(1);
    expect(trial.hud.render).toHaveBeenCalledTimes(1);
    expect(trial.hud.setVisible).toHaveBeenCalledWith(false);
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.hud.fail).not.toHaveBeenCalled();
  });

  it.each(["pose", "hud", "input"] as const)("preserves a committed snapshot and closes reset failure at %s", (site) => {
    const trial = fixture(0, 0.5);
    const retained = trial.controller.currentSnapshot;
    const primary = new Error(`Injected ${site} reset failure`);
    const fail = (): never => { throw primary; };
    if (site === "pose") trial.renderer.setFlightPose.mockImplementationOnce(fail);
    if (site === "hud") trial.hud.render.mockImplementationOnce(fail);
    if (site === "input") trial.input.reset.mockImplementationOnce(fail);
    expect(() => { trial.controller.reset(trial.session.snapshot_json()); }).toThrow(primary);
    trial.controller.resume();
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.controller.currentSnapshot).toBe(retained);
    expect(trial.port.advance_tick_json).not.toHaveBeenCalled();
    trial.controller.reset(trial.session.snapshot_json());
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(1_010);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(1);
  });

  it("closes failed synchronization and retains the original error if cleanup also fails", () => {
    const trial = fixture();
    const retained = trial.controller.currentSnapshot;
    const primary = new Error("Injected synchronization failure");
    const cleanup = new Error("Injected cleanup failure");
    trial.hud.render.mockImplementationOnce(() => { throw primary; });
    trial.input.suspend.mockImplementationOnce(() => { throw cleanup; });
    let caught: unknown;
    try {
      trial.controller.synchronizeSnapshot(trial.session.snapshot_json());
    } catch (error: unknown) {
      caught = error;
    }
    expect(caught).toBeInstanceOf(AggregateError);
    if (!(caught instanceof AggregateError)) throw new Error("Expected original and cleanup failures");
    expect(caught.cause).toBe(primary);
    expect(caught.errors).toEqual([primary, cleanup]);
    trial.controller.resume();
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    expect(trial.controller.currentSnapshot).toBe(retained);
    expect(trial.port.advance_tick_json).not.toHaveBeenCalled();
  });

  it.each([false, true])("finishes old clock bookkeeping before a terminal callback resets the controller (throws=%s)", (throws) => {
    const trial = fixture();
    trial.port.advance_tick_json.mockImplementationOnce(() => trial.session.abort());
    trial.onTerminal.mockImplementation(() => {
      trial.controller.onFrame(20_000);
      trial.session.retry();
      trial.session.start_countdown(1);
      trial.session.advance_countdown();
      trial.controller.reset(trial.session.launch());
      if (throws) throw new Error("Old terminal callback failure after reset");
    });
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.controller.currentSnapshot.phaseCode).toBe(5);
    expect(trial.hud.render.mock.lastCall?.[0]).toMatchObject({ kind: "tail_flight", stamp: { tick: 0, fraction: 0 } });
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.hud.fail).not.toHaveBeenCalled();
    trial.controller.onFrame(2_000);
    trial.controller.onFrame(2_010);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(1);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(2);
  });

  it.each([false, true])("suppresses old terminal rendering, cleanup and diagnostics after callback disposal (throws=%s)", (throws) => {
    const trial = fixture();
    trial.port.advance_tick_json.mockImplementationOnce(() => trial.session.abort());
    trial.onTerminal.mockImplementation(() => {
      trial.controller.dispose();
      if (throws) throw new Error("Old terminal callback failure after disposal");
    });
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    trial.controller.onFrame(2_000);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
    expect(trial.hud.render).toHaveBeenCalledTimes(1);
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.hud.fail).not.toHaveBeenCalled();
    expect(trial.port.free).toHaveBeenCalledTimes(1);
  });

  it.each(["pose", "hud"] as const)("suppresses terminal work after %s reentrantly resets the controller", (site) => {
    const trial = fixture();
    trial.port.advance_tick_json.mockImplementationOnce(() => trial.session.abort());
    const reset = (): void => {
      trial.session.retry();
      trial.session.start_countdown(1);
      trial.session.advance_countdown();
      trial.controller.reset(trial.session.launch());
    };
    if (site === "pose") trial.renderer.setFlightPose.mockImplementationOnce(reset);
    else trial.hud.render.mockImplementationOnce(reset);
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.hud.render.mock.lastCall?.[0]).toMatchObject({ kind: "tail_flight", stamp: { tick: 0 } });
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.hud.fail).not.toHaveBeenCalled();
    trial.controller.onFrame(2_000);
    trial.controller.onFrame(2_010);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(1);
  });

  it("finishes live clock work before a renderer resets the controller and rejects the old HUD update", () => {
    const trial = fixture();
    const restart = (): void => {
      trial.session.abort();
      trial.session.retry();
      trial.session.start_countdown(1);
      trial.session.advance_countdown();
      trial.controller.reset(trial.session.launch());
    };
    trial.renderer.setFlightPose.mockImplementationOnce(restart);
    trial.controller.onFrame(0);
    trial.controller.onFrame(30);
    expect(trial.port.advance_tick_json).toHaveBeenCalledTimes(3);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(0);
    expect(trial.hud.render).toHaveBeenCalledTimes(2);
    trial.controller.onFrame(100);
    trial.controller.onFrame(110);
    expect(trial.controller.currentSnapshot.frame.state.tick).toBe(1);
    expect(trial.input.suspend).not.toHaveBeenCalled();
  });

  it.each(["pose", "hud"] as const)("suppresses terminal work after %s reentrantly disposes the controller", (site) => {
    const trial = fixture();
    trial.port.advance_tick_json.mockImplementationOnce(() => trial.session.abort());
    const dispose = (): void => { trial.controller.dispose(); };
    if (site === "pose") trial.renderer.setFlightPose.mockImplementationOnce(dispose);
    else trial.hud.render.mockImplementationOnce(dispose);
    trial.controller.onFrame(0);
    trial.controller.onFrame(1_000);
    expect(trial.onTerminal).toHaveBeenCalledTimes(1);
    expect(trial.input.suspend).not.toHaveBeenCalled();
    expect(trial.hud.fail).not.toHaveBeenCalled();
    expect(trial.port.free).toHaveBeenCalledTimes(1);
    expect(trial.renderer.setFlightPose).toHaveBeenLastCalledWith(null);
  });
});
