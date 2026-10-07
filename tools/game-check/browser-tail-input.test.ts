import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserTailPilotInput, DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION } from "../../web/src/game/browser-tail-input.js";
import { tailGamepadDemand } from "../../web/src/game/tail-device-input.js";

const disposals: (() => void)[] = [];
afterEach(() => {
  for (const dispose of disposals.splice(0)) dispose();
  vi.unstubAllGlobals();
});

function fixture() {
  const browser = new BrowserWindow();
  vi.stubGlobal("HTMLElement", browser.HTMLElement);
  const input = new BrowserTailPilotInput(browser as unknown as Window);
  disposals.push(() => { input.dispose(); });
  const key = (type: string, code: string, target = browser) => target.dispatchEvent(new browser.KeyboardEvent(type, { code, bubbles: true, cancelable: true }));
  const connection = (type: string, gamepad: Gamepad) => {
    const event = new browser.Event(type);
    Object.defineProperty(event, "gamepad", { value: gamepad });
    browser.dispatchEvent(event);
  };
  return { input, browser, key, connection };
}

function pad(index: number, turnRight = 0, noseUp = 0, pilotPosition = 0): Gamepad {
  return { index, connected: true, id: "tail-test-pad", timestamp: 0, axes: [turnRight, noseUp, 0, pilotPosition] } as unknown as Gamepad;
}

describe("browser two-tail logical input", () => {
  it("uses the Rust held target for keyboard increments and releases to Hold without an independent roll binding", () => {
    const trial = fixture();
    trial.input.reset(0.75);
    trial.key("keydown", "KeyL");
    trial.key("keydown", "ArrowUp");
    trial.key("keydown", "ArrowRight");
    trial.key("keydown", "KeyA");
    expect(trial.input.readDemand([], 0.75)).toEqual({ controlLayout: "tail_incidence", noseUp: 1, turnRight: 1,
      pilotPositionCommand: { kind: "set", normalized: 0.76 } });
    trial.key("keyup", "KeyL");
    expect(trial.input.readDemand([], 0.76).pilotPositionCommand).toEqual({ kind: "hold" });
    trial.key("keydown", "KeyJ");
    expect(trial.input.readDemand([], -0.9).pilotPositionCommand).toEqual({ kind: "set", normalized: -0.91 });
    expect(() => trial.input.readDemand([], 1.01)).toThrow(RangeError);
  });

  it("requires neutral acquisition and pickup of the held target before absolute gamepad positioning", () => {
    const trial = fixture();
    trial.input.reset(0.5);
    expect(trial.input.readDemand([pad(0, 1, -1, -1)], 0.5).pilotPositionCommand).toEqual({ kind: "hold" });
    expect(trial.input.readDemand([pad(0)], 0.5).pilotPositionCommand).toEqual({ kind: "hold" });
    expect(trial.input.readDemand([pad(0, 0.54, -0.54, -0.2)], 0.5)).toEqual({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: 0.5,
      pilotPositionCommand: { kind: "hold" } });
    expect(trial.input.readDemand([pad(0, 0.54, -0.54, -0.8)], 0.5).pilotPositionCommand).toEqual({ kind: "hold" });
    const acquired = pad(0, 0.54, -0.54, -1);
    expect(trial.input.readDemand([acquired], 0.5)).toEqual(tailGamepadDemand([acquired]));
    trial.connection("gamepaddisconnected", acquired);
    trial.connection("gamepadconnected", acquired);
    expect(trial.input.readDemand([acquired], 0.5)).toMatchObject({ noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" } });
  });

  it("keeps the selected pad, rearms after keyboard ownership, and isolates editable focus", () => {
    const trial = fixture();
    trial.input.readDemand([pad(1)], 0);
    trial.input.readDemand([pad(1)], 0);
    const steering = pad(1, 0.54, -0.54);
    expect(trial.input.readDemand([pad(0, 1, -1), steering], 0)).toEqual(tailGamepadDemand([steering]));
    trial.key("keydown", "ArrowDown");
    expect(trial.input.readDemand([steering], 0)).toMatchObject({ noseUp: -1, turnRight: 0 });
    trial.key("keyup", "ArrowDown");
    expect(trial.input.readDemand([steering], 0)).toMatchObject({ noseUp: 0, turnRight: 0 });
    const editor = trial.browser.document.createElement("input");
    trial.browser.document.body.append(editor);
    trial.key("keydown", "ArrowUp", editor as unknown as BrowserWindow);
    expect(trial.input.readDemand([], 0)).toMatchObject({ noseUp: 0 });
  });

  it("suspends, resets and disposes without issuing a neutral position command", () => {
    const trial = fixture();
    trial.key("keydown", "KeyL");
    trial.input.suspend();
    trial.key("keydown", "ArrowUp");
    expect(trial.input.readDemand([], 0.75)).toMatchObject({ noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" } });
    trial.input.reset(-0.5);
    trial.input.resume();
    expect(trial.input.readDemand([], -0.5).pilotPositionCommand).toEqual({ kind: "hold" });
    trial.input.dispose();
    trial.input.dispose();
    trial.key("keydown", "KeyJ");
    expect(trial.input.readDemand([], -0.5).pilotPositionCommand).toEqual({ kind: "hold" });
    expect(() => { trial.input.reset(0); }).toThrow("disposed");
    const invalid = { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION,
      gamepad: { ...DEFAULT_BROWSER_TAIL_INPUT_CONFIGURATION.gamepad, deadZone: 1 } };
    expect(() => new BrowserTailPilotInput(trial.browser as unknown as Window, invalid)).toThrow(RangeError);
  });
});
