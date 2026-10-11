import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { Window as BrowserWindow } from "happy-dom";
import { TailPersonalBestSelectionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { BrowserTailPilotInput } from "../../web/src/game/browser-tail-input.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import { FlightControllerUiBindings } from "../../web/src/app/flight-controller-port.js";
import { viewExposesAction } from "../../web/src/render/contracts/ui.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppMessage } from "../../web/src/app/app-state.js";
import { projectTailFlightSnapshot } from "../../web/src/game/flight-display-snapshot.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";

beforeAll(() => {
  const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
});

it("clears a DOM pilot target across terminal Result and Rust Retry", () => {
  const window = new BrowserWindow();
  vi.stubGlobal("HTMLElement", window.HTMLElement);
  const input = new BrowserTailPilotInput(window as unknown as Window);
  const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
  let controller: TailFlightController | null = null;
  try {
    launchCurrentSession(session);
    controller = new TailFlightController(session.flightPort, input, { setFlightPose() {} }, {
      render() {}, setVisible() {}, fail(message) { throw new Error(message); }
    }, physics_hz(), () => []);
    window.dispatchEvent(new window.KeyboardEvent("keydown", { code: "KeyL" }));
    controller.onFrame(0);
    controller.onFrame(100);
    window.dispatchEvent(new window.KeyboardEvent("keyup", { code: "KeyL" }));
    expect(controller.currentSnapshot.frame.state.pilotPositionTargetNormalized).toBeGreaterThan(0.03);
    session.executeOperation("abort");
    controller.reset(session.flightPort.snapshot_json());
    const resultPress = new window.KeyboardEvent("keydown", { code: "KeyL", cancelable: true });
    window.dispatchEvent(resultPress);
    expect(resultPress.defaultPrevented).toBe(false);
    session.executeOperation("retry");
    session.executeOperation("start-flight");
    while (session.advanceCountdown() > 0) continue;
    session.launch();
    controller.reset(session.flightPort.snapshot_json());
    controller.onFrame(1000);
    controller.onFrame(1010);
    controller.onFrame(1020);
    expect(controller.currentSnapshot.frame.state.pilotPositionTargetNormalized).toBeCloseTo(0, 12);
    expect(session.readLifecycle().phaseCode).toBe(5);
  } finally {
    if (controller === null) { input.dispose(); session.dispose(); }
    else controller.dispose();
    vi.unstubAllGlobals();
  }
});

describe("current WebAssembly browser binding", () => {
  it.each(["renderer", "hud", "input-suspend"] as const)("publishes Rust Result despite terminal %s failure", (port) => {
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
    const display = { render: (snapshot: FlightDisplaySnapshot): void => {
      if (port === "hud" && snapshot.kind === "tail_result") throw failure;
    }, setVisible: (): void => undefined };
    const bindings = new FlightControllerUiBindings();
    const binding = bindings.bindAppDisplay(session.flightPort, display,
      () => session.readGameSessionProjection(), () => {
        const snapshot = projectTailFlightSnapshot(session.readSnapshot());
        if (snapshot.kind !== "available") throw new Error("Expected flight display");
        return snapshot.value;
      }, dispatch);
    const input = {
      reset: (): void => undefined, resume: (): void => undefined, dispose: (): void => undefined,
      suspend: (): void => { if (port === "input-suspend" && session.readLifecycle().phaseCode === 7) throw failure; },
      readDemand: () => ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" as const } })
    };
    dispatch({ type: "flight-controller-ready", identity: binding.identity });
    const terminalRecords: string[] = [];
    const onTerminal = vi.fn((snapshot: Parameters<typeof binding.onTerminal>[0]): void => {
      terminalRecords.push(session.exportRecordJson());
      binding.onTerminal(snapshot);
    });
    const controller = new TailFlightController(session.flightPort, input, {
      setFlightPose: (pose): void => { if (pose !== null && port === "renderer" && session.readLifecycle().phaseCode === 7) throw failure; }
    }, binding.port, physics_hz(), () => [], onTerminal);
    try {
      controller.onFrame(0);
      for (let frame = 1; frame <= 4_000 && controller.currentSnapshot.phaseCode === 5; frame += 1) controller.onFrame(frame * 10);
      const terminal = controller.currentSnapshot;
      expect(terminal.phaseCode).toBe(7);
      if (terminal.frame.kind !== "result") throw new Error("Expected Result frame");
      expect(terminal.frame.finalization.reason).toBe("water_contact");
      expect(onTerminal).toHaveBeenCalledExactlyOnceWith(terminal);
      expect(model.gameSession.kind).toBe("result");
      expect(model.flightExecution.kind).toBe("ready");
      expect(effects).toEqual(expect.arrayContaining([expect.objectContaining({ type: "persist-flight-record" })]));
      expect(terminalRecords).toEqual([session.exportRecordJson()]);
      for (const mode of ["screen", "phone-vr", "webxr"] as const) {
        const view = createGameViewModel({ ...model, presentation: { type: "ready", mode } }, controller.currentDisplaySnapshot);
        expect(view.scene).toBe("Result");
        for (const controlId of ["game-flight-pause", "game-flight-abort", "game-flight-resume", "game-paused-abort"]) {
          expect(viewExposesAction(view, { type: "activate", controlId })).toBe(false);
        }
      }
      const retained = session.exportRecordJson();
      controller.resume();
      controller.onFrame(50_000);
      controller.onFrame(60_000);
      expect(onTerminal).toHaveBeenCalledTimes(1);
      expect(session.exportRecordJson()).toBe(retained);
    } finally { controller.dispose(); }
  });

  it.each([0, 1, 2] as const)("runs control mode %i through pause, fractional contact, record selection and retry", (mode) => {
    const session = createAppSession({ controlModeCode: mode, seedLow: 0, seedHigh: 0 });
    try {
      launchCurrentSession(session);
      session.executeOperation("pause");
      expect(session.readLifecycle().phaseCode).toBe(6);
      session.executeOperation("resume");
      for (let tick = 0; tick < 4_000 && session.readLifecycle().phaseCode === 5; tick += 1) {
        session.flightPort.advance_tick_json(neutralTailInput);
      }
      const snapshot = session.readSnapshot();
      if (snapshot.frame.kind !== "result") throw new Error("Expected fractional Result");
      expect(snapshot.frame.finalization.reason).toBe("water_contact");
      expect(snapshot.frame.finalization.terminalFraction).toBeGreaterThan(0);
      expect(snapshot.frame.finalization.terminalFraction).toBeLessThan(1);
      expect(snapshot.frame.finalization.scoreMeters?.[0]).toBeGreaterThan(150);
      const encoded = session.exportRecordJson();
      const document = JSON.parse(encoded) as { schema_version: number; header: { personal_best_key: number[] } };
      expect(document.schema_version).toBe(6);
      expect(document.header.personal_best_key).toHaveLength(32);
      const selection = new TailPersonalBestSelectionBridge(encoded);
      try {
        expect(selection.is_eligible()).toBe(true);
        expect(selection.key_hex()).toMatch(/^[0-9a-f]{64}$/);
        selection.consider_existing(12, encoded);
        expect(selection.selected_existing_id()).toBe(12);
        expect(() => { selection.consider_existing(0, encoded); }).toThrow(/InvalidRecord/);
      } finally { selection.free(); }
      session.executeOperation("retry");
      expect(session.readLifecycle().phaseCode).toBe(3);
    } finally { session.dispose(); }
  });

  it("keeps simulation outcomes independent of 30, 60 and 120 FPS rendering", () => {
    const results: string[] = [];
    for (const fps of [30, 60, 120]) {
      const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
      launchCurrentSession(session);
      const input = { reset() {}, resume() {}, suspend() {}, dispose() {},
        readDemand: () => ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" as const } }) };
      const controller = new TailFlightController(session.flightPort, input, { setFlightPose() {} }, {
        render() {}, setVisible() {}, fail(message) { throw new Error(message); }
      }, physics_hz(), () => []);
      try {
        controller.onFrame(0);
        for (let frame = 1; frame <= fps * 40 && controller.currentSnapshot.phaseCode === 5; frame += 1) {
          controller.onFrame(frame * 1_000 / fps);
        }
        expect(controller.currentSnapshot.phaseCode).toBe(7);
        results.push(session.exportRecordJson());
      } finally { controller.dispose(); }
    }
    expect(results[1]).toBe(results[0]);
    expect(results[2]).toBe(results[0]);
  });

  it("routes neutral-confirmed gamepad demand through the current WASM controller", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: 0 });
    launchCurrentSession(session);
    const target = { addEventListener() {}, removeEventListener() {} } as unknown as Window;
    const input = new BrowserTailPilotInput(target);
    const neutral = { connected: true, index: 0, axes: [0, 0, 0, 0] } as unknown as Gamepad;
    const active = { connected: true, index: 0, axes: [0.18, -0.18, 0, 0.18] } as unknown as Gamepad;
    let gamepads: readonly Gamepad[] = [neutral];
    const controller = new TailFlightController(session.flightPort, input, { setFlightPose() {} }, {
      render() {}, setVisible() {}, fail(message) { throw new Error(message); }
    }, physics_hz(), () => gamepads);
    try {
      controller.onFrame(0);
      controller.onFrame(10);
      controller.onFrame(20);
      gamepads = [active];
      controller.onFrame(30);
      controller.onFrame(40);
      const state = controller.currentSnapshot.frame.state;
      expect(state.pilotPositionTargetNormalized).toBeLessThan(0);
      expect(state.physicalIncidence.horizontalTailRadians).not.toBe(0);
      expect(state.physicalIncidence.verticalTailRadians).not.toBe(0);
    } finally { controller.dispose(); }
  });

  it("plays an independent Title demonstration without exporting it as a player flight", () => {
    const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
    try {
      const selection = session.readDifficulty();
      session.executeOperation("enter-attract");
      expect(session.readLifecycle().phaseCode).toBe(10);
      expect(session.readPlaybackContext().phase).toBe("attract");
      expect(session.queryRecordSample(0).controls.layout).toBe("tail_incidence");
      expect(() => session.exportRecordJson()).toThrow();
      session.executeOperation("leave-attract");
      expect(session.readLifecycle().phaseCode).toBe(0);
      expect(session.readDifficulty()).toEqual(selection);
    } finally { session.dispose(); }
  });
});
