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
});
