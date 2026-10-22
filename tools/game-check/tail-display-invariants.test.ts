import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import type { TailFlightHudPort, TailSessionPort } from "../../web/src/game/tail-flight-controller.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { AnalysisDocument } from "../../web/src/generated/record-analysis.js";
import { readContract } from "../game-ui-build/validate.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";
import { createSceneFixture } from "../presentation-check/ui-adapter-fixture.js";
import { IDENTITY_POSE, pose, vec3 } from "../../web/src/render/contracts/math.js";
import { NO_HEAD_HUD } from "../../web/src/render/contracts/head-hud.js";
import type { BackendFrame, PresentationBackendAdapter, PresentationMode, RendererAdapter } from "../../web/src/render/contracts/runtime.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

const TERMINAL_TICK = 100;
const INFORMATION_NAMES = ["full", "standard", "minimal", "realistic", "custom"] as const;
const BACKENDS: readonly PresentationMode[] = ["screen", "webxr", "phone-vr"];
const CASES = INFORMATION_NAMES.flatMap((information, informationCode) => BACKENDS.flatMap((backend) =>
  [30, 60, 120].map((fps) => ({ information, informationCode, backend, fps }))));

function mockSpatialBackend(mode: "webxr" | "phone-vr"): PresentationBackendAdapter {
  return {
    mode, start: () => Promise.resolve(), stop: () => Promise.resolve(),
    currentFrame(timestampMs): BackendFrame {
      return { timestampMs, headHud: NO_HEAD_HUD, panel: { kind: "absent" },
        cameraPose: pose(vec3(mode === "webxr" ? 0.2 : -0.2, 1.6, 0), IDENTITY_POSE.orientation),
        viewport: { x: mode === "webxr" ? 1440 : 960, y: 800, pixelRatio: 1 } };
    }
  };
}

function recordObject(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("Expected a saved record object");
  return value as Record<string, unknown>;
}

async function runInvariantFlight(mode: number, informationCode: number, backend: PresentationMode, fps: number) {
  const informationName = INFORMATION_NAMES[informationCode];
  if (informationName === undefined) throw new RangeError("Unsupported Information code");
  const session = new HybridGameSessionBridge(mode, 11, 12);
  let controller: TailFlightController | null = null;
  let runtime: PresentationRuntime | null = null;
  try {
    expect(physics_hz()).toBe(100);
    session.open_setup();
    session.set_information_level(informationCode);
    if (informationCode === 4) {
      session.set_information_cue(0, false);
      session.set_information_cue(3, false);
    }
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.observe_countdown(session.countdown_token(), 1, true);
    const facade = new TailAppSessionFacade(session, physics_hz());
    const initial = facade.readSnapshot();
    if (initial.phase !== "flight_running" || initial.frame.kind !== "flight") throw new Error("Expected a prepared flight");
    expect(initial.scenario).toMatchObject({ catalog_version: 8, scenario_version: 8,
      aircraft_model_version: 5, controller_profile_version: 3, environment_version: 6, seed_low: 11, seed_high: 12 });
    expect(initial.control_identity).toMatchObject({ aircraft_configuration_id: "bpg041-playable-hybrid-mock" });
    const port: TailSessionPort = { ...facade.flightPort, free: () => undefined,
      advanceFrame(timestamp, gamepads, available) {
        const report = facade.flightPort.advanceFrame(timestamp, gamepads, available);
        if (timestamp !== 1000) return report;
        facade.executeSharedSessionCommand({ kind: "abort" });
        const snapshot = facade.readSnapshot();
        return { ...report, phase_changed: true, snapshot, render_pose: snapshot.render_pose };
      }
    };
    const input = { readGamepads: () => [], dispose: vi.fn() };
    const evidence: { loop: Parameters<RendererAdapter["startLoop"]>[0] | null } = { loop: null };
    const renderFrame = vi.fn<RendererAdapter["render"]>();
    const setFlightPose = vi.fn<RendererAdapter["setFlightPose"]>();
    const renderer: RendererAdapter = {
      startLoop: (callback) => { evidence.loop = callback; }, beginViewFrame: vi.fn(),
      stopLoop: () => { evidence.loop = null; }, render: renderFrame,
      setFlightPose, setPreparedFlightPose: vi.fn(), setLakeVisualCondition: vi.fn(), setLakeSkyCondition: vi.fn(), setLakeVenueVisible: vi.fn(),
    setLakeWaterQuality: vi.fn<RendererAdapter["setLakeWaterQuality"]>(() => Promise.resolve({ kind: "complete" })),
      setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(), projectTrackingFrame: (value) => referenceFrameFixture(value),
      resize: vi.fn(), setStereoPresentation: vi.fn(), setScreenRenderScale: vi.fn(), setScreenRenderAuto: vi.fn(), setSelectRayHandler: vi.fn(), dispose: vi.fn()
    };
    const failHud = vi.fn<TailFlightHudPort["fail"]>();
    const hud: TailFlightHudPort = { render: vi.fn(), fail: failHud, setVisible: vi.fn() };
    const onTerminal = vi.fn();
    const activeController = new TailFlightController(port, input, renderer, hud, onTerminal);
    controller = activeController;
    const activeRuntime = new PresentationRuntime(renderer, [
      new ScreenPresentationBackend(() => ({ x: 1280, y: 720, pixelRatio: 1 })),
      mockSpatialBackend("webxr"), mockSpatialBackend("phone-vr")
    ], () => ({ viewModel: { ...createSceneFixture(activeController.currentSnapshot.phase === "result" ? "Result" : "Flight"),
      title: `Information ${informationName}` }, menu: { kind: "absent" } }),
    (timestampMs) => { activeController.onFrame(timestampMs); });
    runtime = activeRuntime;
    expect(await activeRuntime.start(backend)).toEqual({ ok: true });
    expect(activeRuntime.currentMode).toBe(backend);
    const loop = evidence.loop;
    if (loop === null) throw new Error("Expected a presentation frame loop");
    const viewer = unavailableViewerFrame("not-stereo");
    facade.flightPort.observeFlightKey({ control: "pilot_increase", pressed: true, repeat: false });
    for (let frame = 0; frame <= fps; frame += 1) {
      loop(frame * 1_000 / fps, viewer);
      if (frame === fps / 2) facade.flightPort.observeFlightKey({ control: "pilot_increase", pressed: false, repeat: false });
    }
    const terminal = activeController.currentSnapshot;
    if (terminal.phase !== "result" || terminal.frame.kind !== "result") throw new Error("Expected fixed-tick ManualAbort");
    expect(terminal.frame.finalization).toMatchObject({ reason: "manual_abort", disposition: "interrupted",
      terminal_tick: TERMINAL_TICK, terminal_fraction: 0, failure: null });
    expect(terminal.frame.state.datum_position_ned_m).not.toEqual(initial.frame.state.datum_position_ned_m);
    expect(terminal.frame.state.pilot_position_target_m).not.toBe(initial.frame.state.pilot_position_target_m);
    expect(onTerminal).toHaveBeenCalledTimes(1);
    expect(failHud).not.toHaveBeenCalled();
    expect(renderFrame).toHaveBeenCalledTimes(fps + 1);
    const renderedFrame = renderFrame.mock.lastCall?.[0];
    expect(renderedFrame?.cameraPose.position.x).toBe(backend === "screen" ? 0 : backend === "webxr" ? 0.2 : -0.2);
    const finalPose = setFlightPose.mock.lastCall?.[0];
    expect(finalPose).toMatchObject({ controls: { layout: "tail_incidence",
      physicalIncidence: { horizontalTailRadians: terminal.frame.state.physical_incidence.horizontal_tail_rad,
        verticalTailRadians: terminal.frame.state.physical_incidence.vertical_tail_rad } }, simulationTimeSeconds: 1 });
    const savedJson = session.export_flight_record_json();
    const saved: unknown = JSON.parse(savedJson);
    const record = recordObject(saved);
    const header = recordObject(record.header);
    const difficulty = recordObject(header.difficulty);
    expect(difficulty.information).toBe(informationName);
    expect(header).not.toHaveProperty("personal_best_key");
    if (!Array.isArray(record.samples)) throw new Error("Expected saved physical samples");
    const savedSamples: readonly unknown[] = record.samples;
    expect(savedSamples).toHaveLength(TERMINAL_TICK + 1);
    const namedSamples = readContract<AnalysisDocument>("record_analysis", session.flight_analysis_samples_json()).samples;
    expect(namedSamples).toHaveLength(TERMINAL_TICK + 1);
    expect(namedSamples.at(-1)).toMatchObject({ tick_index: TERMINAL_TICK, fraction: 0, flight_time_s: 1,
      controls: { layout: "tail_incidence", physical_incidence: terminal.frame.state.physical_incidence } });
    loop(1_100, viewer);
    loop(1_200, viewer);
    expect(onTerminal).toHaveBeenCalledTimes(1);
    expect(session.export_flight_record_json()).toBe(savedJson);
    return { initialIdentity: { scenario: initial.scenario, control_identity: initial.control_identity }, initialFrame: initial.frame, terminalFrame: terminal.frame,
      savedSamples, namedSamples,
      savedFinalization: record.finalization, finalPose, difficulty };
  } finally {
    controller?.dispose();
    if (runtime !== null) await runtime.dispose();
    session.free();
  }
}

describe.each([0, 1, 2])("same-WASM physical display invariants in control mode %s", (mode) => {
  let reference: Awaited<ReturnType<typeof runInvariantFlight>> | null = null;
  beforeAll(async () => { reference = await runInvariantFlight(mode, 0, "screen", 60); });
  it.each(CASES)("preserves physical columns for $information / $backend / $fps FPS", async ({ informationCode, backend, fps }) => {
    if (reference === null) throw new Error("Missing same-mode reference flight");
    const observed = await runInvariantFlight(mode, informationCode, backend, fps);
    expect(observed.initialIdentity).toEqual(reference.initialIdentity);
    expect(observed.initialFrame).toEqual(reference.initialFrame);
    expect(observed.terminalFrame).toEqual(reference.terminalFrame);
    expect(observed.savedSamples).toEqual(reference.savedSamples);
    expect(observed.namedSamples).toEqual(reference.namedSamples);
    expect(observed.savedFinalization).toEqual(reference.savedFinalization);
    expect(observed.finalPose).toEqual(reference.finalPose);
    expect(observed.difficulty.assistance).toBe(reference.difficulty.assistance);
    expect(observed.difficulty.weather).toBe(reference.difficulty.weather);
    if (informationCode !== 0) expect(observed.difficulty).not.toEqual(reference.difficulty);
  });
});
import { referenceFrameFixture } from "../presentation-check/menu-fixture.js";
