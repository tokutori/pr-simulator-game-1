import type { FlightHudPort } from "../game/flight-controller.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { FlightLaunchProjection, FlightRuntimeUiState } from "./flight-runtime-ui.js";

export interface FlightControllerIdentity<Session, Controller> {
  readonly session: Session;
  readonly controller: Controller;
  readonly launchRequestId: number;
}

export function sameFlightControllerIdentity<Session, Controller>(
  current: FlightControllerIdentity<Session, Controller> | null,
  captured: FlightControllerIdentity<Session, Controller>
): boolean {
  return current !== null && current.session === captured.session && current.controller === captured.controller
    && current.launchRequestId === captured.launchRequestId;
}

export function flightSnapshotForView<Session, Controller extends Readonly<{ currentSnapshot: FlightSnapshot }>>(
  runtime: FlightRuntimeUiState, phaseCode: number, modelSnapshot: FlightSnapshot | null,
  identity: FlightControllerIdentity<Session, Controller> | null, session: Session | null, controller: Controller | null
): FlightSnapshot | null {
  if (runtime.kind === "projection-unavailable") return null;
  return runtime.kind === "active" && identity !== null && identity.session === session && identity.controller === controller
    && identity.launchRequestId === runtime.launchRequestId && (phaseCode === 5 || phaseCode === 6)
    ? identity.controller.currentSnapshot : modelSnapshot;
}

export function createCorrelatedFlightHudPort<Session, Controller>(
  delegate: FlightHudPort,
  captureIdentity: () => FlightControllerIdentity<Session, Controller> | null,
  onStop: (identity: FlightControllerIdentity<Session, Controller>, cause: string) => void
): FlightHudPort {
  return {
    render: (snapshot) => { delegate.render(snapshot); },
    setVisible: (visible) => { delegate.setVisible(visible); },
    fail: (cause) => {
      const identity = captureIdentity();
      if (identity === null) return;
      onStop(Object.freeze({ ...identity }), cause);
      delegate.fail(cause);
    }
  };
}

export interface FlightSafetySessionPort {
  phase_code(): number;
  abort(): ArrayLike<number>;
}

export type FlightRecoveryResult<Projection extends FlightLaunchProjection> =
  | { readonly kind: "stale" }
  | { readonly kind: "unavailable"; readonly cause: string }
  | { readonly kind: "ready"; readonly projection: Projection; readonly resetFailure: string | null };

export function recoverStoppedFlight<Projection extends FlightLaunchProjection>(
  session: FlightSafetySessionPort,
  isCurrent: () => boolean,
  readProjection: (terminal: ArrayLike<number> | null) => Projection,
  commitProjection: (projection: Projection) => void,
  resetController: (terminal: ArrayLike<number>) => void
): FlightRecoveryResult<Projection> {
  if (!isCurrent()) return Object.freeze({ kind: "stale" });
  try {
    const phase = session.phase_code();
    if (!isCurrent()) return Object.freeze({ kind: "stale" });
    const terminal = phase === 5 || phase === 6 ? session.abort() : null;
    if (!isCurrent()) return Object.freeze({ kind: "stale" });
    const projection = readProjection(terminal);
    if (!isCurrent()) return Object.freeze({ kind: "stale" });
    commitProjection(projection);
    if (!isCurrent()) return Object.freeze({ kind: "stale" });
    let resetFailure: string | null = null;
    if (terminal !== null) {
      try {
        resetController(terminal);
      } catch (error: unknown) {
        resetFailure = error instanceof Error ? error.message : String(error);
      }
    }
    return Object.freeze({ kind: "ready", projection, resetFailure });
  } catch (error: unknown) {
    return Object.freeze({ kind: "unavailable", cause: error instanceof Error ? error.message : String(error) });
  }
}
