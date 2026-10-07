import { FixedTickClock } from "./fixed-tick-clock.js";
import { projectFlightRenderPose, projectTailFlightSnapshot } from "./flight-display-snapshot.js";
import type { FlightDisplaySnapshot } from "./flight-display-snapshot.js";
import type { FlightPosePort } from "./flight-controller.js";
import { parseTailControlProfile, tailInputFromControlProfile, validateTailControlProfileSession } from "./tail-control-profile.js";
import type { TailControlProfile } from "./tail-control-profile.js";
import type { TailPilotDemand } from "./tail-device-input.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "./tail-session-codec.js";
import type { TailSessionSnapshot } from "./tail-session-codec.js";

export interface TailSessionPort {
  snapshot_json(): string;
  control_profile_json(): string;
  advance_tick_json(json: string): string;
  free(): void;
}
export interface TailPilotInputPort {
  readDemand(gamepads: readonly (Gamepad | null)[], heldTargetNormalized: number): TailPilotDemand;
  reset(heldTargetNormalized: number): void;
  suspend(): void;
  resume(): void;
  dispose(): void;
}
export interface TailFlightHudPort {
  render(snapshot: FlightDisplaySnapshot): void;
  fail(message: string): void;
  setVisible(visible: boolean): void;
}
export type TailControllerSnapshot = Extract<TailSessionSnapshot, { phaseCode: 5 | 6 | 7 }>;
export type TailResultSnapshot = Extract<TailSessionSnapshot, { phaseCode: 7 }>;

export class TailFlightController {
  private readonly clock: FixedTickClock;
  private readonly physicsHz: number;
  private execution: "running" | "suspended" | "failed" | "disposed" = "running";
  private snapshotValue: TailControllerSnapshot;
  private profile: TailControlProfile;
  private initialPilotPositionMeters: number;
  private generation = Symbol("tail controller generation");

  constructor(private readonly session: TailSessionPort, private readonly input: TailPilotInputPort,
    private readonly renderer: FlightPosePort, private hud: TailFlightHudPort, physicsHz: number,
    private readonly readGamepads: () => readonly (Gamepad | null)[] = availableGamepads,
    private readonly onTerminal: (snapshot: TailResultSnapshot) => void = () => undefined) {
    if (!Number.isSafeInteger(physicsHz) || physicsHz <= 0) throw new RangeError("Tail physics frequency must be a positive integer");
    this.physicsHz = physicsHz;
    this.clock = new FixedTickClock(1_000 / physicsHz);
    this.snapshotValue = this.decodeSnapshot(session.snapshot_json());
    this.profile = parseTailControlProfile(session.control_profile_json(), this.snapshotValue);
    this.initialPilotPositionMeters = this.snapshotValue.frame.state.pilotPositionMeters;
    this.initializeInput(this.snapshotValue);
    this.applySnapshot(this.snapshotValue);
    if (this.snapshotValue.phaseCode !== 5) this.suspend();
  }

  get currentSnapshot(): TailControllerSnapshot {
    return this.snapshotValue;
  }

  get currentDisplaySnapshot(): FlightDisplaySnapshot {
    return displaySnapshot(this.snapshotValue);
  }

  onFrame(timestampMs: number): void {
    if (this.execution !== "running" || this.snapshotValue.phaseCode !== 5) return;
    const generation = this.generation;
    const completion: { value: Readonly<{ kind: "none" }> | Readonly<{ kind: "terminal"; snapshot: TailResultSnapshot }> } = { value: { kind: "none" } };
    try {
      const advancedTicks = this.clock.advanceFrame(timestampMs, () => {
        const demand = this.input.readDemand(this.readGamepads(), this.snapshotValue.frame.state.pilotPositionTargetNormalized);
        const intent = tailInputFromControlProfile(demand, this.profile, this.snapshotValue);
        const next = this.decodeSnapshot(this.session.advance_tick_json(encodeTailLogicalInput(intent)));
        validateTailControlProfileSession(this.profile, next);
        this.snapshotValue = next;
        if (next.phaseCode === 7) {
          completion.value = { kind: "terminal", snapshot: next };
          return false;
        }
        return next.phaseCode === 5;
      });
      const completed = completion.value;
      if (completed.kind === "terminal") {
        this.onTerminal(completed.snapshot);
        if (!this.isCurrentFrame(generation, completed.snapshot)) return;
      }
      if (advancedTicks === 0 || !this.isCurrentGeneration(generation)) return;
      const committed = this.snapshotValue;
      if (!this.isCurrentFrame(generation, committed)) return;
      this.renderer.setFlightPose(projectFlightRenderPose(displaySnapshot(committed), this.initialPilotPositionMeters));
      if (!this.isCurrentFrame(generation, committed)) return;
      this.hud.render(displaySnapshot(committed));
      if (!this.isCurrentFrame(generation, committed)) return;
      if (completed.kind === "terminal") this.input.suspend();
    } catch (error: unknown) {
      if (!this.isCurrentGeneration(generation)) return;
      this.execution = "failed";
      this.clock.suspend();
      let message = error instanceof Error ? error.message : String(error);
      try {
        this.input.suspend();
      } catch (cleanup: unknown) {
        message += `; input suspension failed: ${cleanup instanceof Error ? cleanup.message : String(cleanup)}`;
      }
      if (!this.isCurrentGeneration(generation)) return;
      this.hud.fail(message);
    }
  }

  suspend(): void {
    if (this.execution === "disposed") return;
    this.clock.suspend();
    this.input.suspend();
    if (this.execution !== "failed") this.execution = "suspended";
  }

  resume(): void {
    if (this.execution === "failed" || this.execution === "disposed" || this.snapshotValue.phaseCode === 7) return;
    this.synchronizeSnapshot(this.session.snapshot_json());
    if (this.snapshotValue.phaseCode !== 5) return;
    this.input.resume();
    this.clock.resume();
    this.execution = "running";
  }

  synchronizeSnapshot(json: string): void {
    if (this.execution === "failed" || this.execution === "disposed") return;
    const previousExecution = this.execution;
    this.execution = "failed";
    this.clock.suspend();
    try {
      const next = this.decodeSnapshot(json);
      validateTailControlProfileSession(this.profile, next);
      this.applySnapshot(next);
      this.snapshotValue = next;
      if (previousExecution === "running" && next.phaseCode === 5) {
        this.clock.resume();
        this.execution = "running";
      } else {
        this.input.suspend();
        this.execution = "suspended";
      }
    } catch (error: unknown) {
      let failure = error;
      try {
        this.input.suspend();
      } catch (cleanup: unknown) {
        failure = new AggregateError([error, cleanup], "Tail synchronization and input suspension failed", { cause: error });
      }
      throw failure;
    }
  }

  reset(json: string, hud: TailFlightHudPort = this.hud): void {
    if (this.execution === "disposed") throw new Error("Cannot reset a disposed tail controller");
    this.generation = Symbol("reset tail controller generation");
    this.execution = "failed";
    this.clock.suspend();
    try {
      const next = this.decodeSnapshot(json);
      const profile = parseTailControlProfile(this.session.control_profile_json(), next);
      this.initializeInput(next);
      this.applySnapshot(next, next.frame.state.pilotPositionMeters, hud);
      this.snapshotValue = next;
      this.profile = profile;
      this.initialPilotPositionMeters = next.frame.state.pilotPositionMeters;
      this.hud = hud;
      this.clock.reset();
      if (next.phaseCode === 5) this.execution = "running";
      else {
        this.clock.suspend();
        this.execution = "suspended";
      }
    } catch (error: unknown) {
      let failure = error;
      try {
        this.input.suspend();
      } catch (cleanup: unknown) {
        failure = new AggregateError([error, cleanup], "Tail reset and input suspension failed", { cause: error });
      }
      throw failure;
    }
  }

  renderCurrentSnapshot(): void {
    if (this.execution === "disposed") return;
    this.applySnapshot(this.snapshotValue);
  }

  dispose(): void {
    if (this.execution === "disposed") return;
    this.generation = Symbol("disposed tail controller generation");
    this.execution = "disposed";
    this.input.dispose();
    this.session.free();
    this.renderer.setFlightPose(null);
    this.hud.setVisible(false);
  }

  private decodeSnapshot(json: string): TailControllerSnapshot {
    const snapshot = parseTailSessionSnapshot(json, this.physicsHz);
    if (snapshot.phaseCode !== 5 && snapshot.phaseCode !== 6 && snapshot.phaseCode !== 7) {
      throw new RangeError("Tail controller requires a Rust flight or Result snapshot");
    }
    return snapshot;
  }

  private isCurrentGeneration(generation: symbol): boolean {
    return this.generation === generation && this.execution !== "disposed";
  }

  private isCurrentFrame(generation: symbol, snapshot: TailControllerSnapshot): boolean {
    return this.isCurrentGeneration(generation) && this.execution === "running" && this.snapshotValue === snapshot;
  }

  private initializeInput(snapshot: TailControllerSnapshot): void {
    this.input.reset(snapshot.frame.state.pilotPositionTargetNormalized);
    if (snapshot.phaseCode === 5) this.input.resume();
    else this.input.suspend();
  }

  private applySnapshot(snapshot: TailControllerSnapshot, initialPilotPositionMeters = this.initialPilotPositionMeters,
    hud: TailFlightHudPort = this.hud): void {
    const display = displaySnapshot(snapshot);
    this.renderer.setFlightPose(projectFlightRenderPose(display, initialPilotPositionMeters));
    hud.render(display);
  }
}

function displaySnapshot(snapshot: TailControllerSnapshot): FlightDisplaySnapshot {
  const projection = projectTailFlightSnapshot(snapshot);
  if (projection.kind !== "available") throw new RangeError("Tail controller has no flight display");
  return projection.value;
}

function availableGamepads(): readonly (Gamepad | null)[] {
  if (typeof navigator === "undefined" || typeof navigator.getGamepads !== "function") return [];
  return navigator.getGamepads();
}
