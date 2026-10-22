import type { TailSnapshotDocument } from "../generated/tail-snapshot.js";
import type { FlightFrameDocument } from "../generated/flight-frame.js";
import type { FlightGamepadObservation } from "../generated/flight-gamepad.js";
import type { FlightKeyObservation } from "../generated/flight-key.js";
import type { FlightRenderPose } from "../render/contracts/runtime.js";
import type { TailLogicalInput } from "./tail-input.js";

export interface FlightPosePort {
  setFlightPose(pose: FlightRenderPose | null): void;
}

export interface TailSessionPort {
  readSnapshot(): TailSnapshotDocument;
  advanceTick(input: TailLogicalInput): TailSnapshotDocument;
  advanceFrame(timestampMilliseconds: number, gamepads: readonly FlightGamepadObservation[], available: boolean): FlightFrameDocument;
  observeFlightKey(observation: FlightKeyObservation): boolean;
  invalidateFlightInput(): void;
  failAdapter(): void;
  pause(reason: number): void;
  clear_pause_reason(reason: number): void;
  free(): void;
}
export interface FlightInputDevice {
  readGamepads(): readonly FlightGamepadObservation[];
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
  private execution: "ready" | "failed" | "disposed" = "ready";
  private snapshotValue: TailControllerSnapshot;
  private generation = Symbol("flight display generation");

  constructor(private readonly session: TailSessionPort, private readonly input: FlightInputDevice,
    private readonly renderer: FlightPosePort, private hud: TailFlightHudPort,
    private onTerminal: (snapshot: TailResultSnapshot) => void = () => undefined,
    private readonly onPauseChanged: () => void = () => undefined) {
    try {
      this.snapshotValue = this.decodeSnapshot(session.readSnapshot());
      this.applySnapshot(this.snapshotValue);
    } catch (error: unknown) {
      this.session.failAdapter();
      throw error;
    }
  }

  get currentSnapshot(): TailControllerSnapshot { return this.snapshotValue; }

  onFrame(timestampMilliseconds: number, available = true): void {
    if (this.execution !== "ready") return;
    const generation = this.generation;
    try {
      const gamepads = this.input.readGamepads();
      if (!this.isCurrentGeneration(generation)) return;
      const report = this.session.advanceFrame(timestampMilliseconds, gamepads, available);
      if (!this.isCurrentGeneration(generation)) return;
      if (report.snapshot === null) {
        if (report.phase_changed) this.onPauseChanged();
        return;
      }
      const wasTerminal = this.snapshotValue.phase === "result";
      const next = this.decodeSnapshot(report.snapshot);
      this.snapshotValue = next;
      if (next.phase === "result" && !wasTerminal) {
        this.onTerminal(next as TailResultSnapshot);
        if (!this.isCurrentFrame(generation, next)) return;
      }
      if (report.phase_changed && next.phase !== "result") {
        this.onPauseChanged();
        if (!this.isCurrentFrame(generation, next)) return;
      }
      this.renderer.setFlightPose(report.render_pose ?? next.render_pose);
      if (!this.isCurrentFrame(generation, next)) return;
      this.hud.render(next);
    } catch (error: unknown) {
      this.fail(error, generation);
    }
  }

  invalidateInput(): void {
    if (this.execution !== "disposed") this.session.invalidateFlightInput();
  }

  synchronizeSnapshot(snapshot: TailSnapshotDocument): void {
    if (this.execution !== "ready") return;
    const generation = this.generation;
    try {
      const next = this.decodeSnapshot(snapshot);
      this.snapshotValue = next;
      this.applySnapshot(next);
    } catch (error: unknown) {
      this.fail(error, generation);
      throw error;
    }
  }

  reset(snapshot: TailSnapshotDocument, hud: TailFlightHudPort = this.hud,
    onTerminal: (snapshot: TailResultSnapshot) => void = this.onTerminal): void {
    if (this.execution === "disposed") throw new Error("Cannot reset a disposed flight display");
    this.generation = Symbol("reset flight display generation");
    this.execution = "failed";
    const generation = this.generation;
    try {
      const next = this.decodeSnapshot(snapshot);
      this.renderer.setFlightPose(next.render_pose);
      if (!this.isCurrentGeneration(generation)) return;
      hud.render(next);
      if (!this.isCurrentGeneration(generation)) return;
      this.snapshotValue = next;
      this.hud = hud;
      this.onTerminal = onTerminal;
      this.execution = "ready";
    } catch (error: unknown) {
      this.fail(error, generation);
      throw error;
    }
  }

  renderCurrentSnapshot(): void {
    if (this.execution !== "disposed") this.applySnapshot(this.snapshotValue);
  }

  dispose(): void {
    if (this.execution === "disposed") return;
    this.generation = Symbol("disposed flight display generation");
    this.execution = "disposed";
    const failures: unknown[] = [];
    for (const cleanup of [
      () => { this.input.dispose(); },
      () => { this.session.free(); },
      () => { this.renderer.setFlightPose(null); },
      () => { this.hud.setVisible(false); }
    ]) {
      try { cleanup(); } catch (error: unknown) { failures.push(error); }
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, "Flight display cleanup failed", { cause: failures[0] });
  }

  private fail(error: unknown, generation: symbol): void {
    if (!this.isCurrentGeneration(generation)) return;
    this.execution = "failed";
    let message = error instanceof Error ? error.message : String(error);
    try { this.session.failAdapter(); }
    catch (failure: unknown) { message += `; execution stop failed: ${String(failure)}`; }
    if (!this.isCurrentGeneration(generation)) return;
    try {
      const retained = this.decodeSnapshot(this.session.readSnapshot());
      if (!this.isCurrentGeneration(generation)) return;
      this.snapshotValue = retained;
    } catch (failure: unknown) { message += `; snapshot refresh failed: ${String(failure)}`; }
    if (this.isCurrentGeneration(generation)) this.hud.fail(message);
  }

  private decodeSnapshot(snapshot: TailSnapshotDocument): TailControllerSnapshot {
    if (snapshot.phase !== "flight_running" && snapshot.phase !== "flight_paused" && snapshot.phase !== "result") {
      throw new RangeError("Flight display requires a Rust flight or Result snapshot");
    }
    return snapshot as TailControllerSnapshot;
  }

  private isCurrentGeneration(generation: symbol): boolean {
    return this.generation === generation && this.execution !== "disposed";
  }

  private isCurrentFrame(generation: symbol, snapshot: TailControllerSnapshot): boolean {
    return this.isCurrentGeneration(generation) && this.execution === "ready" && this.snapshotValue === snapshot;
  }

  private applySnapshot(snapshot: TailControllerSnapshot): void {
    const generation = this.generation;
    this.renderer.setFlightPose(snapshot.render_pose);
    if (this.isCurrentFrame(generation, snapshot)) this.hud.render(snapshot);
  }
}
