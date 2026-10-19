import { FixedTickClock } from "./fixed-tick-clock.js";
import type { TailSnapshotDocument } from "../generated/tail-snapshot.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";
import { tailInputFromControlProfile } from "./tail-device-input.js";
import type { TailControlProfileDocument } from "../generated/tail-control-profile.js";
import type { TailPilotDemand } from "./tail-device-input.js";
import type { TailLogicalInput } from "./tail-input.js";

export interface FlightPosePort {
  setFlightPose(pose: FlightRenderPose | null): void;
}

export interface TailSessionPort {
  readSnapshot(): TailSnapshotDocument;
  readControlProfile(): TailControlProfileDocument;
  advanceTick(input: TailLogicalInput): TailSnapshotDocument;
  pause(reason: number): void;
  clear_pause_reason(reason: number): void;
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
  render(snapshot: TailControllerSnapshot): void;
  fail(message: string): void;
  setVisible(visible: boolean): void;
}
export type TailControllerSnapshot = TailSnapshotDocument & Readonly<{ phase: "flight_running" | "flight_paused" | "result"; render_pose: FlightRenderPose; frame: Exclude<TailSnapshotDocument["frame"], { kind: "menu" }> }>;
export type TailResultSnapshot = TailControllerSnapshot & Readonly<{ phase: "result"; frame: Extract<TailSnapshotDocument["frame"], { kind: "result" }> }>;

export class TailFlightController {
  private readonly clock: FixedTickClock;
  private execution: "running" | "suspended" | "failed" | "disposed" = "running";
  private snapshotValue: TailControllerSnapshot;
  private profile: TailControlProfileDocument;
  private generation = Symbol("tail controller generation");

  constructor(private readonly session: TailSessionPort, private readonly input: TailPilotInputPort,
    private readonly renderer: FlightPosePort, private hud: TailFlightHudPort, physicsHz: number, maximumFrameGapMilliseconds: number,
    private readonly readGamepads: () => readonly (Gamepad | null)[] = availableGamepads,
    private onTerminal: (snapshot: TailResultSnapshot) => void = () => undefined,
    private readonly onPauseChanged: () => void = () => undefined) {
    if (!Number.isSafeInteger(physicsHz) || physicsHz <= 0) throw new RangeError("Tail physics frequency must be a positive integer");
    this.clock = new FixedTickClock(1_000 / physicsHz, maximumFrameGapMilliseconds);
    this.snapshotValue = this.decodeSnapshot(session.readSnapshot());
    this.profile = session.readControlProfile();
    this.initializeInput(this.snapshotValue);
    this.applySnapshot(this.snapshotValue);
    if (this.snapshotValue.phase !== "flight_running") this.suspend();
  }

  get currentSnapshot(): TailControllerSnapshot {
    return this.snapshotValue;
  }

  onFrame(timestampMs: number): void {
    if (this.execution === "failed" || this.execution === "disposed" || this.snapshotValue.phase === "result") return;
    const generation = this.generation;
    const completion: { value: Readonly<{ kind: "none" }> | Readonly<{ kind: "terminal"; snapshot: TailResultSnapshot }> } = { value: { kind: "none" } };
    try {
      const advancedTicks = this.clock.advanceFrame(timestampMs, () => {
        const demand = this.input.readDemand(this.readGamepads(), this.snapshotValue.frame.state.pilot_position_target_normalized);
        const intent = tailInputFromControlProfile(demand, this.profile);
        const next = this.decodeSnapshot(this.session.advanceTick(intent));
        this.snapshotValue = next;
        if (next.phase === "result") {
          completion.value = { kind: "terminal", snapshot: next as TailResultSnapshot };
          return false;
        }
        return next.phase === "flight_running";
      });
      if (advancedTicks.kind === "processing-delay") {
        if (advancedTicks.blocked) {
          this.suspend();
          if (!this.isCurrentGeneration(generation)) return;
          this.session.pause(3);
        } else {
          this.session.clear_pause_reason(3);
        }
        if (!this.isCurrentGeneration(generation)) return;
        this.synchronizeSnapshot(this.session.readSnapshot());
        if (!this.isCurrentGeneration(generation)) return;
        this.onPauseChanged();
        return;
      }
      const completed = completion.value;
      if (completed.kind === "terminal") {
        this.onTerminal(completed.snapshot);
        if (!this.isCurrentFrame(generation, completed.snapshot)) return;
      }
      if (advancedTicks.count === 0 || !this.isCurrentGeneration(generation)) return;
      const committed = this.snapshotValue;
      if (!this.isCurrentFrame(generation, committed)) return;
      this.renderer.setFlightPose(committed.render_pose);
      if (!this.isCurrentFrame(generation, committed)) return;
      this.hud.render(committed);
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
    if (this.execution !== "failed") this.execution = "suspended";
    this.input.suspend();
  }

  resume(): void {
    if (this.execution === "failed" || this.execution === "disposed" || this.snapshotValue.phase === "result") return;
    this.synchronizeSnapshot(this.session.readSnapshot());
    if (this.snapshotValue.phase !== "flight_running") return;
    this.input.resume();
    this.clock.resume();
    this.execution = "running";
  }

  synchronizeSnapshot(snapshot: TailSnapshotDocument): void {
    if (this.execution === "failed" || this.execution === "disposed") return;
    const previousExecution = this.execution;
    this.execution = "failed";
    this.clock.suspend();
    try {
      const next = this.decodeSnapshot(snapshot);
      this.applySnapshot(next);
      this.snapshotValue = next;
      if (previousExecution === "running" && next.phase === "flight_running") {
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

  reset(snapshot: TailSnapshotDocument, hud: TailFlightHudPort = this.hud, onTerminal: (snapshot: TailResultSnapshot) => void = this.onTerminal): void {
    if (this.execution === "disposed") throw new Error("Cannot reset a disposed tail controller");
    this.generation = Symbol("reset tail controller generation");
    this.execution = "failed";
    this.clock.suspend();
    try {
      const next = this.decodeSnapshot(snapshot);
      const profile = this.session.readControlProfile();
      this.initializeInput(next);
      this.applySnapshot(next, hud);
      this.snapshotValue = next;
      this.profile = profile;
      this.hud = hud;
      this.onTerminal = onTerminal;
      this.clock.reset();
      if (next.phase === "flight_running") this.execution = "running";
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
    this.clock.suspend();
    const failures: unknown[] = [];
    for (const cleanup of [
      () => { this.input.dispose(); },
      () => { this.session.free(); },
      () => { this.renderer.setFlightPose(null); },
      () => { this.hud.setVisible(false); },
    ]) {
      try {
        cleanup();
      } catch (error: unknown) {
        failures.push(error);
      }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, "Tail controller cleanup failed", { cause: failures[0] });
  }

  private decodeSnapshot(snapshot: TailSnapshotDocument): TailControllerSnapshot {
    if (snapshot.phase !== "flight_running" && snapshot.phase !== "flight_paused" && snapshot.phase !== "result") {
      throw new RangeError("Tail controller requires a Rust flight or Result snapshot");
    }
    return snapshot as TailControllerSnapshot;
  }

  private isCurrentGeneration(generation: symbol): boolean {
    return this.generation === generation && this.execution !== "disposed";
  }

  private isCurrentFrame(generation: symbol, snapshot: TailControllerSnapshot): boolean {
    return this.isCurrentGeneration(generation) && this.execution === "running" && this.snapshotValue === snapshot;
  }

  private initializeInput(snapshot: TailControllerSnapshot): void {
    this.input.reset(snapshot.frame.state.pilot_position_target_normalized);
    if (snapshot.phase === "flight_running") this.input.resume();
    else this.input.suspend();
  }

  private applySnapshot(snapshot: TailControllerSnapshot, hud: TailFlightHudPort = this.hud): void {
    this.renderer.setFlightPose(snapshot.render_pose);
    hud.render(snapshot);
  }
}

function availableGamepads(): readonly (Gamepad | null)[] {
  if (typeof navigator === "undefined" || typeof navigator.getGamepads !== "function") return [];
  return navigator.getGamepads();
}
