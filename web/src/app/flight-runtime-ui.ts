import type { FlightSnapshot } from "../game/flight-snapshot.js";

export type FlightRuntimeUiState =
  | { readonly kind: "inactive" }
  | { readonly kind: "active"; readonly launchRequestId: number }
  | { readonly kind: "stopped"; readonly launchRequestId: number; readonly cause: string }
  | { readonly kind: "projection-unavailable"; readonly launchRequestId: number; readonly cause: string };

export interface FlightRuntimeCorrelation {
  readonly expectedLaunchRequestId: number | null;
  readonly flightRuntime: FlightRuntimeUiState;
}

export interface FlightLaunchProjection {
  readonly phaseCode: number;
  readonly snapshot: FlightSnapshot | null;
}

export type FlightLaunchFailure<Projection extends FlightLaunchProjection> =
  | { readonly kind: "not-launched" }
  | { readonly kind: "launched"; readonly projection: Projection }
  | { readonly kind: "projection-unavailable" };

export function acceptsFlightActivation(
  correlation: FlightRuntimeCorrelation, launchRequestId: number, phaseCode: number,
  countdownRemaining: number, projection: FlightLaunchProjection
): boolean {
  return correlation.expectedLaunchRequestId === launchRequestId && phaseCode === 4 && countdownRemaining === 0
    && projection.phaseCode === 5 && projection.snapshot?.terminal === "airborne";
}

export function failFlightLaunch<Projection extends FlightLaunchProjection>(
  correlation: FlightRuntimeCorrelation, launchRequestId: number, phaseCode: number,
  countdownRemaining: number, cause: string, outcome: FlightLaunchFailure<Projection>
): Readonly<{ correlation: FlightRuntimeCorrelation; projection: Projection | null }> | null {
  if (correlation.expectedLaunchRequestId !== launchRequestId) return null;
  if (outcome.kind === "not-launched") return Object.freeze({ correlation: Object.freeze({ expectedLaunchRequestId: null, flightRuntime: correlation.flightRuntime }), projection: null });
  if (phaseCode !== 4 || countdownRemaining !== 0) return null;
  const projection = outcome.kind === "launched" && outcome.projection.phaseCode === 5 && outcome.projection.snapshot?.terminal === "airborne" ? outcome.projection : null;
  return Object.freeze({
    correlation: Object.freeze({ expectedLaunchRequestId: null,
      flightRuntime: Object.freeze({ kind: projection === null ? "projection-unavailable" : "stopped", launchRequestId, cause }) }),
    projection
  });
}

export function stopFlightRuntime(runtime: FlightRuntimeUiState, launchRequestId: number, phaseCode: number, cause: string): FlightRuntimeUiState {
  return runtime.kind === "active" && runtime.launchRequestId === launchRequestId && (phaseCode === 5 || phaseCode === 6 || phaseCode === 7)
    ? Object.freeze({ kind: "stopped", launchRequestId, cause }) : runtime;
}

export function flightRuntimeNotice(correlation: FlightRuntimeCorrelation, phaseCode: number): string | null {
  const diagnostic = correlation.flightRuntime;
  if (diagnostic.kind === "inactive" || diagnostic.kind === "active") return null;
  if (diagnostic.kind === "projection-unavailable") return `飛行状態を取得できない: ${diagnostic.cause}`;
  const currentFlight = (phaseCode === 5 || phaseCode === 6) && correlation.expectedLaunchRequestId === null;
  return `${currentFlight ? "飛行処理を停止した。最後の有効値を表示している" : "直前の操作飛行の停止理由"}: ${diagnostic.cause}`;
}
