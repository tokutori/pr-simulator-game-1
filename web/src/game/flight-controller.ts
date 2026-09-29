import { FixedTickClock } from "./fixed-tick-clock.js";
import type { FlightSnapshot } from "./flight-snapshot.js";
import { parseFlightSnapshot } from "./flight-snapshot.js";
import type { PilotIntent } from "./keyboard-intent.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";

export interface FlightPosePort {
  setFlightPose(pose: FlightRenderPose | null): void;
}

export interface FlightSessionPort {
  advance_tick(roll: number, pitch: number, yaw: number, pilotPositionMeters: number): ArrayLike<number>;
  snapshot(): ArrayLike<number>;
  free(): void;
}

export interface PilotInputPort {
  readIntent(gamepads: readonly (Gamepad | null)[]): PilotIntent;
  dispose(): void;
}

export interface FlightHudPort {
  render(snapshot: FlightSnapshot): void;
  fail(message: string): void;
  setVisible(visible: boolean): void;
}

export class FlightController {
  private readonly clock: FixedTickClock;
  private snapshotValue: FlightSnapshot;
  private disposed = false;
  private failed = false;
  private terminalReported = false;
  private initialPilotPositionMeters: number;

  constructor(
    private readonly session: FlightSessionPort,
    private readonly input: PilotInputPort,
    private readonly renderer: FlightPosePort,
    private readonly hud: FlightHudPort,
    physicsHz: number,
    private readonly readGamepads: () => readonly (Gamepad | null)[] = readAvailableGamepads,
    private readonly onTerminal: (snapshot: FlightSnapshot) => void = () => undefined
  ) {
    if (!Number.isFinite(physicsHz) || physicsHz <= 0) throw new RangeError("Physics frequency must be positive and finite");
    this.clock = new FixedTickClock(1_000 / physicsHz);
    this.snapshotValue = parseFlightSnapshot(session.snapshot());
    this.initialPilotPositionMeters = this.snapshotValue.pilotPositionMeters;
    this.applySnapshot(this.snapshotValue);
  }

  get currentSnapshot(): FlightSnapshot {
    return this.snapshotValue;
  }

  onFrame(timestampMs: number): void {
    if (this.disposed || this.failed || this.snapshotValue.terminal !== "airborne") return;
    try {
      this.clock.advanceFrame(timestampMs, () => {
        const intent = this.input.readIntent(this.readGamepads());
        this.snapshotValue = parseFlightSnapshot(this.session.advance_tick(
          intent.roll,
          intent.pitch,
          intent.yaw,
          intent.pilotPositionMeters
        ));
        this.applySnapshot(this.snapshotValue);
        if (this.snapshotValue.terminal !== "airborne" && !this.terminalReported) {
          this.terminalReported = true;
          this.onTerminal(this.snapshotValue);
        }
        return this.snapshotValue.terminal === "airborne";
      });
    } catch (error: unknown) {
      this.failed = true;
      this.clock.suspend();
      this.hud.fail(error instanceof Error ? error.message : String(error));
    }
  }

  suspend(): void {
    this.clock.suspend();
  }

  resume(): void {
    this.clock.resume();
  }

  reset(snapshot: ArrayLike<number>): void {
    if (this.disposed) throw new Error("Cannot reset a disposed flight controller");
    this.failed = false;
    this.terminalReported = false;
    this.clock.reset();
    this.snapshotValue = parseFlightSnapshot(snapshot);
    this.initialPilotPositionMeters = this.snapshotValue.pilotPositionMeters;
    this.applySnapshot(this.snapshotValue);
  }

  renderCurrentSnapshot(): void {
    if (this.disposed) return;
    this.applySnapshot(this.snapshotValue);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.input.dispose();
    this.session.free();
    this.renderer.setFlightPose(null);
    this.hud.setVisible(false);
  }

  private applySnapshot(snapshot: FlightSnapshot): void {
    const pose: FlightRenderPose = Object.freeze({
      datumPositionNed: snapshot.positionNed,
      attitudeBodyToNed: snapshot.attitudeBodyToNed,
      pilotPositionMeters: snapshot.pilotPositionMeters,
      initialPilotPositionMeters: this.initialPilotPositionMeters,
      simulationTimeSeconds: snapshot.flightTimeSeconds,
      airspeedMetersPerSecond: snapshot.telemetry?.airspeedMetersPerSecond ?? null,
      actuatorDeflectionRadians: snapshot.actuatorDeflectionRadians,
      windVelocityNedMetersPerSecond: snapshot.telemetry?.windVelocityNedMetersPerSecond ?? null
    });
    this.renderer.setFlightPose(pose);
    this.hud.render(snapshot);
  }
}

function readAvailableGamepads(): readonly (Gamepad | null)[] {
  if (typeof navigator === "undefined" || typeof navigator.getGamepads !== "function") return [];
  return navigator.getGamepads();
}
