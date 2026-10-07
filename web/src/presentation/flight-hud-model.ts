import type { FlightSnapshot } from "../game/flight-snapshot.js";
import { projectLegacyFlightSnapshot } from "../game/flight-display-snapshot.js";
import type { FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import type { TailTerminalReason } from "../game/tail-session-codec.js";
import type { HudProfileUiState } from "../app/app-state.js";
import { nedToWgs84 } from "../render/contracts/launch-venue.js";

export type InformationLevelCode = 0 | 1 | 2 | 3 | 4;

export interface FlightHudModel {
  readonly status: string;
  readonly attitude: Readonly<{ rollDegrees: number; pitchDegrees: number }> | null;
  readonly flightPathAngleDegrees: number | null;
  readonly warning: string | null;
  readonly headingDegrees: number | null;
  readonly pilotPositionRatio: number | null;
  readonly windDirectionDegrees: number | null;
  readonly angleOfAttackDegrees: number | null;
  readonly readouts: string;
  readonly heading: string | null;
  readonly pilotPosition: string | null;
  readonly wind: string | null;
  readonly angleOfAttack: string | null;
  readonly telemetry: string;
  readonly location: string;
  readonly mapAttribution: string;
  readonly supplementaryReadouts: readonly string[];
  readonly controlsDescription: string;
}

export function createFlightHudModel(
  snapshot: FlightSnapshot,
  informationCode: InformationLevelCode,
  customProfile: HudProfileUiState = fullProfile
): FlightHudModel {
  return createFlightDisplayHudModel(projectLegacyFlightSnapshot(snapshot), informationCode, customProfile);
}

export function createFlightDisplayHudModel(
  snapshot: FlightDisplaySnapshot,
  informationCode: InformationLevelCode,
  customProfile: HudProfileUiState = fullProfile
): FlightHudModel {
  if (informationCode === 4) return customFlightHudModel(snapshot, customProfile);
  const telemetry = snapshot.telemetry;
  const distance = distanceReadout(snapshot);
  const duration = `時間 ${snapshot.stamp.timeSeconds.toFixed(1)} s`;
  const coordinates = nedToWgs84(snapshot.positionNed.north, snapshot.positionNed.east);
  const location = `緯度 ${coordinates.latitudeDegrees.toFixed(6)}° · 経度 ${coordinates.longitudeDegrees.toFixed(6)}°`;
  const mapAttribution = "湖岸 © OpenStreetMap contributors · ODbL 1.0";
  const presentation = Object.freeze({
    status: snapshotStatus(snapshot),
    supplementaryReadouts: informationCode === 0 ? supplementaryReadouts(snapshot) : Object.freeze([]),
    controlsDescription: controlsDescription(snapshot)
  });
  if (telemetry.kind === "unavailable") {
    return Object.freeze({
      ...presentation,
      attitude: null,
      flightPathAngleDegrees: informationCode === 0 ? flightPathAngle(snapshot) : null,
      warning: informationCode === 0 ? warningFor(snapshot) : null,
      headingDegrees: null,
      pilotPositionRatio: informationCode === 0 ? legacyPilotPositionRatio(snapshot) : null,
      windDirectionDegrees: null,
      angleOfAttackDegrees: null,
      readouts: informationCode === 2 ? `${distance} · ${duration}` : `計器データ unavailable · ${distance} · ${duration}`,
      heading: null,
      pilotPosition: informationCode === 0 ? `${snapshot.pilotPositionMeters.toFixed(2)} m` : null,
      wind: informationCode === 0 ? "unavailable" : null,
      angleOfAttack: informationCode === 0 ? "unavailable" : null,
      telemetry: "telemetry unavailable",
      location,
      mapAttribution
    });
  }

  const values = telemetry.value;
  const rollDegrees = values.rollRadians * 180 / Math.PI;
  const pitchDegrees = values.pitchRadians * 180 / Math.PI;
  const headingDegrees = normalizeDegrees(values.headingRadians * 180 / Math.PI);
  const airspeed = `IAS ${values.airspeedMetersPerSecond.toFixed(1)} m/s`;
  const altitude = `ALT ${values.altitudeMeters.toFixed(1)} m`;
  const attitudeText = `PITCH ${pitchDegrees.toFixed(0)}° · ROLL ${rollDegrees.toFixed(0)}°`;
  const attitude = Object.freeze({ rollDegrees, pitchDegrees });
  const angleOfAttackDegrees = values.angleOfAttackRadians.kind === "unavailable"
    ? null
    : values.angleOfAttackRadians.value * 180 / Math.PI;

  switch (informationCode) {
    case 0: {
      const wind = values.windVelocityNedMetersPerSecond;
      const groundspeed = `対地速度 ${values.groundspeedMetersPerSecond.toFixed(1)} m/s`;
      return Object.freeze({
        ...presentation,
        attitude,
        flightPathAngleDegrees: flightPathAngle(snapshot),
        warning: warningFor(snapshot),
        headingDegrees,
        pilotPositionRatio: legacyPilotPositionRatio(snapshot),
        windDirectionDegrees: windDirectionDegrees(wind.north, wind.east),
        angleOfAttackDegrees,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: `${snapshot.pilotPositionMeters >= 0 ? "+" : ""}${snapshot.pilotPositionMeters.toFixed(2)} m`,
        wind: `N ${wind.north.toFixed(1)} · E ${wind.east.toFixed(1)} · D ${wind.down.toFixed(1)} m/s`,
        angleOfAttack: angleOfAttackDegrees === null
          ? "—"
          : `${angleOfAttackDegrees.toFixed(1)}°`,
        telemetry: `${groundspeed} · ${distance} · ${duration}`,
        location,
        mapAttribution
      });
    }
    case 1:
      return Object.freeze({
        ...presentation,
        attitude,
        flightPathAngleDegrees: null,
        warning: null,
        headingDegrees,
        pilotPositionRatio: null,
        windDirectionDegrees: null,
        angleOfAttackDegrees: null,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: null,
        wind: null,
        angleOfAttack: null,
        telemetry: `対地速度 ${values.groundspeedMetersPerSecond.toFixed(1)} m/s · ${distance} · ${duration}`,
        location,
        mapAttribution
      });
    case 2:
      return Object.freeze({
        ...presentation,
        attitude: null,
        flightPathAngleDegrees: null,
        warning: null,
        headingDegrees: null,
        pilotPositionRatio: null,
        windDirectionDegrees: null,
        angleOfAttackDegrees: null,
        readouts: `${altitude} · ${distance} · ${duration}`,
        heading: null,
        pilotPosition: null,
        wind: null,
        angleOfAttack: null,
        telemetry: "",
        location,
        mapAttribution
      });
    case 3:
      return Object.freeze({
        ...presentation,
        attitude,
        flightPathAngleDegrees: null,
        warning: null,
        headingDegrees,
        pilotPositionRatio: null,
        windDirectionDegrees: null,
        angleOfAttackDegrees: null,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: null,
        wind: null,
        angleOfAttack: null,
        telemetry: "代表計器表示 · 対象機の実機構成は未確認",
        location,
        mapAttribution
      });
  }
}

function customFlightHudModel(snapshot: FlightDisplaySnapshot, profile: HudProfileUiState): FlightHudModel {
  const full = createFlightDisplayHudModel(snapshot, 0);
  const telemetry = snapshot.telemetry;
  const heading = profile.attitude ? full.heading : null;
  const angleOfAttack = profile.angleOfAttack ? full.angleOfAttack : null;
  const attitudeReadout = profile.attitude && full.attitude !== null
    ? `PITCH ${full.attitude.pitchDegrees.toFixed(0)}° · ROLL ${full.attitude.rollDegrees.toFixed(0)}°`
    : null;
  const telemetryReadout = profile.telemetry && telemetry.kind === "available"
    ? `IAS ${telemetry.value.airspeedMetersPerSecond.toFixed(1)} m/s · ALT ${telemetry.value.altitudeMeters.toFixed(1)} m`
    : null;
  const wind = profile.wind ? full.wind : null;
  const profileReadouts = [telemetryReadout, attitudeReadout].filter((value) => value !== null).join("\n");
  return Object.freeze({
    ...full,
    attitude: profile.attitude ? full.attitude : null,
    headingDegrees: profile.attitude ? full.headingDegrees : null,
    pilotPositionRatio: profile.telemetry ? full.pilotPositionRatio : null,
    windDirectionDegrees: profile.wind ? full.windDirectionDegrees : null,
    angleOfAttackDegrees: profile.angleOfAttack ? full.angleOfAttackDegrees : null,
    flightPathAngleDegrees: profile.flightPath ? flightPathAngle(snapshot) : null,
    warning: profile.warnings ? warningFor(snapshot) : null,
    readouts: profileReadouts,
    heading,
    pilotPosition: profile.telemetry ? full.pilotPosition : null,
    wind,
    angleOfAttack,
    telemetry: profile.telemetry ? full.telemetry : "",
    supplementaryReadouts: supplementaryReadouts(snapshot, profile.attitude, profile.telemetry)
  });
}

function flightPathAngle(snapshot: FlightDisplaySnapshot): number | null {
  const horizontalSpeed = Math.hypot(snapshot.velocityNed.north, snapshot.velocityNed.east);
  if (horizontalSpeed === 0) return null;
  return Math.atan2(-snapshot.velocityNed.down, horizontalSpeed) * 180 / Math.PI;
}

function warningFor(snapshot: FlightDisplaySnapshot): string | null {
  switch (snapshot.kind) {
    case "legacy_live": return terminalWarning(snapshot.terminal);
    case "tail_flight": return null;
    case "legacy_record":
    case "tail_record": return null;
    case "tail_result": return terminalWarning(snapshot.finalization.reason);
  }
}

function terminalWarning(reason: FlightSnapshot["terminal"] | TailTerminalReason): string | null {
  switch (reason) {
    case "out_of_valid_envelope":
    case "out-of-valid-envelope": return "AERODYNAMIC ENVELOPE";
    case "fatal_simulation_error":
    case "fatal-simulation-error": return "SIMULATION FAILURE";
    case "airborne":
    case "water_contact":
    case "water-contact":
    case "time_limit":
    case "time-limit":
    case "manual-abort":
    case "manual_abort": return null;
  }
}

const fullProfile: HudProfileUiState = Object.freeze({
  telemetry: true,
  attitude: true,
  wind: true,
  flightPath: true,
  angleOfAttack: true,
  warnings: true
});

function normalizeDegrees(degrees: number): number {
  return ((degrees % 360) + 360) % 360;
}

function pilotPositionRatio(positionMeters: number): number {
  return Math.max(-1, Math.min(1, positionMeters / 0.4));
}

function legacyPilotPositionRatio(snapshot: FlightDisplaySnapshot): number | null {
  return snapshot.controls.layout === "legacy_three_axis" ? pilotPositionRatio(snapshot.pilotPositionMeters) : null;
}

function distanceReadout(snapshot: FlightDisplaySnapshot): string {
  if (snapshot.kind === "legacy_live") return `距離 ${snapshot.scoreCourseMeters.toFixed(1)} m`;
  if (snapshot.kind === "tail_result" && snapshot.finalization.scoreMeters !== null) {
    return `確定距離 ${snapshot.finalization.scoreMeters[0].toFixed(1)} m`;
  }
  return snapshot.kind === "legacy_record" || snapshot.kind === "tail_record" ? "保存標本" : "距離 unavailable";
}

function snapshotStatus(snapshot: FlightDisplaySnapshot): string {
  switch (snapshot.kind) {
    case "legacy_live": return terminalLabel(snapshot.terminal);
    case "tail_flight": return snapshot.phaseCode === 6 ? "一時停止" : "滑空中";
    case "tail_result": return terminalLabel(snapshot.finalization.reason);
    case "legacy_record":
    case "tail_record": return "記録再生";
  }
}

function supplementaryReadouts(snapshot: FlightDisplaySnapshot, attitudeVisible = true, telemetryVisible = true): readonly string[] {
  const lines: string[] = [];
  if (attitudeVisible && snapshot.angularRateBodyRadiansPerSecond.kind === "available") {
    const rate = snapshot.angularRateBodyRadiansPerSecond.value;
    lines.push(`p ${degrees(rate.roll)}  q ${degrees(rate.pitch)}  r ${degrees(rate.yaw)} °/s`);
  }
  if (telemetryVisible && snapshot.controls.layout === "tail_incidence") {
    const incidence = snapshot.controls.physicalIncidence;
    lines.push(`水平尾翼 ${degrees(incidence.horizontalTailRadians)}°  垂直尾翼 ${degrees(incidence.verticalTailRadians)}°`);
    const target = snapshot.pilotPositionTargetMeters;
    lines.push(target.kind === "available" ? `PILOT TARGET ${target.value.toFixed(2)} m` : "PILOT TARGET unavailable");
  }
  return Object.freeze(lines);
}

function degrees(radians: number): string {
  return (radians * 180 / Math.PI).toFixed(1);
}

function controlsDescription(snapshot: FlightDisplaySnapshot): string {
  return snapshot.controls.layout === "tail_incidence"
    ? "↑/↓ nose-up/down intent · ←/→ right/left intent · J/L pilot Set · キー解放 pilot Hold · Gamepad pilot Set"
    : "A/D roll · ↑/↓ pitch · ←/→ yaw · J/L CG · Gamepad sticks";
}

function windDirectionDegrees(north: number, east: number): number | null {
  if (north === 0 && east === 0) return null;
  return normalizeDegrees(Math.atan2(east, north) * 180 / Math.PI);
}

function terminalLabel(terminal: FlightSnapshot["terminal"] | TailTerminalReason): string {
  switch (terminal) {
    case "airborne": return "滑空中";
    case "water_contact":
    case "water-contact": return "着水";
    case "time_limit":
    case "time-limit": return "時間制限";
    case "out_of_valid_envelope":
    case "out-of-valid-envelope": return "空力モデルの適用範囲外";
    case "manual_abort":
    case "manual-abort": return "手動終了";
    case "fatal_simulation_error":
    case "fatal-simulation-error": return "シミュレーションエラー";
  }
}
