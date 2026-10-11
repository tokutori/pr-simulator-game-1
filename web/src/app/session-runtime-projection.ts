import type { ReplayClockState, TailGameSessionProjection } from "./app-state.js";
import type { TailAppSessionFacade } from "./session-facade.js";
import { analysisInitialPilotPositionMeters } from "../game/flight-analysis-view.js";
import type { NamedAnalysisDataset } from "../game/flight-analysis-view.js";
import { projectFlightRenderPose, projectRecordedFlightSnapshot } from "../game/flight-display-snapshot.js";
import type { NamedPlaybackClock } from "../game/named-record-query.js";

export function readRuntimeSessionProjection(session: TailAppSessionFacade): TailGameSessionProjection {
  const token = session.captureQueryToken();
  const lifecycle = session.readLifecycle();
  let projection: TailGameSessionProjection;
  if (lifecycle.phaseCode !== 9 && lifecycle.phaseCode !== 10) projection = session.readGameSessionProjection();
  else {
    const context = session.readPlaybackContext();
    const common = { controlLayout: "tail_incidence" as const, controlModeCode: lifecycle.controlModeCode,
      difficulty: session.readDifficulty(), configurationMetadata: null,
      countdownRemaining: lifecycle.countdownRemaining, canResume: lifecycle.canResume };
    const display = session.queryRecordDisplay(session.readPlaybackClock().timeSeconds);
    if (context.phase === "attract") {
      if (display.kind !== "tail_record") throw new RangeError("Attract requires its two-tail demonstration record");
      projection = Object.freeze({ ...common, phaseCode: 10, display: Object.freeze({ kind: "available", value: display }) });
    } else {
      if (display.kind !== "tail_record") throw new RangeError("Replay requires a recorded display");
      projection = Object.freeze({ ...common, phaseCode: 9, returnTarget: session.readReplayReturnTarget(), display: Object.freeze({ kind: "available", value: display }) });
    }
  }
  if (session.acceptQuery(token, projection).kind === "stale") throw new RangeError("Runtime session observation changed during projection");
  return projection;
}

export function queryRuntimeRecordPose(session: TailAppSessionFacade, dataset: NamedAnalysisDataset, timeSeconds: number) {
  const cursor = session.queryAnalysisCursor(timeSeconds, dataset);
  const display = projectRecordedFlightSnapshot(cursor, cursor.context);
  return Object.freeze({ cursor, display, pose: projectFlightRenderPose(display, analysisInitialPilotPositionMeters(dataset)) });
}

export function projectRuntimePlaybackClock(clock: NamedPlaybackClock): ReplayClockState {
  return Object.freeze({ timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: clock.kind === "playing" });
}
