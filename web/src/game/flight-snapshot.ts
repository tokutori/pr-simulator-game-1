export const FLIGHT_SNAPSHOT_LENGTH = 20;

export interface FlightSnapshot {
  readonly tick: number;
  readonly positionNed: Readonly<{ north: number; east: number; down: number }>;
  readonly velocityNed: Readonly<{ north: number; east: number; down: number }>;
  readonly attitudeBodyToNed: Readonly<{ w: number; x: number; y: number; z: number }>;
  readonly pilotPositionMeters: number;
  readonly pilotVelocityMetersPerSecond: number;
  readonly actuatorDeflectionRadians: Readonly<{ roll: number; pitch: number; yaw: number }>;
  readonly terminal: "airborne" | "water-contact" | "time-limit";
  readonly scoreCourseMeters: number;
  readonly crossTrackMeters: number;
  readonly contactFraction: number | null;
}

export function parseFlightSnapshot(values: ArrayLike<number>): FlightSnapshot {
  if (values.length !== FLIGHT_SNAPSHOT_LENGTH) {
    throw new RangeError(`Flight snapshot must contain ${String(FLIGHT_SNAPSHOT_LENGTH)} values`);
  }
  const snapshot = Array.from({ length: FLIGHT_SNAPSHOT_LENGTH }, (_, index) => values[index]);
  if (snapshot.some((value) => value === undefined || !Number.isFinite(value))) {
    throw new RangeError("Flight snapshot values must be finite");
  }

  const [
    tick,
    north,
    east,
    down,
    velocityNorth,
    velocityEast,
    velocityDown,
    attitudeW,
    attitudeX,
    attitudeY,
    attitudeZ,
    pilotPositionMeters,
    pilotVelocityMetersPerSecond,
    actuatorRoll,
    actuatorPitch,
    actuatorYaw,
    terminalCode,
    scoreCourseMeters,
    crossTrackMeters,
    contactFraction
  ] = snapshot as [number, number, number, number, number, number, number, number, number, number,
    number, number, number, number, number, number, number, number, number, number];
  const terminal = terminalFromCode(terminalCode);
  if (!Number.isInteger(tick) || tick < 0) throw new RangeError("Flight tick must be a nonnegative integer");
  if (terminal === "water-contact" && (contactFraction < 0 || contactFraction > 1)) {
    throw new RangeError("Water-contact fraction must lie in [0, 1]");
  }
  if (terminal !== "water-contact" && contactFraction !== -1) {
    throw new RangeError("Non-contact snapshots must use the sentinel contact fraction");
  }
  const attitudeNorm = Math.hypot(attitudeW, attitudeX, attitudeY, attitudeZ);
  if (Math.abs(attitudeNorm - 1) > 1e-8) throw new RangeError("Flight attitude must be a unit quaternion");

  return Object.freeze({
    tick,
    positionNed: Object.freeze({ north, east, down }),
    velocityNed: Object.freeze({ north: velocityNorth, east: velocityEast, down: velocityDown }),
    attitudeBodyToNed: Object.freeze({ w: attitudeW, x: attitudeX, y: attitudeY, z: attitudeZ }),
    pilotPositionMeters,
    pilotVelocityMetersPerSecond,
    actuatorDeflectionRadians: Object.freeze({ roll: actuatorRoll, pitch: actuatorPitch, yaw: actuatorYaw }),
    terminal,
    scoreCourseMeters,
    crossTrackMeters,
    contactFraction: terminal === "water-contact" ? contactFraction : null
  });
}

function terminalFromCode(code: number): FlightSnapshot["terminal"] {
  switch (code) {
    case 0: return "airborne";
    case 1: return "water-contact";
    case 2: return "time-limit";
    default: throw new RangeError(`Unknown flight terminal code: ${String(code)}`);
  }
}
