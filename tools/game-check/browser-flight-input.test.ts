import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserFlightInput } from "../../web/src/game/browser-flight-input.js";
import type { FlightInputSink } from "../../web/src/game/browser-flight-input.js";

const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  vi.unstubAllGlobals();
});

function fixture() {
  const browser = new BrowserWindow();
  vi.stubGlobal("HTMLElement", browser.HTMLElement);
  const sink = { observeFlightKey: vi.fn<FlightInputSink["observeFlightKey"]>(() => true), invalidateFlightInput: vi.fn() } satisfies FlightInputSink;
  const sample = vi.fn<() => readonly (Gamepad | null)[]>(() => []);
  const input = new BrowserFlightInput(browser as unknown as Window, sink, sample);
  disposals.push(() => { input.dispose(); });
  const key = (type: string, code: string, repeat = false, target = browser) => {
    const event = new browser.KeyboardEvent(type, { code, repeat, bubbles: true, cancelable: true });
    target.dispatchEvent(event);
    return event;
  };
  const connection = (type: string, gamepad: Gamepad) => {
    const event = new browser.Event(type);
    Object.defineProperty(event, "gamepad", { value: gamepad });
    browser.dispatchEvent(event);
  };
  return { browser, input, sink, sample, key, connection };
}

function pad(index: number, axes: readonly number[], connected = true): Gamepad {
  return { index, connected, axes } as Gamepad;
}

describe("browser raw flight observations", () => {
  it("routes ordered press, repeat and release events and uses shared admission for default prevention", () => {
    const trial = fixture();
    expect(trial.key("keydown", "KeyL").defaultPrevented).toBe(true);
    trial.key("keydown", "KeyL", true);
    trial.key("keyup", "KeyL");
    expect(vi.mocked(trial.sink.observeFlightKey).mock.calls.map(([observation]) => observation)).toEqual([
      { control: "pilot_increase", pressed: true, repeat: false },
      { control: "pilot_increase", pressed: true, repeat: true },
      { control: "pilot_increase", pressed: false, repeat: false }
    ]);
    vi.mocked(trial.sink.observeFlightKey).mockReturnValue(false);
    expect(trial.key("keydown", "ArrowUp").defaultPrevented).toBe(false);
    const editor = trial.browser.document.createElement("input");
    trial.browser.document.body.append(editor);
    const before = vi.mocked(trial.sink.observeFlightKey).mock.calls.length;
    trial.key("keydown", "ArrowUp", false, editor as unknown as BrowserWindow);
    trial.key("keydown", "KeyA");
    expect(trial.sink.observeFlightKey).toHaveBeenCalledTimes(before);
    trial.key("keyup", "ArrowUp", false, editor as unknown as BrowserWindow);
    expect(trial.sink.observeFlightKey).toHaveBeenLastCalledWith({ control: "nose_up", pressed: false, repeat: false });
    vi.mocked(trial.sink.observeFlightKey).mockReturnValue(true);
    const button = trial.browser.document.createElement("button");
    trial.browser.document.body.append(button);
    expect(trial.key("keydown", "ArrowUp", false, button as unknown as BrowserWindow).defaultPrevented).toBe(true);
  });

  it("preserves unfiltered axes and connection words while mapping browser axis direction and unavailable readings", () => {
    const trial = fixture();
    const gamepad = pad(3, [0.03, -0.54, 0, -0.9]);
    trial.sample.mockReturnValue([null, pad(0, [], false), gamepad]);
    expect(trial.input.readGamepads()).toEqual([{ identity: { slot_low: 3, slot_high: 0, generation: 0 },
      connected: true, axes: [0.54, 0.03, 0.9] }]);
    trial.connection("gamepaddisconnected", gamepad);
    trial.connection("gamepadconnected", gamepad);
    trial.sample.mockReturnValue([pad(3, [NaN, Infinity])]);
    expect(trial.input.readGamepads()).toEqual([{ identity: { slot_low: 3, slot_high: 0, generation: 2 },
      connected: true, axes: [null, null, null] }]);
  });

  it("invalidates on blur and removes observers before final input invalidation", () => {
    const trial = fixture();
    trial.browser.dispatchEvent(new trial.browser.Event("blur"));
    expect(trial.sink.invalidateFlightInput).toHaveBeenCalledOnce();
    trial.input.dispose();
    trial.input.dispose();
    trial.key("keydown", "ArrowUp");
    trial.browser.dispatchEvent(new trial.browser.Event("blur"));
    expect(trial.sink.observeFlightKey).not.toHaveBeenCalled();
    expect(trial.sink.invalidateFlightInput).toHaveBeenCalledTimes(2);
    expect(trial.input.readGamepads()).toEqual([]);
    expect(trial.sample).not.toHaveBeenCalled();
  });
});
