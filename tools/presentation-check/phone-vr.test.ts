import { describe, expect, it } from "vitest";
import { IDENTITY_POSE, quaternion } from "../../web/src/render/contracts/math.js";
import type { RendererAdapter, StereoPresentationProfile, ViewportSize } from "../../web/src/render/contracts/runtime.js";
import type { UiAction, UiViewModel } from "../../web/src/render/contracts/ui.js";
import { createAnchorFixture, createSceneFixture, SCENE_FIXTURE_OVERLAYS } from "../../web/src/presentation/fixtures.js";
import { createBrowserPhoneVrSensorPort } from "../../web/src/presentation/phone-vr-browser.js";
import { createBrowserPhoneVrGamepadInputPort } from "../../web/src/presentation/phone-vr-gamepad-browser.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import { GamepadUiSelector } from "../../web/src/presentation/gamepad-ui-selector.js";
import type { PhoneVrAvailability, PhoneVrGamepadInputPort, PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import { PHONE_VR_OPTICAL_PROFILE } from "../../web/src/presentation/phone-vr-contracts.js";
import { phoneOrientationQuaternion } from "../../web/src/presentation/phone-vr-orientation.js";
import { GAME_SCENES } from "../../web/src/render/contracts/ui.js";

describe("Phone VR device orientation", () => {
  it("converts the W3C intrinsic Z-X'-Y'' angles and screen rotation", () => {
    expect(phoneOrientationQuaternion(reading(0, 0, 0), 0)).toEqual(IDENTITY_POSE.orientation);
    expect(phoneOrientationQuaternion(reading(90, 0, 0), 0)).toEqual(axisQuaternion("z", 90));
    expect(phoneOrientationQuaternion(reading(0, 90, 0), 0)).toEqual(axisQuaternion("x", 90));
    expect(phoneOrientationQuaternion(reading(0, 0, 90), 0)).toEqual(axisQuaternion("y", 90));
    expect(phoneOrientationQuaternion(reading(0, 0, 0), 90)).toEqual(axisQuaternion("z", 90));
  });

  it("rejects null and non-finite sensor data instead of substituting zero", () => {
    expect(phoneOrientationQuaternion({ ...reading(0, 0, 0), alpha: null }, 0)).toBeNull();
    expect(phoneOrientationQuaternion(reading(0, Number.NaN, 0), 0)).toBeNull();
    expect(phoneOrientationQuaternion(reading(0, 0, 0), Number.POSITIVE_INFINITY)).toBeNull();
  });
});

describe("Phone VR browser sensor adapter", () => {
  it("requests permission synchronously, preserves null angles, reports rotation, and removes listeners", async () => {
    const browser = createMockBrowserWindow();
    const sensors = createBrowserPhoneVrSensorPort(browser.window);
    expect(await sensors.checkAvailability()).toMatchObject({ supported: true });
    const permission = sensors.requestPermissionFromUserGesture();
    expect(browser.permissionRequests).toEqual([false]);
    expect(await permission).toEqual({ ok: true });

    const readings: PhoneVrSensorReading[] = [];
    const screenAngles: Array<number | null> = [];
    sensors.startListening((value) => { readings.push(value); }, (angle) => { screenAngles.push(angle); });
    browser.emitDeviceOrientation({ alpha: null, beta: 20, gamma: -10, timestampMs: 42 });
    browser.setScreenAngle(90);
    browser.emitScreenOrientationChange();
    expect(readings).toEqual([{ alpha: null, beta: 20, gamma: -10, timestampMs: 42 }]);
    expect(screenAngles).toEqual([90]);

    sensors.stopListening();
    browser.emitDeviceOrientation({ alpha: 0, beta: 0, gamma: 0, timestampMs: 50 });
    browser.emitScreenOrientationChange();
    expect(readings).toHaveLength(1);
    expect(screenAngles).toHaveLength(1);
  });

  it("reports insecure contexts and missing sensor APIs", async () => {
    const insecure = createMockBrowserWindow({ secure: false });
    expect(await createBrowserPhoneVrSensorPort(insecure.window).checkAvailability()).toMatchObject({ supported: false });
    const missingOrientation = createMockBrowserWindow({ orientationApi: false });
    expect(await createBrowserPhoneVrSensorPort(missingOrientation.window).checkAvailability()).toMatchObject({ supported: false });
    const missingScreen = createMockBrowserWindow({ screenOrientationApi: false });
    expect(await createBrowserPhoneVrSensorPort(missingScreen.window).checkAvailability()).toMatchObject({ supported: false });
  });
});

describe("Phone VR gamepad UI", () => {
  it("moves a cursor, focuses controls, selects, backs, and scrolls through common actions", () => {
    const actions: UiAction[] = [];
    const selector = new GamepadUiSelector((action) => { actions.push(action); });
    const panel = createSceneFixture("Title").panels[0];
    if (panel === undefined) throw new Error("Missing UI panel fixture");
    selector.update(panel, gamepadState([0, 0, 0, 0], [false, false]), 100);
    let cursor = null;
    for (let frame = 1; frame <= 13; frame++) {
      cursor = selector.update(panel, gamepadState([0, 1, 0, 0], [false, false]), 100 + frame * 16);
    }
    expect(cursor?.point.y).toBeCloseTo(-0.449, 2);
    expect(actions).toContainEqual({ type: "focus", controlId: "title-action" });

    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 324);
    selector.update(panel, gamepadState([0, 0, 0, 0.5], [true, true]), 340);
    expect(actions).toContainEqual({ type: "activate", controlId: "title-action" });
    expect(actions).toContainEqual({ type: "back" });
    expectScrollAction(actions);
    expect(cursor?.progress).toBe(0);

    selector.reset();
    actions.length = 0;
    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 400);
    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 416);
    expect(actions).not.toContainEqual({ type: "set-toggle", controlId: "title-toggle", value: true });
    selector.update(panel, gamepadState([0, 0, 0, 0], [false, false]), 432);
    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 448);
    expect(actions).toContainEqual({ type: "set-toggle", controlId: "title-toggle", value: true });
  });

  it("reads only connected standard-mapped browser gamepads", () => {
    let currentGamepads: readonly unknown[] = [
      { connected: true, mapping: "", axes: [1, 1], buttons: [{ pressed: true }] },
      { connected: true, mapping: "standard", axes: [0.5, -0.25, 0, 1], buttons: [{ pressed: true }, { pressed: false }] }
    ];
    const input = createBrowserPhoneVrGamepadInputPort({
      getGamepads: () => currentGamepads as never
    });
    expect(input.readState()).toEqual({
      axes: [0.5, -0.25, 0, 1],
      buttons: [true, false]
    });
    currentGamepads = [{ connected: false, mapping: "standard", axes: [], buttons: [] }];
    expect(input.readState()).toBeNull();
    expect(createBrowserPhoneVrGamepadInputPort({}).readState()).toBeNull();
    expect(createBrowserPhoneVrGamepadInputPort({ getGamepads: () => { throw new Error("blocked"); } }).readState()).toBeNull();
  });
});

describe("Phone VR presentation backend", () => {
  it("requests permission synchronously and enables stereo only after a valid sensor event", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    expect(await backend.checkAvailability()).toEqual(sensors.availability);
    const permissionRequest = backend.requestPermissionFromUserGesture();
    expect(sensors.permissionRequestCount).toBe(1);
    expect(await permissionRequest).toEqual({ ok: true });

    let started = false;
    const startup = backend.start().then(() => { started = true; });
    expect(renderer.stereoProfile).toEqual(PHONE_VR_OPTICAL_PROFILE);
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    expect(started).toBe(true);
    expect(backend.currentFrame(110, createSceneFixture("Title")).panelVisible).toBe(true);
    await backend.stop();
    expect(renderer.stereoProfile).toBeNull();
    expect(sensors.stopCount).toBe(1);
  });

  it("rejects denied permissions without starting sensors or stereo", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    sensors.permissionResult = { ok: false, message: "Phone VR sensor permission denied" };
    const backend = createBackend(sensors, renderer);
    expect(await backend.requestPermissionFromUserGesture()).toEqual(sensors.permissionResult);
    await expect(backend.start()).rejects.toThrow("explicit user permission action");
    expect(sensors.startCount).toBe(0);
    expect(renderer.stereoProfile).toBeNull();
  });

  it("cancels a startup that is waiting for its first sensor sample", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    await backend.cancelPendingRequest();
    await expect(startup).rejects.toThrow("canceled");
    expect(renderer.stereoProfile).toBeNull();
    expect(sensors.stopCount).toBe(2);
  });

  it("restores Screen presentation after missing, null, or stale tracking", async () => {
    const timeoutSensors = new FakePhoneVrSensors();
    const timeoutRenderer = new FakeRenderer();
    const timeoutBackend = createBackend(timeoutSensors, timeoutRenderer, [], 2);
    await timeoutBackend.requestPermissionFromUserGesture();
    await expect(timeoutBackend.start()).rejects.toThrow("before timeout");
    expect(timeoutRenderer.stereoProfile).toBeNull();
    expect(timeoutSensors.stopCount).toBe(1);

    const nullSensors = new FakePhoneVrSensors();
    const nullRenderer = new FakeRenderer();
    const nullBackend = createBackend(nullSensors, nullRenderer);
    await nullBackend.requestPermissionFromUserGesture();
    const nullStartup = nullBackend.start();
    nullSensors.emit({ ...reading(0, 0, 0, 100), gamma: null });
    await expect(nullStartup).rejects.toThrow("null or non-finite");
    expect(nullRenderer.stereoProfile).toBeNull();

    const staleSensors = new FakePhoneVrSensors();
    const staleRenderer = new FakeRenderer();
    const unavailable: string[] = [];
    const staleBackend = createBackend(staleSensors, staleRenderer, unavailable, 3000, 50);
    await staleBackend.requestPermissionFromUserGesture();
    const staleStartup = staleBackend.start();
    staleSensors.emit(reading(0, 0, 0, 100));
    await staleStartup;
    expect(staleBackend.currentFrame(151, createSceneFixture("Title")).panelVisible).toBe(false);
    expect(unavailable).toEqual(["Phone VR orientation data is stale"]);
    expect(staleRenderer.stereoProfile).toBeNull();
    await staleBackend.stop();
  });

  it("renders and gaze-activates common controls in every scene and overlay", async () => {
    let timestampMs = 100;
    for (const scene of GAME_SCENES) {
      for (const overlay of [null, ...SCENE_FIXTURE_OVERLAYS[scene]]) {
        const sensors = new FakePhoneVrSensors();
        const renderer = new FakeRenderer();
        const actions: UiAction[] = [];
        const backend = new PhoneVrPresentationBackend(
          sensors, renderer, viewport, (action) => { actions.push(action); }, () => undefined,
          { opticalProfile: PHONE_VR_OPTICAL_PROFILE, firstSampleTimeoutMs: 3000, staleAfterMs: 60_000 }
        );
        await backend.requestPermissionFromUserGesture();
        const startup = backend.start();
        sensors.emit(reading(0, 0, 0, timestampMs));
        await startup;
        const viewModel = withMenuAnchors(createSceneFixture(scene, overlay));
        const frame = backend.currentFrame(timestampMs + 1, viewModel);
        expect(frame.panelVisible).toBe(true);
        expect(frame.panel?.id).toBe(`${scene.toLowerCase()}-panel`);
        sensors.emit(reading(0, -10.2, 0, timestampMs + 2));
        backend.currentFrame(timestampMs + 2, viewModel);
        const activatedFrame = backend.currentFrame(timestampMs + 1002, viewModel);
        expect(activatedFrame.gazeCursor?.progress).toBe(1);
        if (!actions.some((action) => action.type === "activate" && action.controlId === `${scene.toLowerCase()}-action`)) {
          throw new Error(`Missing gaze activation for ${scene}/${String(overlay)} at ${String(timestampMs)}`);
        }
        await backend.stop();
        timestampMs += 2000;
      }
    }
  });

  it("re-centers tracking continuously and applies screen-orientation changes", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    const initial = backend.currentFrame(110, createSceneFixture("Title"));
    sensors.emit(reading(0, 20, 0, 120));
    const turned = backend.currentFrame(120, createSceneFixture("Title"));
    expect(turned.cameraPose.orientation).not.toEqual(initial.cameraPose.orientation);
    backend.recenterTracking();
    const recentered = backend.currentFrame(121, createSceneFixture("Title"));
    expect(recentered.cameraPose.orientation).toEqual(IDENTITY_POSE.orientation);
    sensors.changeScreenOrientation(90);
    const rotated = backend.currentFrame(122, createSceneFixture("Title"));
    expect(rotated.cameraPose.orientation).not.toEqual(IDENTITY_POSE.orientation);
    await backend.stop();
  });

  it("keeps Menu fixed, makes Head follow tracking, and maps World/Cockpit to the tracking reference", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    const menuBefore = backend.currentFrame(110, { ...createSceneFixture("Title"), panels: [createAnchorFixture("menu")] });
    sensors.emit(reading(0, 20, 0, 120));
    const menuAfter = backend.currentFrame(120, { ...createSceneFixture("Title"), panels: [createAnchorFixture("menu")] });
    const head = backend.currentFrame(121, { ...createSceneFixture("Title"), panels: [createAnchorFixture("head")] });
    const world = backend.currentFrame(122, { ...createSceneFixture("Title"), panels: [createAnchorFixture("world")] });
    const cockpit = backend.currentFrame(123, { ...createSceneFixture("Flight"), panels: [createAnchorFixture("cockpit")] });
    expect(menuAfter.panelPose).toEqual(menuBefore.panelPose);
    expect(head.panelPose.orientation).toEqual(head.cameraPose.orientation);
    expect(world.panelPose).toEqual(cockpit.panelPose);
    await backend.stop();
  });

  it("uses a connected gamepad for focus, selection, back, and scrolling", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const gamepad = new FakePhoneVrGamepad();
    const actions: UiAction[] = [];
    const backend = new PhoneVrPresentationBackend(
      sensors, renderer, viewport, (action) => { actions.push(action); }, () => undefined,
      { gamepadInput: gamepad }
    );
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    const viewModel = withMenuAnchors(createSceneFixture("Title"));
    backend.currentFrame(110, viewModel);
    gamepad.state = gamepadState([0, 0, 0, 0], [false, false]);
    backend.currentFrame(120, viewModel);
    for (let frame = 1; frame <= 13; frame++) {
      gamepad.state = gamepadState([0, 1, 0, 0], [false, false]);
      backend.currentFrame(120 + frame * 16, viewModel);
    }
    gamepad.state = gamepadState([0, 0, 0, 0], [true, false]);
    const selected = backend.currentFrame(344, viewModel);
    expect(selected.gazeCursor?.point.y).toBeCloseTo(-0.449, 2);
    gamepad.state = gamepadState([0, 0, 0, 0.5], [false, true]);
    backend.currentFrame(360, viewModel);
    expect(actions).toContainEqual({ type: "activate", controlId: "title-action" });
    expect(actions).toContainEqual({ type: "back" });
    expectScrollAction(actions);
    await backend.stop();
  });
});

class FakePhoneVrSensors implements PhoneVrSensorPort {
  availability: PhoneVrAvailability = { supported: true, message: "Phone VR sensor API available" };
  permissionResult: PhoneVrPermissionResult = { ok: true };
  permissionRequestCount = 0;
  startCount = 0;
  stopCount = 0;
  private readingHandler: ((value: PhoneVrSensorReading) => void) | null = null;
  private screenHandler: ((angle: number | null) => void) | null = null;
  private screenAngle: number | null = 0;

  checkAvailability(): Promise<PhoneVrAvailability> {
    return Promise.resolve(this.availability);
  }

  requestPermissionFromUserGesture(): Promise<PhoneVrPermissionResult> {
    this.permissionRequestCount++;
    return Promise.resolve(this.permissionResult);
  }

  getScreenOrientationAngle(): number | null {
    return this.screenAngle;
  }

  startListening(
    onReading: (value: PhoneVrSensorReading) => void,
    onScreenOrientationChange: (angle: number | null) => void
  ): void {
    this.startCount++;
    this.readingHandler = onReading;
    this.screenHandler = onScreenOrientationChange;
  }

  stopListening(): void {
    this.stopCount++;
    this.readingHandler = null;
    this.screenHandler = null;
  }

  emit(value: PhoneVrSensorReading): void {
    this.readingHandler?.(value);
  }

  changeScreenOrientation(angle: number | null): void {
    this.screenAngle = angle;
    this.screenHandler?.(angle);
  }
}

class FakeRenderer implements RendererAdapter {
  stereoProfile: StereoPresentationProfile | null = null;

  startLoop(): void {}
  stopLoop(): void {}
  render(): void {}
  setFlightPose(): void {}
  resize(): void {}
  setStereoPresentation(profile: StereoPresentationProfile | null): void { this.stereoProfile = profile; }
  setSelectRayHandler(): void {}
  dispose(): void {}
}

class FakePhoneVrGamepad implements PhoneVrGamepadInputPort {
  state = gamepadState([0, 0, 0, 0], [false, false]);

  readState() {
    return this.state;
  }
}

function createBackend(
  sensors: FakePhoneVrSensors,
  renderer: FakeRenderer,
  unavailable: string[] = [],
  firstSampleTimeoutMs = 3000,
  staleAfterMs = 1000
): PhoneVrPresentationBackend {
  return new PhoneVrPresentationBackend(
    sensors, renderer, viewport, () => undefined,
    (message) => { unavailable.push(message); },
    { opticalProfile: PHONE_VR_OPTICAL_PROFILE, firstSampleTimeoutMs, staleAfterMs }
  );
}

function withMenuAnchors(viewModel: UiViewModel): UiViewModel {
  return Object.freeze({
    ...viewModel,
    panels: Object.freeze(viewModel.panels.map((panel) => Object.freeze({ ...panel, anchor: "menu" as const })))
  });
}

function viewport(): ViewportSize {
  return Object.freeze({ x: 1280, y: 800, pixelRatio: 1 });
}

function reading(alpha: number, beta: number, gamma: number, timestampMs = 100): PhoneVrSensorReading {
  return Object.freeze({ alpha, beta, gamma, timestampMs });
}

function gamepadState(axes: readonly number[], buttons: readonly boolean[]) {
  return Object.freeze({ axes, buttons });
}

function expectScrollAction(actions: readonly UiAction[]): void {
  const scrollAction = actions.find((action) => action.type === "scroll");
  expect(scrollAction?.type).toBe("scroll");
  if (scrollAction?.type === "scroll") {
    expect(scrollAction.deltaX).toBe(0);
    expect(scrollAction.deltaY).toBeCloseTo(4.32);
  }
}

function axisQuaternion(axis: "x" | "y" | "z", degrees: number) {
  const halfAngle = degrees * Math.PI / 360;
  const sine = Math.sin(halfAngle);
  return quaternion(
    Math.cos(halfAngle),
    axis === "x" ? sine : 0,
    axis === "y" ? sine : 0,
    axis === "z" ? sine : 0
  );
}

function createMockBrowserWindow(options: {
  readonly secure?: boolean;
  readonly orientationApi?: boolean;
  readonly screenOrientationApi?: boolean;
} = {}) {
  const windowListeners = new Map<string, (event: unknown) => void>();
  const orientationListeners = new Map<string, () => void>();
  const permissionRequests: boolean[] = [];
  let screenAngle = 0;
  const orientationConstructor = options.orientationApi === false ? undefined : Object.assign(
    function MockDeviceOrientationEvent() {},
    {
      requestPermission: (absolute?: boolean): Promise<"granted" | "denied" | "prompt"> => {
        permissionRequests.push(absolute ?? false);
        return Promise.resolve("granted");
      }
    }
  );
  const screenOrientation = options.screenOrientationApi === false ? undefined : {
    get angle() { return screenAngle; },
    addEventListener: (type: string, listener: () => void) => { orientationListeners.set(type, listener); },
    removeEventListener: (type: string) => { orientationListeners.delete(type); }
  };
  const fakeWindow = {
    isSecureContext: options.secure ?? true,
    DeviceOrientationEvent: orientationConstructor,
    screen: { orientation: screenOrientation },
    addEventListener: (type: string, listener: unknown) => {
      windowListeners.set(type, listener as (event: unknown) => void);
    },
    removeEventListener: (type: string) => { windowListeners.delete(type); }
  };
  return {
    window: fakeWindow as Parameters<typeof createBrowserPhoneVrSensorPort>[0],
    permissionRequests,
    emitDeviceOrientation(value: PhoneVrSensorReading) {
      windowListeners.get("deviceorientation")?.({
        alpha: value.alpha, beta: value.beta, gamma: value.gamma, timeStamp: value.timestampMs
      });
    },
    emitScreenOrientationChange() { orientationListeners.get("change")?.(); },
    setScreenAngle(value: number) { screenAngle = value; }
  };
}
