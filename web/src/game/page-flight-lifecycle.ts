import type { SessionLifecycleDocument } from "../generated/session-lifecycle.js";
export interface PageFlightSessionPort {
  phase(): SessionLifecycleDocument["phase"];
  cancel_countdown(): void;
  pause(reason: number): void;
}

export function suspendPageFlight(
  session: PageFlightSessionPort | null,
  inputController: { suspend(): void } | null,
  synchronize: () => void
): void {
  inputController?.suspend();
  if (session === null) return;
  const phase = session.phase();
  if (phase === "countdown") {
    session.cancel_countdown();
    synchronize();
  } else if (phase === "flight_running" || phase === "flight_paused") {
    session.pause(1);
    synchronize();
  }
}
