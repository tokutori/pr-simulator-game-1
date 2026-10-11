import type { TailPhysicalFlightControls, TailPresentationGeometryAvailability } from "../render/contracts/flight-controls.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";
import type { NamedRecordSample, RecordQueryContext } from "./named-record-query.js";
import type { TailControlIdentity, TailFlightProgressMeters, TailFlightTelemetry, TailPreparedLaunchSnapshot, TailScenarioIdentity, TailSessionSnapshot, TailTerminalFinalization } from "./tail-session-codec.js";

type NedVector = Readonly<{ north: number; east: number; down: number }>;
type Attitude = Readonly<{ w: number; x: number; y: number; z: number }>;
type BodyRate = Readonly<{ roll: number; pitch: number; yaw: number }>;
export type DisplayAvailability<Value, Reason extends string> =
  | Readonly<{ kind: "available"; value: Value }>
  | Readonly<{ kind: "unavailable"; reason: Reason }>;
type Available<Value> = Extract<DisplayAvailability<Value, never>, { kind: "available" }>;
type FlowAngle = DisplayAvailability<number, "undefined_flow_angle">;
type ExactStamp = Readonly<{ kind: "exact"; tick: number; fraction: number; timeSeconds: number }>;
export interface FlightDisplayTelemetry {
  readonly altitudeMeters: number;
  readonly airspeedMetersPerSecond: number;
  readonly groundspeedMetersPerSecond: number;
  readonly windVelocityNedMetersPerSecond: NedVector;
  readonly angleOfAttackRadians: FlowAngle;
  readonly sideslipAngleRadians: FlowAngle;
  readonly rollRadians: number;
  readonly pitchRadians: number;
  readonly headingRadians: number;
}

interface DisplayState {
  readonly positionNed: NedVector;
  readonly velocityNed: NedVector;
  readonly attitudeBodyToNed: Attitude;
  readonly pilotPositionMeters: number;
  readonly pilotVelocityMetersPerSecond: number;
  readonly telemetry: Available<FlightDisplayTelemetry>;
}
interface NamedDisplayState extends DisplayState {
  readonly stamp: ExactStamp;
  readonly angularRateBodyRadiansPerSecond: Available<BodyRate>;
  readonly compositeCgPositionNedMeters: Available<NedVector>;
  readonly telemetry: Available<FlightDisplayTelemetry>;
}
export type FlightDisplaySnapshot =
  | (NamedDisplayState & Readonly<{
      kind: "tail_flight";
      phaseCode: 5 | 6;
      controls: TailPhysicalFlightControls;
      tailGeometry: TailPresentationGeometryAvailability;
      pilotPositionTargetMeters: Available<number>;
      pilotPositionTargetNormalized: Available<number>;
      progressMeters: Available<TailFlightProgressMeters>;
    }>)
  | (NamedDisplayState & Readonly<{
      kind: "tail_result";
      controls: TailPhysicalFlightControls;
      tailGeometry: TailPresentationGeometryAvailability;
      pilotPositionTargetMeters: Available<number>;
      pilotPositionTargetNormalized: Available<number>;
      progressMeters: Readonly<{ kind: "unavailable"; reason: "terminal_progress_unavailable" }>;
      finalization: TailTerminalFinalization;
    }>)

  | (NamedDisplayState & Readonly<{
      kind: "tail_record";
      controls: TailPhysicalFlightControls;
      tailGeometry: TailPresentationGeometryAvailability;
      pilotPositionTargetMeters: Readonly<{ kind: "unavailable"; reason: "record_pilot_target_unavailable" }>;
      pilotPositionTargetNormalized: Readonly<{ kind: "unavailable"; reason: "record_pilot_target_unavailable" }>;
      progressMeters: Readonly<{ kind: "unavailable"; reason: "record_course_axis_unavailable" }>;
      finalization: TailTerminalFinalization;
    }>);

export function projectTailFlightSnapshot(snapshot: TailSessionSnapshot): DisplayAvailability<FlightDisplaySnapshot, "menu_phase"> {
  if (snapshot.frame.kind === "menu") return unavailable("menu_phase");
  if (snapshot.identity.kind !== "prepared") throw new RangeError("Tail flight display requires a sealed Rust identity");
  const tailGeometry = projectTailGeometry(snapshot.identity.scenario, snapshot.identity.controls);
  const state = snapshot.frame.state;
  const telemetry = snapshot.frame.telemetry;
  const values: NamedDisplayState = { positionNed: ned(state.datumPositionNedMeters), velocityNed: ned(state.datumVelocityNedMetersPerSecond),
    attitudeBodyToNed: attitude(state.attitudeBodyToNed), pilotPositionMeters: state.pilotPositionMeters,
    pilotVelocityMetersPerSecond: state.pilotVelocityMetersPerSecond, telemetry: available(displayTelemetry(telemetry)),
    stamp: Object.freeze({ kind: "exact", tick: state.tick, fraction: state.fraction, timeSeconds: state.flightTimeSeconds }),
    angularRateBodyRadiansPerSecond: available(bodyRate(state.angularRateBodyRadiansPerSecond)), compositeCgPositionNedMeters: available(ned(telemetry.compositeCgPositionNedMeters)) };
  const controls: TailPhysicalFlightControls = Object.freeze({ layout: "tail_incidence", physicalIncidence: state.physicalIncidence });
  if (snapshot.frame.kind === "result") {
    return available(Object.freeze({ ...values, kind: "tail_result", controls, tailGeometry, pilotPositionTargetMeters: available(state.pilotPositionTargetMeters),
      pilotPositionTargetNormalized: available(state.pilotPositionTargetNormalized),
      progressMeters: unavailable("terminal_progress_unavailable"),
      finalization: snapshot.frame.finalization }));
  }
  if (snapshot.phaseCode !== 5 && snapshot.phaseCode !== 6) throw new RangeError("Tail flight display requires a Rust flight phase");
  return available(Object.freeze({ ...values, kind: "tail_flight", phaseCode: snapshot.phaseCode, controls, tailGeometry, pilotPositionTargetMeters: available(state.pilotPositionTargetMeters),
    pilotPositionTargetNormalized: available(state.pilotPositionTargetNormalized), progressMeters: available(snapshot.frame.progressMeters) }));
}

export function projectRecordedFlightSnapshot(sample: NamedRecordSample, context: RecordQueryContext): FlightDisplaySnapshot {
  const state = sample.state;
  const values: NamedDisplayState = { positionNed: ned(state.datumPositionNedMeters), velocityNed: ned(state.datumVelocityNedMetersPerSecond),
    attitudeBodyToNed: attitude(state.attitudeBodyToNed), pilotPositionMeters: state.pilotPositionMeters,
    pilotVelocityMetersPerSecond: state.pilotVelocityMetersPerSecond, telemetry: available(displayTelemetry({ ...state.telemetry, windAtCgNedMetersPerSecond: state.windAtCgNedMetersPerSecond })),
    stamp: Object.freeze({ kind: "exact", tick: sample.tickIndex, fraction: sample.fraction, timeSeconds: sample.timeSeconds }),
    angularRateBodyRadiansPerSecond: available(bodyRate(state.angularVelocityBodyRadiansPerSecond)), compositeCgPositionNedMeters: available(ned(state.telemetry.compositeCgPositionNedMeters)) };

  return Object.freeze({ ...values, kind: "tail_record", controls: sample.controls,
    tailGeometry: projectTailGeometry(context.scenario, context.controlIdentity), pilotPositionTargetMeters: unavailable("record_pilot_target_unavailable"),
    pilotPositionTargetNormalized: unavailable("record_pilot_target_unavailable"), progressMeters: unavailable("record_course_axis_unavailable"), finalization: context.finalization });
}

export function projectFlightRenderPose(snapshot: FlightDisplaySnapshot, initialPilotPositionMeters: number): FlightRenderPose {
  if (!Number.isFinite(initialPilotPositionMeters)) throw new RangeError("Initial pilot position must be finite");
  const pose = { datumPositionNed: snapshot.positionNed, attitudeBodyToNed: snapshot.attitudeBodyToNed,
    pilotPositionMeters: snapshot.pilotPositionMeters, initialPilotPositionMeters, simulationTimeSeconds: snapshot.stamp.timeSeconds,
    airspeedMetersPerSecond: snapshot.telemetry.value.airspeedMetersPerSecond,
    windVelocityNedMetersPerSecond: snapshot.telemetry.value.windVelocityNedMetersPerSecond };
  return Object.freeze({ ...pose, controls: snapshot.controls, tailGeometry: snapshot.tailGeometry });
}

export function projectPreparedLaunchRenderPose(snapshot: TailPreparedLaunchSnapshot): FlightRenderPose | null {
  if (snapshot.kind === "unavailable") return null;
  const state = snapshot.state;
  return Object.freeze({
    datumPositionNed: ned(state.datumPositionNedMeters),
    attitudeBodyToNed: attitude(state.attitudeBodyToNed),
    pilotPositionMeters: state.pilotPositionMeters,
    initialPilotPositionMeters: state.pilotPositionMeters,
    simulationTimeSeconds: 0,
    controls: Object.freeze({ layout: "tail_incidence", physicalIncidence: state.physicalIncidence }),
    tailGeometry: projectTailGeometry(snapshot.identity.scenario, snapshot.identity.controls)
  });
}

function projectTailGeometry(scenario: TailScenarioIdentity, controls: TailControlIdentity): TailPresentationGeometryAvailability {
  if (scenario.aircraftModelVersion === 2 && controls.aircraftConfigurationId === "bpg041-playable-hybrid-mock") {
    return available(Object.freeze({ kind: "bpg041_playable_version_two", horizontalTailArmMeters: 3.6 }));
  }
  return unavailable("unregistered_aircraft_geometry");
}

function ned(values: readonly [number, number, number]): NedVector {
  return Object.freeze({ north: values[0], east: values[1], down: values[2] });
}

function attitude(values: readonly [number, number, number, number]): Attitude {
  return Object.freeze({ w: values[0], x: values[1], y: values[2], z: values[3] });
}

function bodyRate(values: readonly [number, number, number]): BodyRate {
  return Object.freeze({ roll: values[0], pitch: values[1], yaw: values[2] });
}

function displayTelemetry(telemetry: TailFlightTelemetry): FlightDisplayTelemetry {
  return Object.freeze({ altitudeMeters: telemetry.altitudeMeters, airspeedMetersPerSecond: telemetry.airspeedMetersPerSecond,
    groundspeedMetersPerSecond: telemetry.groundspeedMetersPerSecond, windVelocityNedMetersPerSecond: ned(telemetry.windAtCgNedMetersPerSecond),
    angleOfAttackRadians: flowAngle(telemetry.angleOfAttackRadians), sideslipAngleRadians: flowAngle(telemetry.sideslipAngleRadians),
    rollRadians: telemetry.attitudeEulerRadians[0], pitchRadians: telemetry.attitudeEulerRadians[1], headingRadians: telemetry.attitudeEulerRadians[2] });
}

function flowAngle(value: number | null): FlowAngle {
  return value === null ? unavailable("undefined_flow_angle") : available(value);
}

function available<Value>(value: Value): Available<Value> {
  return Object.freeze({ kind: "available", value });
}

function unavailable<Reason extends string>(reason: Reason): Readonly<{ kind: "unavailable"; reason: Reason }> {
  return Object.freeze({ kind: "unavailable", reason });
}
