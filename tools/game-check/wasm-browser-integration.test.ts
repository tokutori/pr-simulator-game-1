import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { Window as BrowserWindow } from "happy-dom";
import { TailPersonalBestSelectionBridge, initSync, physics_hz, screen_vertical_field_of_view_degrees } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { BrowserFlightInput } from "../../web/src/game/browser-flight-input.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { viewExposesAction } from "../../web/src/render/contracts/ui.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppMessage } from "../../web/src/app/app-state.js";
import type { TailControllerSnapshot } from "../../web/src/game/tail-flight-controller.js";
import { launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";

beforeAll(() => {
  const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
});

it("exposes the shared Screen presentation field of view without changing the physics frequency", () => {
  expect(screen_vertical_field_of_view_degrees()).toBe(60);
  expect(physics_hz()).toBe(100);
});

it("clears a DOM pilot target across terminal Result and Rust Retry", () => {
  const window = new BrowserWindow();
  vi.stubGlobal("HTMLElement", window.HTMLElement);
  const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
  const input = new BrowserFlightInput(window as unknown as Window, session.flightPort, () => []);
  let controller: TailFlightController | null = null;
  try {
    launchCurrentSession(session);
    controller = new TailFlightController(session.flightPort, input, { setFlightPose() {} }, {
      render() {}, setVisible() {}, fail(message) { throw new Error(message); }
    });
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyL" }));
    controller.onFrame(0);
    controller.onFrame(100);
    window.dispatchEvent(new window.KeyboardEvent("keyup", { code: "KeyL" }));
    expect(controller.currentSnapshot.frame.state.pilot_position_target_normalized).toBeGreaterThan(0.03);
    session.executeSharedSessionCommand({ kind: "abort" });
    controller.reset(session.flightPort.readSnapshot());
    const resultPress = new window.KeyboardEvent("keydown", { code: "KeyL", cancelable: true });
    window.dispatchEvent(resultPress);
    expect(resultPress.defaultPrevented).toBe(false);
    session.executeSharedSessionCommand({ kind: "retry" });
    session.executeSharedSessionCommand({ kind: "start_countdown" });
    session.observeCountdown(session.captureCountdownToken(), 3, true);
    controller.reset(session.flightPort.readSnapshot());
    controller.onFrame(1000);
    controller.onFrame(1010);
    controller.onFrame(1020);
    expect(controller.currentSnapshot.frame.state.pilot_position_target_normalized).toBeCloseTo(0, 12);
    expect(session.readLifecycle().phase).toBe("flight_running");
  } finally {
    if (controller === null) { input.dispose(); session.dispose(); }
    else controller.dispose();
    vi.unstubAllGlobals();
  }
});

describe("current WebAssembly browser binding", () => {
  it.each(["renderer", "hud"] as const)("publishes Rust Result despite terminal %s failure", (port) => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    launchCurrentSession(session);
    let model = updateApp(createInitialAppModel(), { type: "game-session-synced", ...session.readGameSessionProjection() }).model;
    const effects: AppEffect[] = [];
    const dispatch = (message: AppMessage): void => {
      const transition = updateApp(model, message);
      model = transition.model;
      effects.push(...transition.effects);
    };
    const failure = new Error(`Injected terminal ${port} failure`);
    const display = { render: (snapshot: TailControllerSnapshot): void => {
      if (port === "hud" && snapshot.phase === "result") throw failure;
    }, setVisible: (): void => undefined };
    const bindings = new FlightControllerUiBindings();
    const binding = bindings.bindAppDisplay(session.flightPort, display,
      () => session.readGameSessionProjection(), () => controller.currentSnapshot, dispatch);
    const input = { readGamepads: () => [], dispose() {} };
    dispatch({ type: "flight-controller-ready", identity: binding.identity });
    const terminalRecords: string[] = [];
    const onTerminal = vi.fn((snapshot: Parameters<typeof binding.onTerminal>[0]): void => {
      terminalRecords.push(session.exportRecordJson());
      binding.onTerminal(snapshot);
    });
    const controller = new TailFlightController(session.flightPort, input, {
      setFlightPose: (pose): void => { if (pose !== null && port === "renderer" && session.readLifecycle().phase === "result") throw failure; }
    }, binding.port, onTerminal);
    try {
      controller.onFrame(0);
      for (let frame = 1; frame <= 4_000 && controller.currentSnapshot.phase === "flight_running"; frame += 1) controller.onFrame(frame * 10);
      const terminal = controller.currentSnapshot;
      expect(terminal.phase).toBe("result");
      if (terminal.frame.kind !== "result") throw new Error("Expected Result frame");
      expect(terminal.frame.finalization.reason).toBe("water_contact");
      expect(onTerminal).toHaveBeenCalledExactlyOnceWith(terminal);
      expect(model.gameSession.phase).toBe("result");
      expect(model.flightExecution.kind).toBe("ready");
      expect(effects).toEqual(expect.arrayContaining([expect.objectContaining({ type: "persist-flight-record" })]));
      expect(terminalRecords).toEqual([session.exportRecordJson()]);
      for (const mode of ["screen", "phone-vr", "webxr"] as const) {
        const view = createGameViewModel(session.readSharedUi(sharedGameUiPlatform({
          presentation_mode: mode === "phone-vr" ? "phone_vr" : mode
        })));
        expect(view.scene).toBe("Result");
        for (const controlId of ["game-flight-pause", "game-flight-abort", "game-flight-resume", "game-paused-abort"]) {
          expect(viewExposesAction(view, { type: "activate", controlId })).toBe(false);
        }
      }
      const retained = session.exportRecordJson();
      controller.onFrame(50_000);
      controller.onFrame(60_000);
      expect(onTerminal).toHaveBeenCalledTimes(1);
      expect(session.exportRecordJson()).toBe(retained);
    } finally { controller.dispose(); }
  });

  it.each([0, 1, 2] as const)("runs control mode %i through a calm flight, fractional contact, record selection and retry", (mode) => {
    const session = createAppSession({ controlModeCode: mode, seedLow: 0, seedHigh: 0 });
    try {
      session.executeSharedSessionCommand({ kind: "open_setup" });
      session.executeSharedSessionCommand({ kind: "select_weather", weather: "calm" });
      session.executeSharedSessionCommand({ kind: "prepare" });
      session.executeSharedSessionCommand({ kind: "start_countdown" });
      session.observeCountdown(session.captureCountdownToken(), 3, true);
      session.executeSharedSessionCommand({ kind: "pause" });
      expect(session.readLifecycle().phase).toBe("flight_paused");
      session.executeSharedSessionCommand({ kind: "resume" });
      for (let tick = 0; tick < 4_000 && session.readLifecycle().phase === "flight_running"; tick += 1) {
        session.flightPort.advanceTick(neutralTailInput);
      }
      const snapshot = session.readSnapshot();
      if (snapshot.frame.kind !== "result") throw new Error("Expected fractional Result");
      expect(snapshot.scenario).toMatchObject({ environment_version: 1 });
      expect(snapshot.frame.finalization.reason).toBe("water_contact");
      expect(snapshot.frame.finalization.terminal_fraction).toBeGreaterThan(0);
      expect(snapshot.frame.finalization.terminal_fraction).toBeLessThan(1);
      expect(snapshot.frame.finalization.score_m?.[0]).toBeGreaterThan(150);
      const encoded = session.exportRecordJson();
      const document = JSON.parse(encoded) as { schema_version: number; header: { personal_best_key: number[] } };
      expect(document.schema_version).toBe(7);
      expect(document.header.personal_best_key).toHaveLength(32);
      const pose = snapshot.render_pose;
      if (pose === null) throw new Error("Result requires an accepted render pose");
      const terminalSample = session.queryRecordSample(pose.simulationTimeSeconds);
      expect(pose.wingDeformation).toEqual({ kind: "accepted", factor: terminalSample.wing_bending_factor });
      session.executeSharedSessionCommand({ kind: "enter_replay" });
      expect(session.queryReplayCamera(pose.simulationTimeSeconds, "chase", true).wingDeformation).toEqual(pose.wingDeformation);
      session.executeSharedSessionCommand({ kind: "leave_replay" });
      const selection = new TailPersonalBestSelectionBridge(encoded);
      try {
        expect(selection.is_eligible()).toBe(true);
        expect(selection.key_hex()).toMatch(/^[0-9a-f]{64}$/);
        selection.consider_existing(12, encoded);
        expect(selection.selected_existing_id()).toBe(12);
        expect(() => { selection.consider_existing(0, encoded); }).toThrow(/InvalidRecord/);
      } finally { selection.free(); }
      session.executeSharedSessionCommand({ kind: "retry" });
      expect(session.readLifecycle().phase).toBe("briefing_ready");
    } finally { session.dispose(); }
  });

  describe("rendering frame-rate independence", () => {
    let referenceRecord: string;
    beforeAll(() => { referenceRecord = recordedFlightAtFrameRate(30); });

    it.each([60, 120])("preserves the 30 FPS outcome at %i FPS", (fps) => {
      expect(recordedFlightAtFrameRate(fps)).toBe(referenceRecord);
    });

    function recordedFlightAtFrameRate(fps: number): string {
      const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
      launchCurrentSession(session);
      const input = { readGamepads: () => [], dispose() {} };
      const controller = new TailFlightController(session.flightPort, input, { setFlightPose() {} }, {
        render() {}, setVisible() {}, fail(message) { throw new Error(message); }
      });
      try {
        controller.onFrame(0);
        for (let frame = 1; frame <= fps * 40 && controller.currentSnapshot.phase === "flight_running"; frame += 1) {
          controller.onFrame(frame * 1_000 / fps);
        }
        expect(controller.currentSnapshot.phase).toBe("result");
        return session.exportRecordJson();
      } finally { controller.dispose(); }
    }
  });

  it("routes neutral-confirmed gamepad demand through the current WASM controller", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    launchCurrentSession(session);
    const target = { addEventListener() {}, removeEventListener() {} } as unknown as Window;
    const neutral = { connected: true, index: 0, axes: [0, 0, 0, 0] } as unknown as Gamepad;
    const active = { connected: true, index: 0, axes: [0.18, -0.18, 0, 0.18] } as unknown as Gamepad;
    let gamepads: readonly Gamepad[] = [neutral];
    const input = new BrowserFlightInput(target, session.flightPort, () => gamepads);
    const controller = new TailFlightController(session.flightPort, input, { setFlightPose() {} }, {
      render() {}, setVisible() {}, fail(message) { throw new Error(message); }
    });
    try {
      controller.onFrame(0);
      controller.onFrame(10);
      controller.onFrame(20);
      gamepads = [active];
      controller.onFrame(30);
      controller.onFrame(40);
      const state = controller.currentSnapshot.frame.state;
      expect(state.pilot_position_target_normalized).toBeLessThan(0);
      expect(state.physical_incidence.horizontal_tail_rad).not.toBe(0);
      expect(state.physical_incidence.vertical_tail_rad).not.toBe(0);
    } finally { controller.dispose(); }
  });

  it("plays an independent Title demonstration without exporting it as a player flight", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      const selection = session.readEnvironment();
      session.executeSharedSessionCommand({ kind: "enter_attract" });
      expect(session.readLifecycle().phase).toBe("attract");
      expect(session.readPlaybackContext().phase).toBe("attract");
      expect(session.queryRecordSample(0).controls.layout).toBe("tail_incidence");
      expect(() => session.exportRecordJson()).toThrow();
      session.executeSharedSessionCommand({ kind: "leave_attract" });
      expect(session.readLifecycle().phase).toBe("title");
      expect(session.readEnvironment()).toEqual(selection);
    } finally { session.dispose(); }
  });
});
