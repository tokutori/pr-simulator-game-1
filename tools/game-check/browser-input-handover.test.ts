import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserPilotInput, DEFAULT_PILOT_INPUT_CONFIGURATION, gamepadIntent } from "../../web/src/game/browser-input.js";
import type { BrowserPilotInputConfiguration } from "../../web/src/game/browser-input.js";
import { FlightController } from "../../web/src/game/flight-controller.js";
import { FLIGHT_SNAPSHOT_LENGTH } from "../../web/src/game/flight-snapshot.js";
import type { PilotIntent } from "../../web/src/game/keyboard-intent.js";

const disposals: (() => void)[] = [];

afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  vi.unstubAllGlobals();
});

describe("gamepad pilot acquisition through flight ticks", () => {
  it("requires the next selected device to pass neutral without an all-disconnected poll", () => {
    const trial = fixture();
    trial.tick([pad(0), pad(1, 1, -1)]);
    const held = trial.tick([pad(0, 0.54, -0.54), pad(1, 1, -1)]).pilotPositionMeters;
    expect(trial.tick([null, pad(1, 1, -1)])).toEqual(heldIntent(held));
    expect(trial.tick([null, pad(1)])).toEqual(heldIntent(held));
    expect(trial.tick([null, pad(1, 0.54, -0.2)])).toEqual({ ...heldIntent(held), roll: 0.5 });
    expect(trial.tick([null, pad(1, 0.54, -0.8)])).toEqual({ ...heldIntent(held), roll: 0.5 });
    expect(trial.tick([null, pad(1, 0.54, -1)])).toEqual(mapped(pad(1, 0.54, -1)));
  });

  it("keeps the selected connection when a lower index connects and another device changes generation", () => {
    const trial = fixture();
    trial.tick([null, pad(1)]);
    trial.tick([null, pad(1, 0.54, -0.54)]);
    trial.connection("gamepadconnected", pad(0, 1, 1));
    expect(trial.tick([pad(0, 1, 1), pad(1, -0.54, -0.8)])).toEqual(mapped(pad(1, -0.54, -0.8)));
    trial.connection("gamepaddisconnected", pad(0));
    trial.connection("gamepadconnected", pad(0));
    expect(trial.tick([pad(0), pad(1, 0.54, -1)])).toEqual(mapped(pad(1, 0.54, -1)));
  });

  it("accepts fresh snapshot objects and input timestamps from the same connection", () => {
    const trial = fixture();
    trial.tick([pad(0)]);
    const first = pad(0, 0.54, -0.54, { timestamp: 12 });
    expect(trial.tick([first])).toEqual(mapped(first));
    const next = pad(0, -0.54, -0.8, { timestamp: 24 });
    expect(trial.tick([next])).toEqual(mapped(next));
  });

  it.each(["gamepadconnected", "gamepaddisconnected", "both"] as const)(
    "rearms same-index same-id devices after %s events between polls", (event) => {
      const trial = fixture();
      trial.tick([pad(0)]);
      const held = trial.tick([pad(0, 0.54, -0.54)]).pilotPositionMeters;
      if (event !== "gamepadconnected") trial.connection("gamepaddisconnected", pad(0));
      if (event !== "gamepaddisconnected") trial.connection("gamepadconnected", pad(0));
      expect(trial.tick([pad(0, 1, -1)])).toEqual(heldIntent(held));
      expect(trial.tick([pad(0)])).toEqual(heldIntent(held));
      expect(trial.tick([pad(0, 0.54, -1)])).toEqual({ ...heldIntent(held), roll: 0.5 });
      expect(trial.tick([pad(0, 0.54, -0.8)])).toEqual(mapped(pad(0, 0.54, -0.8)));
    }
  );

  it.each(["null", "disconnected"] as const)("rearms after a %s selected slot", (absence) => {
    const trial = fixture();
    trial.tick([pad(0)]);
    const held = trial.tick([pad(0, 0.54, -0.54)]).pilotPositionMeters;
    expect(trial.tick([absence === "null" ? null : pad(0, 0, 0, { connected: false })])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0, 1, -1)])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0)])).toEqual(heldIntent(held));
  });

  it.each([0.54, -0.54])("holds the crossing sample before restoring absolute mapping for axis %s", (axis) => {
    const held = mapped(pad(0, 0, axis)).pilotPositionMeters;
    const trial = fixture(held);
    expect(trial.tick([pad(0)])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0, 0.54, axis / 2)])).toEqual({ ...heldIntent(held), roll: 0.5 });
    expect(trial.tick([pad(0, -0.54, -axis)])).toEqual({ ...heldIntent(held), roll: -0.5 });
    expect(trial.tick([pad(0, 0.54, Math.sign(axis))])).toEqual({ ...heldIntent(held), roll: 0.5 });
    expect(trial.tick([pad(0, -0.54, axis / 2)])).toEqual(mapped(pad(0, -0.54, axis / 2)));
  });

  it("accepts an exact pickup and retains configured axes, directions, dead zone and range", () => {
    const configuration: BrowserPilotInputConfiguration = {
      ...DEFAULT_PILOT_INPUT_CONFIGURATION,
      pilotPositionRangeMeters: 0.8,
      gamepad: {
        axes: { roll: 2, pitch: 3, yaw: 0, pilotPosition: 1 },
        directions: { roll: -1, pitch: 1, yaw: -1, pilotPosition: 1 },
        deadZone: 0.2
      }
    };
    const exact = pad(0, 0, 0, { axes: [0.6, 0.6, -0.6, 0.6] });
    const held = mapped(exact, configuration).pilotPositionMeters;
    const trial = fixture(held, configuration);
    expect(trial.tick([pad(0)])).toEqual(heldIntent(held));
    expect(trial.tick([exact])).toEqual({ ...mapped(exact, configuration), pilotPositionMeters: held });
    const next = pad(0, 0, 0, { axes: [-0.6, -0.6, 0.6, -0.6] });
    expect(trial.tick([next])).toEqual(mapped(next, configuration));
  });

  it("requires every configured axis to be neutral before enabling steering", () => {
    const trial = fixture(0.2);
    for (let axis = 0; axis < 4; axis++) {
      const axes = [0, 0, 0, 0];
      axes[axis] = 0.54;
      expect(trial.tick([pad(0, 0, 0, { axes })])).toEqual(heldIntent(0.2));
    }
    expect(trial.tick([pad(0)])).toEqual(heldIntent(0.2));
    expect(trial.tick([pad(0, 0, 0, { axes: [0.54, -0.54, 0.54, 0] })])).toEqual({
      roll: 0.5, pitch: 0.5, yaw: 0.5, pilotPositionMeters: 0.2
    });
  });

  it("prioritizes keyboard input and reacquires the held target without changing the selected connection", () => {
    const trial = fixture();
    trial.tick([null, pad(1)]);
    const beforeKeyboard = trial.tick([null, pad(1, 0.54, -0.54)]).pilotPositionMeters;
    trial.key("keydown", "KeyL");
    const keyboard = trial.tick([pad(0, 1, 1), pad(1, 1, -1)]);
    expect(keyboard).toEqual(heldIntent(beforeKeyboard + 0.004));
    trial.key("keyup", "KeyL");
    expect(trial.tick([pad(0), pad(1, 1, -1)])).toEqual(heldIntent(keyboard.pilotPositionMeters));
    expect(trial.tick([pad(0, 1, 1), pad(1)])).toEqual(heldIntent(keyboard.pilotPositionMeters));
    expect(trial.tick([pad(0, 1, 1), pad(1, 0.54, -1)])).toEqual({ ...keyboard, roll: 0.5 });
    expect(trial.tick([pad(0, 1, 1), pad(1, -0.54, -0.54)])).toEqual(mapped(pad(1, -0.54, -0.54)));
  });

  it("rearms for a keyboard press released between ticks and ignores repeats and non-control keys", () => {
    const trial = fixture();
    trial.tick([pad(0)]);
    const active = pad(0, 0.54, -0.54);
    const held = trial.tick([active]).pilotPositionMeters;
    trial.key("keydown", "KeyL", true);
    trial.key("keydown", "KeyZ");
    expect(trial.tick([active])).toEqual(mapped(active));
    trial.key("keydown", "KeyD");
    trial.key("keyup", "KeyD");
    expect(trial.tick([active])).toEqual(heldIntent(held));
  });

  it.each(["blur", "resume", "reset"] as const)("rearms on %s without surrendering a connected selected index", (boundary) => {
    const trial = fixture();
    trial.tick([null, pad(1)]);
    const held = trial.tick([null, pad(1, 0.54, -0.54)]).pilotPositionMeters;
    if (boundary === "blur") trial.window.dispatchEvent(new trial.window.Event("blur"));
    else if (boundary === "reset") trial.controller.reset(snapshot(held));
    else {
      trial.controller.suspend();
      trial.connection("gamepadconnected", pad(0, 1, 1));
      const polls = trial.readGamepads.mock.calls.length;
      trial.frame();
      expect(trial.readGamepads).toHaveBeenCalledTimes(polls);
      trial.controller.resume();
    }
    if (boundary !== "blur") trial.frame();
    expect(trial.tick([pad(0), pad(1, 1, -1)])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0, 1, 1), pad(1)])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0, 1, 1), pad(1, 0.54, -1)])).toEqual({ ...heldIntent(held), roll: 0.5 });
  });

  it("records selected-slot reconnection while suspended before choosing a fresh candidate", () => {
    const trial = fixture();
    trial.tick([null, pad(1)]);
    const held = trial.tick([null, pad(1, 0.54, -0.54)]).pilotPositionMeters;
    trial.controller.suspend();
    trial.connection("gamepaddisconnected", pad(1));
    trial.connection("gamepadconnected", pad(1));
    trial.connection("gamepadconnected", pad(0, 1, -1));
    trial.controller.resume();
    trial.frame();
    expect(trial.tick([pad(0, 1, -1), pad(1)])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0), pad(1, -1, 1)])).toEqual(heldIntent(held));
    expect(trial.tick([pad(0, 0.54, -1), pad(1, -1, 1)])).toEqual({ ...heldIntent(held), roll: 0.5 });
    expect(trial.tick([pad(0, -0.54, -0.8), pad(1, -1, 1)])).toEqual(mapped(pad(0, -0.54, -0.8)));
  });

  it("keeps failed reset, terminal and disposed controllers closed to connection events until a valid reset", () => {
    const trial = fixture();
    trial.tick([pad(0)]);
    const held = trial.tick([pad(0, 0.54, -0.54)]).pilotPositionMeters;
    expect(() => { trial.controller.reset([]); }).toThrow();
    const retained = trial.controller.currentSnapshot;
    const polls = trial.readGamepads.mock.calls.length;
    const ticks = trial.session.advance_tick.mock.calls.length;
    trial.connection("gamepaddisconnected", pad(0));
    trial.connection("gamepadconnected", pad(0, 1, -1));
    trial.controller.resume();
    trial.frame();
    trial.frame();
    expect(trial.controller.currentSnapshot).toBe(retained);
    expect(trial.readGamepads).toHaveBeenCalledTimes(polls);
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(ticks);
    trial.controller.reset(snapshot(held));
    trial.frame();
    expect(trial.tick([pad(0, 1, -1)])).toEqual(heldIntent(held));
    trial.controller.reset(snapshot(held, 4));
    trial.controller.resume();
    trial.connection("gamepadconnected", pad(0));
    trial.frame();
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(ticks + 1);
    trial.controller.dispose();
    trial.connection("gamepadconnected", pad(0));
    trial.controller.resume();
    trial.frame();
    expect(trial.session.advance_tick).toHaveBeenCalledTimes(ticks + 1);
    expect(trial.session.free).toHaveBeenCalledTimes(1);
  });

  it("removes every installed listener and releases the input on repeated disposal", () => {
    const trial = fixture();
    const installed = trial.addListener.mock.calls;
    expect(installed.map(([type]) => type).sort()).toEqual([
      "blur", "gamepadconnected", "gamepaddisconnected", "keydown", "keyup"
    ]);
    trial.controller.dispose();
    trial.controller.dispose();
    trial.input.dispose();
    for (const [type, listener] of installed) expect(trial.removeListener).toHaveBeenCalledWith(type, listener);
    expect(trial.removeListener).toHaveBeenCalledTimes(5);
    expect(trial.session.free).toHaveBeenCalledTimes(1);
    trial.key("keydown", "KeyL");
    expect(trial.input.readIntent([pad(0, 1, -1)])).toEqual(heldIntent(0));
  });
});

function fixture(initialPilotPosition = 0, configuration = DEFAULT_PILOT_INPUT_CONFIGURATION) {
  const window = new BrowserWindow();
  vi.stubGlobal("HTMLElement", window.HTMLElement);
  const addListener = vi.spyOn(window, "addEventListener");
  const removeListener = vi.spyOn(window, "removeEventListener");
  const input = new BrowserPilotInput(window as unknown as Window, configuration);
  let gamepads: readonly (Gamepad | null)[] = [];
  let timestamp = 0;
  const readGamepads = vi.fn(() => gamepads);
  const session = {
    snapshot: () => snapshot(initialPilotPosition),
    advance_tick: vi.fn((_roll: number, _pitch: number, _yaw: number, pilotPosition: number) => snapshot(pilotPosition / 2)),
    free: vi.fn<() => void>()
  };
  const controller = new FlightController(session, input, { setFlightPose() {} }, {
    render() {}, fail: vi.fn<(message: string) => void>(), setVisible() {}
  }, 100, readGamepads);
  controller.onFrame(timestamp);
  disposals.push(() => { controller.dispose(); });
  const frame = (): void => { timestamp += 10; controller.onFrame(timestamp); };
  return {
    window, input, controller, session, readGamepads, addListener, removeListener, frame,
    tick: (samples: readonly (Gamepad | null)[]): PilotIntent => {
      gamepads = samples;
      const count = session.advance_tick.mock.calls.length;
      frame();
      expect(session.advance_tick).toHaveBeenCalledTimes(count + 1);
      const command = session.advance_tick.mock.lastCall;
      if (command === undefined) throw new Error("Expected a flight tick");
      return { roll: command[0], pitch: command[1], yaw: command[2], pilotPositionMeters: command[3] };
    },
    connection: (type: "gamepadconnected" | "gamepaddisconnected", gamepad: Gamepad): void => {
      const event = new window.Event(type);
      Object.defineProperty(event, "gamepad", { value: gamepad });
      window.dispatchEvent(event);
    },
    key: (type: "keydown" | "keyup", code: string, repeat = false): void => {
      window.dispatchEvent(new window.KeyboardEvent(type, { code, repeat, cancelable: true }));
    }
  };
}

function pad(index: number, roll = 0, pilotPosition = 0, options: {
  readonly timestamp?: number;
  readonly connected?: boolean;
  readonly axes?: readonly number[];
} = {}): Gamepad {
  return {
    index, connected: options.connected ?? true, id: "same-product", timestamp: options.timestamp ?? 0,
    axes: options.axes ?? [roll, 0, 0, pilotPosition]
  } as unknown as Gamepad;
}

function mapped(gamepad: Gamepad, configuration = DEFAULT_PILOT_INPUT_CONFIGURATION): PilotIntent {
  const intent = gamepadIntent([gamepad], configuration.gamepad, configuration.pilotPositionRangeMeters);
  if (intent === null) throw new Error("Expected a connected gamepad");
  return intent;
}

function heldIntent(pilotPositionMeters: number): PilotIntent {
  return { roll: 0, pitch: 0, yaw: 0, pilotPositionMeters };
}

function snapshot(pilotPosition: number, terminal = 0): number[] {
  const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
  values[7] = 1;
  values[11] = pilotPosition;
  values[16] = terminal;
  values[19] = -1;
  return values;
}
