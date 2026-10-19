import type { ReplayClockState } from "./app-state.js";
import type { TailGameSessionProjection } from "./session-snapshot.js";
import type { PlaybackClock, TailAppSessionFacade } from "./session-facade.js";

export function readRuntimeSessionProjection(session: TailAppSessionFacade): TailGameSessionProjection {
  const lifecycle = session.readLifecycle();
  if (lifecycle.phase !== "replay" && lifecycle.phase !== "attract") return session.readGameSessionProjection();
  const display = { kind: "available" as const, value: session.queryRecordDisplay(session.readPlaybackClock().timeSeconds) };
  return lifecycle.phase === "attract" ? { ...lifecycle, phase: lifecycle.phase, display }
    : { ...lifecycle, phase: lifecycle.phase, returnTarget: session.readReplayReturnTarget(), display };
}

export function projectRuntimePlaybackClock(clock: PlaybackClock): ReplayClockState {
  return { timeSeconds: clock.timeSeconds, rateCode: clock.rateCode, playing: clock.kind === "playing" };
}
