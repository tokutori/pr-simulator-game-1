import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { HudProfileUiState } from "../app/app-state.js";
import { nedToWgs84 } from "../render/contracts/launch-venue.js";

export type InformationLevelCode = 0 | 1 | 2 | 3 | 4;

export interface FlightHudModel {
  readonly diagnostic?: string;
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
}

export function createFlightHudModel(
  snapshot: FlightSnapshot,
  informationCode: InformationLevelCode,
  customProfile: HudProfileUiState = fullProfile,
  diagnostic: string | null = null
): FlightHudModel {
  const model = deriveFlightHudModel(snapshot, informationCode, customProfile);
  return diagnostic === null ? model : Object.freeze({ ...model, status: "飛行処理停止 · 最後の有効値", diagnostic });
}

function deriveFlightHudModel(
  snapshot: FlightSnapshot,
  informationCode: InformationLevelCode,
  customProfile: HudProfileUiState
): FlightHudModel {
  if (informationCode === 4) return customFlightHudModel(snapshot, customProfile);
  const telemetry = snapshot.telemetry;
  const distance = `距離 ${snapshot.scoreCourseMeters.toFixed(1)} m`;
  const duration = `時間 ${snapshot.flightTimeSeconds.toFixed(1)} s`;
  const coordinates = nedToWgs84(snapshot.positionNed.north, snapshot.positionNed.east);
  const location = `緯度 ${coordinates.latitudeDegrees.toFixed(6)}° · 経度 ${coordinates.longitudeDegrees.toFixed(6)}°`;
  const mapAttribution = "湖岸 © OpenStreetMap contributors · ODbL 1.0";
  if (telemetry === null) {
    return Object.freeze({
      status: terminalLabel(snapshot.terminal),
      attitude: null,
      flightPathAngleDegrees: informationCode === 0 ? flightPathAngle(snapshot) : null,
      warning: informationCode === 0 ? warningFor(snapshot) : null,
      headingDegrees: null,
      pilotPositionRatio: informationCode === 0 ? pilotPositionRatio(snapshot.pilotPositionMeters) : null,
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

  const rollDegrees = telemetry.rollRadians * 180 / Math.PI;
  const pitchDegrees = telemetry.pitchRadians * 180 / Math.PI;
  const headingDegrees = normalizeDegrees(telemetry.headingRadians * 180 / Math.PI);
  const airspeed = `IAS ${telemetry.airspeedMetersPerSecond.toFixed(1)} m/s`;
  const altitude = `ALT ${telemetry.altitudeMeters.toFixed(1)} m`;
  const attitudeText = `PITCH ${pitchDegrees.toFixed(0)}° · ROLL ${rollDegrees.toFixed(0)}°`;
  const attitude = Object.freeze({ rollDegrees, pitchDegrees });
  const angleOfAttackDegrees = telemetry.angleOfAttackRadians === null
    ? null
    : telemetry.angleOfAttackRadians * 180 / Math.PI;

  switch (informationCode) {
    case 0: {
      const wind = telemetry.windVelocityNedMetersPerSecond;
      const groundspeed = `対地速度 ${telemetry.groundspeedMetersPerSecond.toFixed(1)} m/s`;
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
        attitude,
        flightPathAngleDegrees: flightPathAngle(snapshot),
        warning: warningFor(snapshot),
        headingDegrees,
        pilotPositionRatio: pilotPositionRatio(snapshot.pilotPositionMeters),
        windDirectionDegrees: windDirectionDegrees(telemetry.windVelocityNedMetersPerSecond.north, telemetry.windVelocityNedMetersPerSecond.east),
        angleOfAttackDegrees,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: `${snapshot.pilotPositionMeters >= 0 ? "+" : ""}${snapshot.pilotPositionMeters.toFixed(2)} m`,
        wind: `N ${wind.north.toFixed(1)} · E ${wind.east.toFixed(1)} · D ${wind.down.toFixed(1)} m/s`,
        angleOfAttack: telemetry.angleOfAttackRadians === null
          ? "—"
          : `${(telemetry.angleOfAttackRadians * 180 / Math.PI).toFixed(1)}°`,
        telemetry: `${groundspeed} · ${distance} · ${duration}`,
        location,
        mapAttribution
      });
    }
    case 1:
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
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
        telemetry: `対地速度 ${telemetry.groundspeedMetersPerSecond.toFixed(1)} m/s · ${distance} · ${duration}`,
        location,
        mapAttribution
      });
    case 2:
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
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
        status: terminalLabel(snapshot.terminal),
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

function customFlightHudModel(snapshot: FlightSnapshot, profile: HudProfileUiState): FlightHudModel {
  const full = createFlightHudModel(snapshot, 0);
  const telemetry = snapshot.telemetry;
  const heading = profile.attitude ? full.heading : null;
  const angleOfAttack = profile.angleOfAttack ? full.angleOfAttack : null;
  const attitudeReadout = profile.attitude && full.attitude !== null
    ? `PITCH ${full.attitude.pitchDegrees.toFixed(0)}° · ROLL ${full.attitude.rollDegrees.toFixed(0)}°`
    : null;
  const telemetryReadout = profile.telemetry && telemetry !== null
    ? `IAS ${telemetry.airspeedMetersPerSecond.toFixed(1)} m/s · ALT ${telemetry.altitudeMeters.toFixed(1)} m`
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
    telemetry: profile.telemetry ? full.telemetry : ""
  });
}

function flightPathAngle(snapshot: FlightSnapshot): number | null {
  const horizontalSpeed = Math.hypot(snapshot.velocityNed.north, snapshot.velocityNed.east);
  if (horizontalSpeed === 0) return null;
  return Math.atan2(-snapshot.velocityNed.down, horizontalSpeed) * 180 / Math.PI;
}

function warningFor(snapshot: FlightSnapshot): string | null {
  switch (snapshot.terminal) {
    case "out-of-valid-envelope": return "AERODYNAMIC ENVELOPE";
    case "fatal-simulation-error": return "SIMULATION FAILURE";
    case "airborne":
    case "water-contact":
    case "time-limit":
    case "manual-abort": return null;
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

function windDirectionDegrees(north: number, east: number): number | null {
  if (north === 0 && east === 0) return null;
  return normalizeDegrees(Math.atan2(east, north) * 180 / Math.PI);
}

function terminalLabel(terminal: FlightSnapshot["terminal"]): string {
  switch (terminal) {
    case "airborne": return "滑空中";
    case "water-contact": return "着水";
    case "time-limit": return "時間制限";
    case "out-of-valid-envelope": return "空力モデルの適用範囲外";
    case "manual-abort": return "手動終了";
    case "fatal-simulation-error": return "シミュレーションエラー";
  }
}
