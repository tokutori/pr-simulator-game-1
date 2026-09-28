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
  clear(): void;
}

export class FlightController {
  private readonly clock: FixedTickClock;
  private snapshotValue: FlightSnapshot;
  private disposed = false;
  private failed = false;

  constructor(
    private readonly session: FlightSessionPort,
    private readonly input: PilotInputPort,
    private readonly renderer: FlightPosePort,
    private readonly hud: FlightHudPort,
    physicsHz: number,
    private readonly readGamepads: () => readonly (Gamepad | null)[] = readAvailableGamepads
  ) {
    if (!Number.isFinite(physicsHz) || physicsHz <= 0) throw new RangeError("Physics frequency must be positive and finite");
    this.clock = new FixedTickClock(1_000 / physicsHz);
    this.snapshotValue = parseFlightSnapshot(session.snapshot());
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

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.input.dispose();
    this.session.free();
    this.renderer.setFlightPose(null);
    this.hud.clear();
  }

  private applySnapshot(snapshot: FlightSnapshot): void {
    const pose: FlightRenderPose = Object.freeze({
      datumPositionNed: snapshot.positionNed,
      attitudeBodyToNed: snapshot.attitudeBodyToNed
    });
    this.renderer.setFlightPose(pose);
    this.hud.render(snapshot);
  }
}

function readAvailableGamepads(): readonly (Gamepad | null)[] {
  if (typeof navigator === "undefined" || typeof navigator.getGamepads !== "function") return [];
  return navigator.getGamepads();
}
