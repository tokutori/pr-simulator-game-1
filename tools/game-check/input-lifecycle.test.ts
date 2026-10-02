import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserPilotInput } from "../../web/src/game/browser-input.js";
import { FlightController } from "../../web/src/game/flight-controller.js";
import { FLIGHT_SNAPSHOT_LENGTH } from "../../web/src/game/flight-snapshot.js";
import type { PilotIntent } from "../../web/src/game/keyboard-intent.js";

afterEach(() => vi.unstubAllGlobals());

function snapshot(pilotPosition = 0, terminal = 0): number[] {
  const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
  values[7] = 1;
  values[11] = pilotPosition;
  values[16] = terminal;
  values[19] = terminal === 1 ? 0.5 : -1;
  return values;
}

function fixture() {
  const window = new BrowserWindow();
  vi.stubGlobal("HTMLElement", window.HTMLElement);
  const input = new BrowserPilotInput(window as unknown as Window);
  const intents: PilotIntent[] = [];
  let nextTerminal = 0;
  let failNextTick = false;
  let gamepadPolls = 0;
  const controller = new FlightController({
    snapshot: () => snapshot(),
    advance_tick(roll, pitch, yaw, pilotPositionMeters) {
      if (failNextTick) throw new Error("tick failed");
      intents.push({ roll, pitch, yaw, pilotPositionMeters });
      return snapshot(pilotPositionMeters / 2, nextTerminal);
    },
    free() {}
  }, input, { setFlightPose() {} }, {
    render() {}, fail() {}, setVisible() {}
  }, 100, () => { gamepadPolls += 1; return []; });
  function key(code: string, repeat = false): boolean {
    const event = new window.KeyboardEvent("keydown", { code, repeat, cancelable: true });
    window.dispatchEvent(event);
    return event.defaultPrevented;
  }
  function release(code: string): void {
    window.dispatchEvent(new window.KeyboardEvent("keyup", { code }));
  }
  return {
    input, controller, intents, key, release,
    endNextTick: () => { nextTerminal = 1; },
    clearTerminal: () => { nextTerminal = 0; },
    failNextTick: () => { failNextTick = true; },
    polls: () => gamepadPolls
  };
}

describe("pilot input lifecycle boundaries", () => {
  it("resets the previous flight target to each new snapshot initial position", () => {
    const trial = fixture();
    trial.key("KeyL");
    trial.controller.onFrame(0);
    trial.controller.onFrame(500);
    expect(trial.intents.at(-1)?.pilotPositionMeters).toBeCloseTo(0.2);
    trial.release("KeyL");
    trial.endNextTick();
    trial.controller.onFrame(510);
    trial.clearTerminal();
    trial.controller.reset(snapshot(0.12));
    trial.controller.onFrame(1000);
    trial.controller.onFrame(1010);
    trial.controller.onFrame(1020);
    expect(trial.intents.at(-1)?.pilotPositionMeters).toBe(0.12);
    trial.controller.reset(snapshot());
    trial.controller.onFrame(2000);
    trial.controller.onFrame(2010);
    expect(trial.intents.at(-1)?.pilotPositionMeters).toBe(0);
    trial.controller.dispose();
  });

  it("preserves the commanded target on pause and discards overlay presses and key repeats", () => {
    const trial = fixture();
    trial.key("KeyL");
    trial.controller.onFrame(0);
    trial.controller.onFrame(100);
    const target = trial.intents.at(-1)?.pilotPositionMeters;
    expect(trial.controller.currentSnapshot.pilotPositionMeters).not.toBe(target);
    const polls = trial.polls();
    trial.controller.suspend();
    expect(trial.key("KeyD")).toBe(false);
    expect(trial.key("KeyL", true)).toBe(false);
    trial.controller.onFrame(10000);
    expect(trial.polls()).toBe(polls);
    trial.controller.resume();
    expect(trial.key("KeyL", true)).toBe(true);
    trial.controller.onFrame(20000);
    trial.controller.onFrame(20010);
    expect(trial.intents.at(-1)).toEqual({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: target });
    trial.release("KeyL");
    expect(trial.key("KeyL")).toBe(true);
    trial.controller.onFrame(20020);
    expect(trial.intents.at(-1)?.pilotPositionMeters).toBeGreaterThan(target ?? 0);
    trial.controller.dispose();
  });

  it("requires neutral gamepad confirmation again after resume and retry", () => {
    const trial = fixture();
    const neutral = { index: 0, connected: true, axes: [0, 0, 0, 0] } as unknown as Gamepad;
    const active = { index: 0, connected: true, axes: [0.54, 0, 0, 0.5] } as unknown as Gamepad;
    trial.input.readIntent([neutral]);
    const command = trial.input.readIntent([active]);
    expect(command.roll).toBe(0.5);
    trial.controller.suspend();
    trial.controller.resume();
    expect(trial.input.readIntent([active])).toEqual({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: command.pilotPositionMeters });
    trial.input.readIntent([neutral]);
    expect(trial.input.readIntent([active]).roll).toBe(0.5);
    trial.controller.reset(snapshot());
    expect(trial.input.readIntent([active])).toEqual({ roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 });
    trial.controller.dispose();
  });

  it("prevents arrow-key repeat scrolling without restoring discarded presses", () => {
    const trial = fixture();
    expect(trial.key("ArrowUp")).toBe(true);
    expect(trial.key("ArrowUp", true)).toBe(true);
    expect(trial.input.readIntent([]).pitch).toBe(1);
    trial.controller.suspend();
    trial.controller.resume();
    expect(trial.key("ArrowUp", true)).toBe(true);
    expect(trial.input.readIntent([]).pitch).toBe(0);
    trial.controller.dispose();
  });

  it("does not capture Result keys or re-enable input on terminal reset", () => {
    const trial = fixture();
    trial.endNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    expect(trial.key("KeyL")).toBe(false);
    trial.controller.reset(snapshot(0.1, 4));
    trial.controller.resume();
    expect(trial.key("KeyD")).toBe(false);
    const polls = trial.polls();
    trial.controller.onFrame(100);
    expect(trial.polls()).toBe(polls);
    trial.controller.dispose();
  });

  it("deactivates input after a failed tick and allows a clean new flight", () => {
    const trial = fixture();
    trial.failNextTick();
    trial.controller.onFrame(0);
    trial.controller.onFrame(10);
    trial.controller.resume();
    expect(trial.key("KeyL")).toBe(false);
    trial.controller.reset(snapshot());
    expect(trial.key("KeyL")).toBe(true);
    trial.controller.dispose();
  });
});
