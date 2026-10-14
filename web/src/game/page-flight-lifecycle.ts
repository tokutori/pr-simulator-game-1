export interface PageFlightSessionPort {
  phase_code(): number;
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
  const phaseCode = session.phase_code();
  if (phaseCode === 4) {
    session.cancel_countdown();
    synchronize();
  } else if (phaseCode === 5 || phaseCode === 6) {
    session.pause(1);
    synchronize();
  }
}
