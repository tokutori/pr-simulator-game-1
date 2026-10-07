import { boundaryInteger, boundaryNumber, boundaryObject } from "./tail-boundary-values.js";
import { tailInputWithRateDemand } from "./tail-device-input.js";
import type { TailPilotDemand } from "./tail-device-input.js";
import type { TailLogicalInput, TailSessionIdentity, TailSessionSnapshot } from "./tail-session-codec.js";

type PreparedIdentity = Extract<TailSessionIdentity, { kind: "prepared" }>;
interface ControlAxes {
  readonly pitch: number;
  readonly yaw: number;
}
export interface TailControlProfile {
  readonly schemaVersion: 2;
  readonly controlLayout: "tail_incidence";
  readonly controllerProfileId: string;
  readonly controllerProfileVersion: number;
  readonly desiredBodyRateLimitRadiansPerSecond: ControlAxes;
  readonly feedbackGainSeconds: ControlAxes;
  readonly maximumSlewRadiansPerSecond: number;
  readonly identity: PreparedIdentity;
}

export function parseTailControlProfile(json: string, snapshot: TailSessionSnapshot): TailControlProfile {
  if (json.length > 2_048) throw new RangeError("Tail control profile exceeds its bounded envelope");
  const identity = preparedIdentity(snapshot);
  const parsed: unknown = JSON.parse(json);
  const document = boundaryObject(parsed, ["schema_version", "control_layout", "controller_profile_id", "controller_profile_version",
    "desired_body_rate_limit_rad_s", "feedback_gain_seconds", "maximum_slew_rad_s"]);
  if (document.schema_version !== 2 || document.control_layout !== "tail_incidence") {
    throw new RangeError("Unsupported tail control profile schema or layout");
  }
  const controllerProfileVersion = boundaryInteger(document.controller_profile_version, 1, 0xffff_ffff);
  if (document.controller_profile_id !== identity.controls.controllerProfileId
      || controllerProfileVersion !== identity.scenario.controllerProfileVersion) {
    throw new RangeError("Tail controller metadata does not match its sealed session");
  }
  const limits = decodeAxes(document.desired_body_rate_limit_rad_s, Number.MIN_VALUE);
  const gains = decodeAxes(document.feedback_gain_seconds, 0);
  return Object.freeze({ schemaVersion: 2, controlLayout: "tail_incidence", controllerProfileId: identity.controls.controllerProfileId,
    controllerProfileVersion, desiredBodyRateLimitRadiansPerSecond: limits, feedbackGainSeconds: gains,
    maximumSlewRadiansPerSecond: boundaryNumber(document.maximum_slew_rad_s, Number.MIN_VALUE), identity });
}

export function tailInputFromControlProfile(demand: TailPilotDemand, profile: TailControlProfile, snapshot: TailSessionSnapshot): TailLogicalInput {
  const identity = preparedIdentity(snapshot);
  if (profile.controllerProfileId !== identity.controls.controllerProfileId
      || profile.controllerProfileVersion !== identity.scenario.controllerProfileVersion
      || !samePreparedIdentity(profile.identity, identity)) {
    throw new RangeError("Tail control profile belongs to another sealed session");
  }
  const noseUp = boundaryNumber(demand.noseUp, -1, 1);
  const turnRight = boundaryNumber(demand.turnRight, -1, 1);
  const limits = profile.desiredBodyRateLimitRadiansPerSecond;
  return tailInputWithRateDemand(demand, {
    pitchRadiansPerSecond: noseUp * boundaryNumber(limits.pitch, Number.MIN_VALUE),
    yawRadiansPerSecond: turnRight * boundaryNumber(limits.yaw, Number.MIN_VALUE)
  });
}

function preparedIdentity(snapshot: TailSessionSnapshot): PreparedIdentity {
  if (snapshot.identity.kind !== "prepared") throw new RangeError("Tail control profile requires a sealed session");
  return snapshot.identity;
}

function decodeAxes(value: unknown, minimum: number): ControlAxes {
  const axes = boundaryObject(value, ["pitch", "yaw"]);
  return Object.freeze({ pitch: boundaryNumber(axes.pitch, minimum), yaw: boundaryNumber(axes.yaw, minimum) });
}

function samePreparedIdentity(first: PreparedIdentity, second: PreparedIdentity): boolean {
  const previous = first.scenario;
  const current = second.scenario;
  return first.controls.aircraftConfigurationId === second.controls.aircraftConfigurationId
    && first.controls.controllerProfileId === second.controls.controllerProfileId
    && previous.catalogVersion === current.catalogVersion && previous.scenarioId === current.scenarioId
    && previous.scenarioVersion === current.scenarioVersion && previous.aircraftModelVersion === current.aircraftModelVersion
    && previous.environmentVersion === current.environmentVersion && previous.controllerProfileVersion === current.controllerProfileVersion
    && previous.seedLow === current.seedLow && previous.seedHigh === current.seedHigh;
}
