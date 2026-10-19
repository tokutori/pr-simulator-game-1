import type { TailInputDocument } from "../generated/tail-input.js";
import { boundaryObject, captureJsonInput } from "./tail-boundary-values.js";

export const TAIL_SESSION_SCHEMA_VERSION: TailInputDocument["schema_version"] = 2;
export type TailPilotPositionCommand = TailInputDocument["pilot_position_command"];
export interface TailLogicalInput {
  readonly controlLayout: TailInputDocument["control_layout"];
  readonly noseUp: TailInputDocument["nose_up"];
  readonly turnRight: TailInputDocument["turn_right"];
  readonly desiredPitchRateRadiansPerSecond: TailInputDocument["desired_pitch_rate_rad_s"];
  readonly desiredYawRateRadiansPerSecond: TailInputDocument["desired_yaw_rate_rad_s"];
  readonly pilotPositionCommand: TailPilotPositionCommand;
}

export function encodeTailLogicalInput(input: TailLogicalInput): string {
  const object = boundaryObject(input, ["controlLayout", "noseUp", "turnRight", "desiredPitchRateRadiansPerSecond",
    "desiredYawRateRadiansPerSecond", "pilotPositionCommand"]);
  const pilotPositionCommand = captureJsonInput(object.pilotPositionCommand);
  if (typeof pilotPositionCommand !== "object" || pilotPositionCommand === null || Array.isArray(pilotPositionCommand)
      || !Object.hasOwn(pilotPositionCommand, "kind")) throw new RangeError("Pilot command requires its own kind");
  if ("kind" in pilotPositionCommand && pilotPositionCommand.kind === "set" && !Object.hasOwn(pilotPositionCommand, "normalized")) {
    throw new RangeError("Pilot set command requires its own normalized target");
  }
  const document = Object.setPrototypeOf({
    schema_version: TAIL_SESSION_SCHEMA_VERSION,
    control_layout: object.controlLayout,
    nose_up: object.noseUp,
    turn_right: object.turnRight,
    desired_pitch_rate_rad_s: object.desiredPitchRateRadiansPerSecond,
    desired_yaw_rate_rad_s: object.desiredYawRateRadiansPerSecond,
    pilot_position_command: pilotPositionCommand
  }, null) as Record<string, unknown>;
  return JSON.stringify(captureJsonInput(document));
}
