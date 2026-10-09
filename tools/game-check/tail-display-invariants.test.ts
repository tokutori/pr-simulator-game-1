import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import type { TailFlightHudPort, TailPilotInputPort, TailSessionPort } from "../../web/src/game/tail-flight-controller.js";
import type { TailPilotDemand } from "../../web/src/game/tail-device-input.js";
import { parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";
import type { TailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";
import { parseNamedAnalysisSamples, tailResultRecordContext } from "../../web/src/game/named-record-query.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
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
type FlightFrame = Extract<TailSessionSnapshot, { phaseCode: 5 | 6 }>["frame"];

function demandAtTick(tick: number): TailPilotDemand {
  const direction = tick >= 20 && tick < 40 ? 1 : tick >= 40 && tick < 60 ? -1 : 0;
  return { controlLayout: "tail_incidence", noseUp: direction * 0.03, turnRight: direction * -0.02,
    pilotPositionCommand: tick === 60 ? { kind: "set", normalized: 0.05 } : { kind: "hold" } };
}

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
    session.advance_countdown();
    session.launch();
    const initial = parseTailSessionSnapshot(session.snapshot_json());
    if (initial.phaseCode !== 5) throw new Error("Expected a prepared flight");
    expect(initial.identity).toMatchObject({ kind: "prepared", scenario: { catalogVersion: 3, scenarioVersion: 3,
      aircraftModelVersion: 2, controllerProfileVersion: 3, environmentVersion: 6, seedLow: 11, seedHigh: 12 },
      controls: { aircraftConfigurationId: "bpg041-playable-hybrid-mock" } });
    const committed: FlightFrame[] = [];
    const advanceTick = vi.fn((json: string) => {
      const nextJson = session.advance_tick_json(json);
      const next = parseTailSessionSnapshot(nextJson);
      if (next.phaseCode !== 5) throw new Error("Invariant sequence must remain airborne before its fixed abort");
      committed.push(next.frame);
      expect(next.frame.state.tick).toBe(committed.length);
      return next.frame.state.tick === TERMINAL_TICK ? session.abort() : nextJson;
    });
    const port: TailSessionPort = {
      snapshot_json: () => session.snapshot_json(), control_profile_json: () => session.control_profile_json(),
      advance_tick_json: advanceTick, free: () => undefined
    };
    let inputTick = 0;
    const readDemand = vi.fn(() => { const demand = demandAtTick(inputTick); inputTick += 1; return demand; });
    const input: TailPilotInputPort = {
      readDemand,
      reset: vi.fn(), suspend: vi.fn(), resume: vi.fn(), dispose: vi.fn()
    };
    const evidence: { loop: Parameters<RendererAdapter["startLoop"]>[0] | null } = { loop: null };
    const renderFrame = vi.fn<RendererAdapter["render"]>();
    const setFlightPose = vi.fn<RendererAdapter["setFlightPose"]>();
    const renderer: RendererAdapter = {
      startLoop: (callback) => { evidence.loop = callback; }, beginViewFrame: vi.fn(),
      stopLoop: () => { evidence.loop = null; }, render: renderFrame,
      setFlightPose, setPreparedFlightPose: vi.fn(), setLakeVisualCondition: vi.fn(), setLakeSkyCondition: vi.fn(), setLakeVenueVisible: vi.fn(),
      setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(), transformTrackingPose: (value) => value,
      resize: vi.fn(), setStereoPresentation: vi.fn(), setSelectRayHandler: vi.fn(), dispose: vi.fn()
    };
    const failHud = vi.fn<TailFlightHudPort["fail"]>();
    const hud: TailFlightHudPort = { render: vi.fn(), fail: failHud, setVisible: vi.fn() };
    const onTerminal = vi.fn();
    const activeController = new TailFlightController(port, input, renderer, hud, physics_hz(), () => [], onTerminal);
    controller = activeController;
    const activeRuntime = new PresentationRuntime(renderer, [
      new ScreenPresentationBackend(() => ({ x: 1280, y: 720, pixelRatio: 1 })),
      mockSpatialBackend("webxr"), mockSpatialBackend("phone-vr")
    ], () => ({ viewModel: { ...createSceneFixture(activeController.currentSnapshot.phaseCode === 7 ? "Result" : "Flight"),
      title: `Information ${informationName}` }, menu: { kind: "absent" } }),
    (timestampMs) => { activeController.onFrame(timestampMs); });
    runtime = activeRuntime;
    expect(await activeRuntime.start(backend)).toEqual({ ok: true });
    expect(activeRuntime.currentMode).toBe(backend);
    const loop = evidence.loop;
    if (loop === null) throw new Error("Expected a presentation frame loop");
    const viewer = unavailableViewerFrame("not-stereo");
    for (let frame = 0; frame <= fps; frame += 1) loop(frame * 1_000 / fps, viewer);
    const terminal = activeController.currentSnapshot;
    if (terminal.phaseCode !== 7) throw new Error("Expected fixed-tick ManualAbort");
    expect(terminal.frame.finalization).toMatchObject({ reason: "manual_abort", disposition: "interrupted",
      terminalTick: TERMINAL_TICK, terminalFraction: 0, failure: null });
    expect(terminal.frame.state).toEqual(committed.at(-1)?.state);
    expect(terminal.frame.telemetry).toEqual(committed.at(-1)?.telemetry);
    expect(terminal.frame.state.datumPositionNedMeters).not.toEqual(initial.frame.state.datumPositionNedMeters);
    expect(terminal.frame.state.pilotPositionTargetMeters).not.toBe(initial.frame.state.pilotPositionTargetMeters);
    expect(committed).toHaveLength(TERMINAL_TICK);
    expect(readDemand).toHaveBeenCalledTimes(TERMINAL_TICK);
    expect(onTerminal).toHaveBeenCalledTimes(1);
    expect(failHud).not.toHaveBeenCalled();
    expect(renderFrame).toHaveBeenCalledTimes(fps + 1);
    const renderedFrame = renderFrame.mock.lastCall?.[0];
    expect(renderedFrame?.cameraPose.position.x).toBe(backend === "screen" ? 0 : backend === "webxr" ? 0.2 : -0.2);
    const finalPose = setFlightPose.mock.lastCall?.[0];
    expect(finalPose).toMatchObject({ controls: { layout: "tail_incidence",
      physicalIncidence: terminal.frame.state.physicalIncidence }, simulationTimeSeconds: 1 });
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
    const namedSamples = parseNamedAnalysisSamples(session.flight_analysis_samples_json(), physics_hz(), tailResultRecordContext(terminal));
    expect(namedSamples).toHaveLength(TERMINAL_TICK + 1);
    expect(namedSamples.at(-1)).toMatchObject({ tickIndex: TERMINAL_TICK, fraction: 0, timeSeconds: 1,
      controls: { layout: "tail_incidence", physicalIncidence: terminal.frame.state.physicalIncidence } });
    loop(1_100, viewer);
    loop(1_200, viewer);
    expect(readDemand).toHaveBeenCalledTimes(TERMINAL_TICK);
    expect(onTerminal).toHaveBeenCalledTimes(1);
    expect(session.export_flight_record_json()).toBe(savedJson);
    return { initialIdentity: initial.identity, initialFrame: initial.frame, committed, terminalFrame: terminal.frame,
      inputs: advanceTick.mock.calls.map(([json]) => json), savedSamples, namedSamples,
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
    expect(observed.inputs).toEqual(reference.inputs);
    expect(observed.committed).toEqual(reference.committed);
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
