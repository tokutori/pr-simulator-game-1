import type { FlightSnapshot } from "../game/flight-snapshot.js";

export type InformationLevelCode = 0 | 1 | 2 | 3;

export interface FlightHudModel {
  readonly status: string;
  readonly attitude: Readonly<{ rollDegrees: number; pitchDegrees: number }> | null;
  readonly readouts: string;
  readonly heading: string | null;
  readonly pilotPosition: string | null;
  readonly wind: string | null;
  readonly angleOfAttack: string | null;
  readonly telemetry: string;
}

export function createFlightHudModel(snapshot: FlightSnapshot, informationCode: InformationLevelCode): FlightHudModel {
  const telemetry = snapshot.telemetry;
  const distance = `距離 ${snapshot.scoreCourseMeters.toFixed(1)} m`;
  const duration = `時間 ${snapshot.flightTimeSeconds.toFixed(1)} s`;
  if (telemetry === null) {
    return Object.freeze({
      status: terminalLabel(snapshot.terminal),
      attitude: null,
      readouts: informationCode === 2 ? `${distance} · ${duration}` : `計器データ unavailable · ${distance} · ${duration}`,
      heading: null,
      pilotPosition: informationCode === 0 ? `${snapshot.pilotPositionMeters.toFixed(2)} m` : null,
      wind: informationCode === 0 ? "unavailable" : null,
      angleOfAttack: informationCode === 0 ? "unavailable" : null,
      telemetry: "telemetry unavailable"
    });
  }

  const rollDegrees = telemetry.rollRadians * 180 / Math.PI;
  const pitchDegrees = telemetry.pitchRadians * 180 / Math.PI;
  const headingDegrees = ((telemetry.headingRadians * 180 / Math.PI) % 360 + 360) % 360;
  const airspeed = `IAS ${telemetry.airspeedMetersPerSecond.toFixed(1)} m/s`;
  const altitude = `ALT ${telemetry.altitudeMeters.toFixed(1)} m`;
  const attitudeText = `PITCH ${pitchDegrees.toFixed(0)}° · ROLL ${rollDegrees.toFixed(0)}°`;
  const attitude = Object.freeze({ rollDegrees, pitchDegrees });

  switch (informationCode) {
    case 0: {
      const wind = telemetry.windVelocityNedMetersPerSecond;
      const groundspeed = `対地速度 ${telemetry.groundspeedMetersPerSecond.toFixed(1)} m/s`;
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
        attitude,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: `${snapshot.pilotPositionMeters >= 0 ? "+" : ""}${snapshot.pilotPositionMeters.toFixed(2)} m`,
        wind: `N ${wind.north.toFixed(1)} · E ${wind.east.toFixed(1)} · D ${wind.down.toFixed(1)} m/s`,
        angleOfAttack: telemetry.angleOfAttackRadians === null
          ? "—"
          : `${(telemetry.angleOfAttackRadians * 180 / Math.PI).toFixed(1)}°`,
        telemetry: `${groundspeed} · ${distance} · ${duration}`
      });
    }
    case 1:
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
        attitude,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: null,
        wind: null,
        angleOfAttack: null,
        telemetry: `対地速度 ${telemetry.groundspeedMetersPerSecond.toFixed(1)} m/s · ${distance} · ${duration}`
      });
    case 2:
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
        attitude: null,
        readouts: `${altitude} · ${distance} · ${duration}`,
        heading: null,
        pilotPosition: null,
        wind: null,
        angleOfAttack: null,
        telemetry: ""
      });
    case 3:
      return Object.freeze({
        status: terminalLabel(snapshot.terminal),
        attitude,
        readouts: `${airspeed} · ${altitude}\n${attitudeText}`,
        heading: `${headingDegrees.toFixed(0)}°`,
        pilotPosition: null,
        wind: null,
        angleOfAttack: null,
        telemetry: "代表計器表示 · 対象機の実機構成は未確認"
      });
  }
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
