import { initializeWasmFixture } from "./wasm-fixture.js";
import type { PlaybackSessionSnapshot, LiveSessionSnapshot } from "../../web/src/app/session-snapshot.js";
import type { TailResultSnapshot } from "../../web/src/game/tail-flight-controller.js";
import type { TailLogicalInput } from "../../web/src/game/tail-input.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";

initializeWasmFixture();

export const neutralTailInput: TailLogicalInput = { controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } };


export function launchCurrentSession(session: AppSessionFacade): void {
  session.executeSharedSessionCommand({ kind: "open_setup" });
  session.executeSharedSessionCommand({ kind: "prepare" });
  session.executeSharedSessionCommand({ kind: "start_countdown" });
  session.observeCountdown(session.captureCountdownToken(), 3, true);
}

let cachedContactRecord: string | undefined;

export function currentRecordFixture(eligible = true): string {
  if (eligible && cachedContactRecord !== undefined) return cachedContactRecord;
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  try {
    session.executeSharedSessionCommand({ kind: "open_setup" });
    session.executeSharedSessionCommand({ kind: "select_weather", weather: "calm" });
    session.executeSharedSessionCommand({ kind: "prepare" });
    session.executeSharedSessionCommand({ kind: "start_countdown" });
    session.observeCountdown(session.captureCountdownToken(), 3, true);
    session.flightPort.advanceTick(neutralTailInput);
    if (!eligible) session.executeSharedSessionCommand({ kind: "abort" });
    else {
      for (let tick = 1; tick < 90_000 && session.readSnapshot().phase !== "result"; tick += 1) {
        session.flightPort.advanceTick(neutralTailInput);
      }
    }
    const document = JSON.parse(session.exportRecordJson()) as {
      samples: readonly unknown[];
      finalization: { reason: string; terminal_tick: number; terminal_fraction: number };
    };
    if (eligible) {
      if (document.finalization.reason !== "water_contact") throw new Error("Contact fixture did not reach a valid water contact");
      document.samples = document.samples.slice(0, 2);
      document.finalization.terminal_tick = 1;
      document.finalization.terminal_fraction = 0;
      cachedContactRecord = JSON.stringify(document);
      return cachedContactRecord;
    }
    return JSON.stringify(document);
  } finally {
    session.dispose();
  }
}

export function currentFlightSnapshotFixture(phase?: "flight_running"): LiveSessionSnapshot<"flight_running">;
export function currentFlightSnapshotFixture(phase: "flight_paused"): LiveSessionSnapshot<"flight_paused">;
export function currentFlightSnapshotFixture(phase: "result"): TailResultSnapshot;
export function currentFlightSnapshotFixture(phase: "flight_running" | "flight_paused" | "result"): LiveSessionSnapshot | TailResultSnapshot;
export function currentFlightSnapshotFixture(phase: "flight_running" | "flight_paused" | "result" = "flight_running"): LiveSessionSnapshot | TailResultSnapshot {
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  try {
    launchCurrentSession(session);
    session.flightPort.advanceTick(neutralTailInput);
    if (phase === "flight_paused") session.pauseForReason("page_hidden");
    if (phase === "result") session.executeSharedSessionCommand({ kind: "abort" });
    const snapshot = session.readSnapshot();
    if (snapshot.phase !== phase || snapshot.frame.kind === "menu") throw new Error("Current fixture did not reach the requested Rust phase");
    return snapshot as LiveSessionSnapshot | TailResultSnapshot;
  } finally { session.dispose(); }
}

export function currentFlightPoseFixture(): FlightRenderPose {
  const snapshot = currentFlightSnapshotFixture();
  return snapshot.render_pose;
}

export function currentRecordedSnapshotFixture(): PlaybackSessionSnapshot {
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  try {
    launchCurrentSession(session);
    session.flightPort.advanceTick(neutralTailInput);
    session.executeSharedSessionCommand({ kind: "abort" });
    session.executeSharedSessionCommand({ kind: "enter_replay" });
    return session.queryRecordDisplay(0);
  } finally { session.dispose(); }
}
