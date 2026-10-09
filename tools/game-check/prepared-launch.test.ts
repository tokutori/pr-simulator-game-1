import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { projectFlightRenderPose, projectTailFlightSnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { encodeTailLogicalInput, parseTailPreparedLaunchSnapshot, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

describe("sealed initial launch presentation", () => {
  for (const mode of [0, 1, 2]) {
    it.each([0, 1, 2, 3, 4])(`preserves mode ${String(mode)} and weather %s from preparation through launch and Retry`, (weather) => {
      const bridge = new HybridGameSessionBridge(mode, 21, 22);
      const session = new TailAppSessionFacade(bridge, physics_hz());
      try {
        expect(session.readPreparedLaunchPose()).toBeNull();
        session.executeOperation("open-setup");
        expect(session.readPreparedLaunchPose()).toBeNull();
        session.executeOperation({ kind: "set-difficulty-option", axis: "weather", code: weather });
        session.executeOperation("prepare");
        const initial = session.readPreparedLaunchPose();
        expect(initial).not.toBeNull();
        expect(initial?.controls?.layout).toBe("tail_incidence");
        const before = bridge.snapshot_json();
        const prepared = parseTailPreparedLaunchSnapshot(bridge.prepared_launch_snapshot_json(), physics_hz());
        expect(prepared.kind).toBe("prepared");
        expect(bridge.snapshot_json()).toBe(before);
        expect(session.readLifecycle().phaseCode).toBe(3);
        session.executeOperation("start-flight");
        for (let step = 0; step < 3; step++) {
          expect(session.readPreparedLaunchPose()).toEqual(initial);
          session.advanceCountdown();
        }
        session.launch();
        expect(session.readPreparedLaunchPose()).toBeNull();
        const live = parseTailSessionSnapshot(bridge.snapshot_json(), physics_hz());
        const display = projectTailFlightSnapshot(live);
        if (display.kind !== "available" || initial === null || live.frame.kind !== "flight" || prepared.kind !== "prepared") {
          throw new Error("Expected sealed preparation and its first live frame");
        }
        expect(live.frame.state).toEqual(prepared.state);
        expect(projectFlightRenderPose(display.value, live.frame.state.pilotPositionMeters)).toMatchObject(initial);
        session.flightPort.advance_tick_json(neutral);
        session.executeOperation("abort");
        expect(session.readPreparedLaunchPose()).toBeNull();
        session.executeOperation("retry");
        expect(session.readPreparedLaunchPose()).toEqual(initial);
        session.executeOperation("cancel-briefing");
        expect(session.readPreparedLaunchPose()).toBeNull();
      } finally { session.dispose(); }
    });
  }

  it.each([0, 1, 5, 6, 7, 9, 10])("rejects a prepared initial state in phase %s", (phase) => {
    const document = preparedDocument();
    document.phase_code = phase;
    expect(() => parseTailPreparedLaunchSnapshot(JSON.stringify(document), physics_hz())).toThrow("preparation phase");
  });

  it.each([2, 3, 4, 8])("requires a zero-tick sealed initial state in phase %s", (phase) => {
    const document = preparedDocument();
    document.phase_code = phase;
    expect(parseTailPreparedLaunchSnapshot(JSON.stringify(document), physics_hz())).toMatchObject({ kind: "prepared", phaseCode: phase });
    document.preview = { kind: "unavailable" };
    expect(() => parseTailPreparedLaunchSnapshot(JSON.stringify(document), physics_hz())).toThrow("sealed initial state");
  });

  it.each(["tick", "fraction", "attitude", "identity", "layout", "extra"])("rejects malformed prepared %s", (field) => {
    const document = preparedDocument();
    const preview = document.preview as Record<string, unknown>;
    const state = preview.state as Record<string, unknown>;
    if (field === "tick") { state.tick = 1; state.flight_time_s = 1 / physics_hz(); }
    if (field === "fraction") { state.fraction = 0.5; state.flight_time_s = 0.5 / physics_hz(); }
    if (field === "attitude") state.attitude_body_to_ned = [0, 0, 0, 0];
    if (field === "identity") { preview.scenario = null; preview.control_identity = null; }
    if (field === "layout") document.control_layout = "legacy_three_axis";
    if (field === "extra") preview.flight_started = true;
    expect(() => parseTailPreparedLaunchSnapshot(JSON.stringify(document), physics_hz())).toThrow();
  });
});

function preparedDocument(): Record<string, unknown> {
  const bridge = new HybridGameSessionBridge(0, 21, 22);
  try {
    bridge.open_setup();
    bridge.prepare();
    return JSON.parse(bridge.prepared_launch_snapshot_json()) as Record<string, unknown>;
  } finally { bridge.free(); }
}
