import { configuredViewerFixture } from "./viewer-fixture.js";
import { fixtureBackendFrame, fixtureSemanticAction } from "./menu-fixture.js";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { GameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel, GameSessionOperation, GameSessionProjection } from "../../web/src/app/app-state.js";
import { executeGameSessionOperation } from "../../web/src/app/game-session-operation.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import { createBrowserPhoneVrGamepadInputPort } from "../../web/src/presentation/phone-vr-gamepad-browser.js";
import { GamepadUiSelector } from "../../web/src/presentation/gamepad-ui-selector.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { NO_PHONE_VR_GAMEPAD_INPUT } from "../../web/src/presentation/phone-vr-contracts.js";
import type { PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import type { RendererAdapter } from "../../web/src/render/contracts/runtime.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";

const cleanups: (() => Promise<void>)[] = [];
beforeAll(() => {
  initSync({ module: new Uint8Array(readFileSync(fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url)))) });
});
afterEach(async () => {
  for (const cleanup of cleanups.splice(0)) await cleanup();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("Phone VR UI Gamepad connection ownership", () => {
  it.each(["remaining device", "same index reconnect", "lower index addition"] as const)(
    "does not start the actual Rust countdown from a held replacement button: %s", async (change) => {
      const first = gamepad(change === "lower index addition" ? 1 : 0);
      const replacement = gamepad(change === "remaining device" ? 1 : 0, [true, true]);
      const browser = new GamepadBrowser();
      browser.connect(first);
      if (change === "remaining device") browser.connect(replacement);
      const fixture = await createFixture(browser);
      fixture.frame();
      moveToStart(fixture, first);
      if (change === "lower index addition") browser.connect(replacement);
      else {
        browser.disconnect(first.index);
        if (change === "same index reconnect") browser.connect(replacement);
      }
      fixture.actions.length = 0;
      fixture.frame();
      expect(fixture.operations).not.toContain("start-flight");
      expect(fixture.session.phase_code()).toBe(3);
      expect(fixture.actions.filter((action) => action.type === "activate" || action.type === "back")).toEqual([]);
      const selected = change === "lower index addition" ? first : replacement;
      if (selected === replacement) moveToStart(fixture, selected);
      fixture.frame();
      expect(fixture.operations).toEqual([]);
      selected.buttons = [{ pressed: false }, { pressed: false }];
      fixture.frame();
      selected.buttons = [{ pressed: true }, { pressed: false }];
      fixture.frame();
      expect(fixture.operations).toEqual(["start-flight"]);
      expect(fixture.session.phase_code()).toBe(4);
      fixture.frame();
      expect(fixture.operations).toEqual(["start-flight"]);
    }
  );
});

describe("browser Gamepad listener ownership", () => {
  it("registers lazily, preserves a connection across object refreshes, and releases both listeners", () => {
    const browser = new GamepadBrowser();
    browser.connect(gamepad(2));
    const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
    expect(browser.listenerCount()).toBe(0);
    expect(input.readState()).toBeNull();
    input.start();
    input.start();
    expect(browser.listenerCount()).toBe(2);
    const first = input.readState();
    browser.pads[2] = { ...gamepad(2), id: "updated-description", timestamp: 500 };
    expect(input.readState()?.connection).toEqual(first?.connection);
    browser.connect(gamepad(0));
    expect(input.readState()?.connection).toEqual(first?.connection);
    input.stop();
    input.stop();
    expect(browser.listenerCount()).toBe(0);
    expect(input.readState()).toBeNull();
    input.start();
    expect(input.readState()?.connection).not.toEqual(first?.connection);
    input.stop();
  });

  it("ignores captured old callbacks after a restart and recognizes poll-time disappearance", () => {
    const browser = new GamepadBrowser();
    browser.connect(gamepad(0));
    const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
    input.start();
    const oldCallbacks = [...browser.captured];
    input.stop();
    input.start();
    const current = input.readState()?.connection;
    for (const captured of oldCallbacks) captured.listener(gamepadEvent(captured.type, gamepad(0)));
    expect(input.readState()?.connection).toEqual(current);
    browser.pads[0] = null;
    expect(input.readState()).toBeNull();
    browser.pads[0] = gamepad(0);
    expect(input.readState()?.connection).not.toEqual(current);
    input.stop();
    expect(browser.listenerCount()).toBe(0);
  });

  it("keeps absent APIs, missing lifecycle events, unsupported mappings and polling failures unavailable", () => {
    for (const input of [NO_PHONE_VR_GAMEPAD_INPUT, createBrowserPhoneVrGamepadInputPort({}, new EventTarget()),
      createBrowserPhoneVrGamepadInputPort({ getGamepads: () => [gamepad(0)] }, null),
      createBrowserPhoneVrGamepadInputPort({ getGamepads: () => { throw new Error("poll blocked"); } }, new EventTarget())]) {
      input.start();
      expect(input.readState()).toBeNull();
      input.stop();
    }
    const browser = new GamepadBrowser();
    browser.connect({ ...gamepad(0), mapping: "" });
    browser.pads[1] = { ...gamepad(1), connected: false };
    const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
    input.start();
    expect(input.readState()).toBeNull();
    browser.connect({ ...gamepad(2), axes: [Number.NaN, Number.POSITIVE_INFINITY, 0.5] });
    expect(input.readState()?.axes).toEqual([0, 0, 0.5]);
    input.stop();
  });

  it("cleans a partial registration failure and preserves its cause despite removal failures", () => {
    const browser = new GamepadBrowser();
    const failure = new Error("second registration failed");
    browser.onAdd = (type) => { if (type === "gamepaddisconnected") throw failure; };
    browser.onRemove = () => { throw new Error("removal failed after detach"); };
    const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
    expect(() => { input.start(); }).toThrow(failure);
    expect(browser.listenerCount()).toBe(0);
    expect(input.readState()).toBeNull();
    browser.onAdd = () => undefined;
    browser.onRemove = () => undefined;
    input.start();
    expect(browser.listenerCount()).toBe(2);
    input.stop();
  });

  it("does not retain listeners when stop is reentrant during registration", () => {
    const browser = new GamepadBrowser();
    const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
    browser.onAdd = () => { input.stop(); };
    input.start();
    expect(browser.listenerCount()).toBe(0);
    expect(input.readState()).toBeNull();
  });

  it("releases only old listeners when a new session starts during removal", () => {
    const browser = new GamepadBrowser();
    browser.connect(gamepad(0));
    const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
    input.start();
    const original = input.readState()?.connection;
    browser.onRemove = () => { browser.onRemove = () => undefined; input.start(); };
    input.stop();
    expect(browser.listenerCount()).toBe(2);
    expect(input.readState()?.connection).not.toEqual(original);
    input.stop();
    expect(browser.listenerCount()).toBe(0);
  });
});

describe("Phone VR attempts own the browser Gamepad port", () => {
  it.each(["stop", "cancel"] as const)("releases listeners on %s before a sample and isolates old callbacks after retry", async (operation) => {
    vi.useFakeTimers();
    const browser = new GamepadBrowser();
    browser.connect(gamepad(0));
    const fixture = await createFixture(browser, { autoStart: false, initialSample: false });
    await fixture.backend.requestPermissionFromUserGesture();
    const first = fixture.backend.start();
    const rejected = expect(first).rejects.toThrow(operation === "stop" ? "stopped" : "canceled");
    const oldCallbacks = [...browser.captured];
    const oldSensors = fixture.sensorCallbacks[0];
    expect(browser.listenerCount()).toBe(2);
    if (operation === "stop") await fixture.backend.stop();
    else await fixture.backend.cancelPendingRequest();
    await rejected;
    expect(browser.listenerCount()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
    await fixture.backend.requestPermissionFromUserGesture();
    const second = fixture.backend.start();
    const current = fixture.input.readState()?.connection;
    for (const captured of oldCallbacks) captured.listener(gamepadEvent(captured.type, gamepad(0)));
    oldSensors?.reading({ alpha: null, beta: null, gamma: null, timestampMs: 0, gravityEvidence: { kind: "earth-z-up" } });
    oldSensors?.rotation(null);
    expect(fixture.input.readState()?.connection).toEqual(current);
    expect(browser.listenerCount()).toBe(2);
    expect(vi.getTimerCount()).toBe(1);
    fixture.sensorCallbacks[1]?.reading({ alpha: 0, beta: 90, gamma: 0, timestampMs: 0, gravityEvidence: { kind: "earth-z-up" } });
    await second;
    expect((fixture.frame().panel.kind === "visible")).toBe(true);
    expect(fixture.unavailable).not.toHaveBeenCalled();
    await fixture.backend.stop();
    expect(browser.listenerCount()).toBe(0);
  });

  it("releases browser listeners on the first-sample timeout", async () => {
    vi.useFakeTimers();
    const browser = new GamepadBrowser();
    const fixture = await createFixture(browser, { autoStart: false, initialSample: false });
    await fixture.backend.requestPermissionFromUserGesture();
    const rejected = expect(fixture.backend.start()).rejects.toThrow("before timeout");
    expect(browser.listenerCount()).toBe(2);
    await vi.advanceTimersByTimeAsync(50);
    await rejected;
    expect(browser.listenerCount()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
    expect(fixture.renderer.setStereoPresentation).toHaveBeenLastCalledWith(null);
  });

  it("cleans partial Gamepad setup without attaching sensors and permits a fresh attempt", async () => {
    const browser = new GamepadBrowser();
    const fixture = await createFixture(browser, { autoStart: false });
    const failure = new Error("Gamepad setup failure");
    browser.onAdd = (type) => { if (type === "gamepaddisconnected") throw failure; };
    await fixture.backend.requestPermissionFromUserGesture();
    await expect(fixture.backend.start()).rejects.toBe(failure);
    expect(browser.listenerCount()).toBe(0);
    expect(fixture.sensors.startListening).not.toHaveBeenCalled();
    expect(fixture.renderer.setStereoPresentation).toHaveBeenLastCalledWith(null);
    browser.onAdd = () => undefined;
    await fixture.backend.requestPermissionFromUserGesture();
    await fixture.backend.start();
    expect((fixture.frame().panel.kind === "visible")).toBe(true);
    expect(browser.listenerCount()).toBe(2);
  });

  it("settles a reentrant stop during Gamepad registration before sensor setup", async () => {
    const browser = new GamepadBrowser();
    const fixture = await createFixture(browser, { autoStart: false });
    const stops: Promise<void>[] = [];
    browser.onAdd = () => { stops.push(fixture.backend.stop()); };
    await fixture.backend.requestPermissionFromUserGesture();
    await expect(fixture.backend.start()).rejects.toThrow("stopped");
    await Promise.all(stops);
    expect(browser.listenerCount()).toBe(0);
    expect(fixture.sensors.startListening).not.toHaveBeenCalled();
    browser.onAdd = () => undefined;
    await fixture.backend.requestPermissionFromUserGesture();
    await fixture.backend.start();
    expect(browser.listenerCount()).toBe(2);
  });

  it.each(["stop", "tracking failure"] as const)("finishes %s despite both Gamepad removals throwing and rejects reentrant startup", async (operation) => {
    const browser = new GamepadBrowser();
    const fixture = await createFixture(browser);
    const failures = [new Error("connected removal failed"), new Error("disconnected removal failed")] as const;
    const permissions: Promise<PhoneVrPermissionResult>[] = [];
    browser.onRemove = (type) => {
      permissions.push(fixture.backend.requestPermissionFromUserGesture());
      throw failures[type === "gamepadconnected" ? 0 : 1];
    };
    if (operation === "stop") await expect(fixture.backend.stop()).rejects.toBe(failures[0]);
    else {
      fixture.sensorCallbacks[0]?.reading({ alpha: null, beta: null, gamma: null, timestampMs: 0, gravityEvidence: { kind: "earth-z-up" } });
      expect(fixture.unavailable).toHaveBeenCalledExactlyOnceWith("Phone VR orientation data contains null or non-finite values");
    }
    expect(browser.listenerCount()).toBe(0);
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
    expect(fixture.renderer.setStereoPresentation).toHaveBeenLastCalledWith(null);
    expect(permissions).toHaveLength(2);
    for (const permission of permissions) expect(await permission).toMatchObject({ ok: false });
    browser.onRemove = () => undefined;
    await fixture.backend.requestPermissionFromUserGesture();
    await fixture.backend.start();
    expect((fixture.frame().panel.kind === "visible")).toBe(true);
    expect(browser.listenerCount()).toBe(2);
  });
});

describe("connection-scoped Menu cursor history", () => {
  const panel = createSceneFixture("Title").panels[0];
  if (panel === undefined) throw new Error("Missing title panel");
  const sample = (generation: number, buttons = [false, false], axes = [0, 0, 0, 0]) => ({
    connection: { index: 0, generation }, buttons, axes
  });

  it("resets cursor/focus/time, seeds held toggle/back, and retains range and scroll operation", () => {
    const actions: UiAction[] = [];
    const selector = new GamepadUiSelector((action) => { actions.push(fixtureSemanticAction(action)); });
    selector.update(panel, sample(1), 0);
    selector.update(panel, sample(1, [false, false], [1, 1, 0, 0]), 50);
    actions.length = 0;
    expect(selector.update(panel, sample(2, [true, true], [1, 1, 1, 1]), 5000)?.point).toEqual({ x: 0, y: 0 });
    expect(actions.filter((action) => action.type !== "focus")).toEqual([]);
    selector.update(panel, sample(2, [true, true]), 5016);
    selector.update(panel, sample(2), 5032);
    selector.update(panel, sample(2, [true, true]), 5048);
    expect(actions.filter((action) => action.type === "set-toggle")).toEqual([{ type: "set-toggle", controlId: "title-toggle", value: true }]);
    expect(actions.filter((action) => action.type === "back")).toHaveLength(1);
    for (let frame = 1; frame <= 8; frame++) selector.update(panel, sample(2, [false, false], [0, -1, 0, 0]), 5048 + frame * 16);
    selector.update(panel, sample(2, [true, false], [0, 0, 1, 1]), 5192);
    expect(actions).toContainEqual({ type: "set-range", controlId: "title-range", value: 0.5 });
    expect(actions.some((action) => action.type === "scroll" && action.deltaX > 0 && action.deltaY > 0)).toBe(true);
    selector.update(panel, null, 5208);
    actions.length = 0;
    expect(selector.update(panel, sample(2, [true, true]), 5224)?.point).toEqual({ x: 0, y: 0 });
    expect(actions.filter((action) => action.type !== "focus")).toEqual([]);
  });

  it("finishes internal reset before a throwing external focus notification", () => {
    const dispatch = vi.fn<(action: UiAction) => void>();
    const selector = new GamepadUiSelector(dispatch);
    selector.update(panel, sample(1), 0);
    const failure = new Error("focus observer failed");
    dispatch.mockImplementationOnce(() => { throw failure; });
    expect(() => { selector.reset(); }).toThrow(failure);
    dispatch.mockClear();
    expect(() => { selector.reset(); }).not.toThrow();
    expect(dispatch).not.toHaveBeenCalled();
    selector.update(panel, sample(1, [true, true]), 1000);
    expect(dispatch.mock.calls.map(([action]) => action).filter((action) => action.type !== "focus")).toEqual([]);
  });

  it("does not replay held inputs after handover focus dispatch throws", () => {
    const dispatch = vi.fn<(action: UiAction) => void>();
    const selector = new GamepadUiSelector(dispatch);
    selector.update(panel, sample(1), 0);
    dispatch.mockImplementationOnce(() => { throw new Error("handover focus failed"); });
    expect(() => selector.update(panel, sample(2, [true, true]), 10)).toThrow("handover focus failed");
    dispatch.mockClear();
    selector.update(panel, sample(2, [true, true]), 20);
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("abandons queued old-connection actions after a reentrant reset", () => {
    const dispatch = vi.fn<(action: UiAction) => void>();
    const selector = new GamepadUiSelector(dispatch);
    selector.update(panel, sample(1), 0);
    dispatch.mockClear();
    dispatch.mockImplementationOnce(() => { selector.reset(); });
    expect(selector.update(panel, sample(1, [true, true], [0, -1, 1, 1]), 50)).toBeNull();
    expect(dispatch.mock.calls.map(([action]) => action).filter((action) => action.type !== "focus")).toEqual([]);
  });
});

interface TestGamepad {
  readonly index: number;
  readonly id: string;
  readonly timestamp: number;
  connected: boolean;
  mapping: string;
  axes: readonly number[];
  buttons: readonly { readonly pressed: boolean }[];
}

class GamepadBrowser extends EventTarget {
  readonly pads: (TestGamepad | null)[] = [];
  readonly getGamepads = () => this.pads;
  readonly captured: { readonly type: string; readonly listener: EventListener }[] = [];
  private readonly listeners = new Map<string, Set<EventListenerOrEventListenerObject>>();
  onAdd: (type: string) => void = () => undefined;
  onRemove: (type: string) => void = () => undefined;

  override addEventListener(type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | AddEventListenerOptions): void {
    super.addEventListener(type, listener, options);
    if (listener !== null) {
      const group = this.listeners.get(type) ?? new Set<EventListenerOrEventListenerObject>();
      group.add(listener);
      this.listeners.set(type, group);
      if (typeof listener === "function") this.captured.push({ type, listener });
    }
    this.onAdd(type);
  }

  override removeEventListener(type: string, listener: EventListenerOrEventListenerObject | null, options?: boolean | EventListenerOptions): void {
    super.removeEventListener(type, listener, options);
    if (listener !== null) this.listeners.get(type)?.delete(listener);
    this.onRemove(type);
  }

  listenerCount(): number { return [...this.listeners.values()].reduce((total, listeners) => total + listeners.size, 0); }

  connect(pad: TestGamepad): void {
    this.pads[pad.index] = pad;
    this.dispatchEvent(gamepadEvent("gamepadconnected", pad));
  }

  disconnect(index: number): void {
    const pad = this.pads[index];
    if (pad === undefined || pad === null) throw new Error("Cannot disconnect an absent gamepad");
    pad.connected = false;
    this.pads[index] = null;
    this.dispatchEvent(gamepadEvent("gamepaddisconnected", pad));
  }
}

function gamepadEvent(type: string, pad: TestGamepad): Event {
  const event = new Event(type);
  Object.defineProperty(event, "gamepad", { value: pad });
  return event;
}

function gamepad(index: number, buttons: readonly boolean[] = [false, false]): TestGamepad {
  return { index, id: "same-product", timestamp: 1, connected: true, mapping: "standard", axes: [0, 0, 0, 0], buttons: buttons.map((pressed) => ({ pressed })) };
}

async function createFixture(browser: GamepadBrowser, options: { readonly autoStart?: boolean; readonly initialSample?: boolean } = {}) {
  const session = new GameSessionBridge(0);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  const actions: UiAction[] = [];
  const operations: GameSessionOperation[] = [];
  let model: AppModel = { ...createInitialAppModel(), presentation: { type: "ready", mode: "phone-vr" } };
  const projection = (): GameSessionProjection => {
    const cues = session.information_profile_codes();
    return {
      phaseCode: session.phase_code(), controlModeCode: session.control_mode_code(), countdownRemaining: session.countdown_remaining(),
      ...(session.phase_code() === 9 ? { returnTarget: session.is_archived_replay() ? "title" as const : "result" as const } : {}),
      canResume: session.can_resume(), configurationMetadata: null, snapshot: null,
      difficulty: {
        presetCode: session.difficulty_preset_code(), informationCode: session.information_level_code(), assistanceCode: session.assistance_level_code(), weatherCode: session.weather_class_code(),
        hudProfile: { telemetry: cues[0] === 1, attitude: cues[1] === 1, wind: cues[2] === 1, flightPath: cues[3] === 1, angleOfAttack: cues[4] === 1, warnings: cues[5] === 1 }
      }
    };
  };
  const dispatch = (action: UiAction): void => {
    const semantic = fixtureSemanticAction(action);
    actions.push(semantic);
    model = updateApp(model, { type: "game-session-synced", ...projection() }).model;
    const update = updateApp(model, { type: "ui-action", action: semantic });
    model = update.model;
    for (const effect of update.effects) {
      if (effect.type !== "game-session-operation") continue;
      operations.push(effect.operation);
      executeGameSessionOperation(session, effect.operation);
      model = updateApp(model, { type: "game-operation-completed", requestId: effect.requestId, ...projection() }).model;
    }
  };
  const sensorCallbacks: { readonly reading: (reading: PhoneVrSensorReading) => void; readonly rotation: (angle: number | null) => void }[] = [];
  const sensors = {
    checkAvailability: () => Promise.resolve({ supported: true, message: "fixture" }),
    requestPermissionFromUserGesture: () => Promise.resolve({ ok: true }),
    getScreenOrientationAngle: () => 0,
    startListening: vi.fn<PhoneVrSensorPort["startListening"]>((reading, rotation) => {
      sensorCallbacks.push({ reading, rotation });
      if (options.initialSample !== false) reading({ alpha: 0, beta: 90, gamma: 0, timestampMs: 0, gravityEvidence: { kind: "earth-z-up" } });
    }),
    stopListening: vi.fn<PhoneVrSensorPort["stopListening"]>()
  } satisfies PhoneVrSensorPort;
  const renderer = {
    startLoop: vi.fn(), beginViewFrame: vi.fn(), stopLoop: vi.fn(), render: vi.fn(), resize: vi.fn(), dispose: vi.fn(), setSelectRayHandler: vi.fn(),
    setFlightPose: vi.fn(), setPreparedFlightPose: vi.fn(), setLakeVisualCondition: vi.fn(), setLakeSkyCondition: vi.fn(), setLakeVenueVisible: vi.fn(), setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(), setStereoPresentation: vi.fn(),
    transformTrackingPose: (pose) => pose
  } satisfies RendererAdapter;
  const input = createBrowserPhoneVrGamepadInputPort(browser, browser);
  const unavailable = vi.fn<(message: string) => void>();
  const backend = new PhoneVrPresentationBackend(sensors, renderer, () => ({ x: 1280, y: 720, pixelRatio: 1 }), dispatch, unavailable, { gamepadInput: input, nowMs: () => 100, firstSampleTimeoutMs: 50 });
  cleanups.push(async () => { try { await backend.stop(); } finally { session.free(); } });
  if (options.autoStart !== false) {
    await backend.requestPermissionFromUserGesture();
    await backend.start();
  }
  let timestamp = 100;
  const frame = () => {
    timestamp += 16;
    model = updateApp(model, { type: "game-session-synced", ...projection() }).model;
    return fixtureBackendFrame(backend, timestamp, createGameViewModel(model, null), configuredViewerFixture());
  };
  return { session, backend, input, actions, operations, frame, sensorCallbacks, sensors, renderer, unavailable };
}

function moveToStart(fixture: Awaited<ReturnType<typeof createFixture>>, pad: TestGamepad): void {
  for (let frame = 0; frame < 120; frame++) {
    const focus = fixture.actions.filter((action) => action.type === "focus").at(-1);
    if (focus?.controlId === "game-briefing-start") break;
    pad.axes = [0, 1, 0, 0];
    fixture.frame();
  }
  pad.axes = [0, 0, 0, 0];
  fixture.frame();
  expect(fixture.actions.filter((action) => action.type === "focus").at(-1)).toEqual({ type: "focus", controlId: "game-briefing-start" });
}
