export type FrameAdvance = Readonly<{ kind: "ticks"; count: number }>
  | Readonly<{ kind: "processing-delay"; blocked: boolean }>;

export class FixedTickClock {
  private previousTimestampMs: number | null = null;
  private accumulatedMilliseconds = 0;
  private suspended = false;
  private processingDelayed = false;

  constructor(private readonly tickMilliseconds: number, private readonly maximumFrameGapMilliseconds: number) {
    if (!Number.isFinite(tickMilliseconds) || tickMilliseconds <= 0) {
      throw new RangeError("Tick duration must be positive and finite");
    }
    if (!Number.isFinite(maximumFrameGapMilliseconds) || maximumFrameGapMilliseconds < tickMilliseconds) {
      throw new RangeError("Maximum frame gap must be finite and at least one tick");
    }
  }

  advanceFrame(timestampMs: number, advanceTick: () => boolean): FrameAdvance {
    if (!Number.isFinite(timestampMs) || timestampMs < 0) {
      throw new RangeError("Frame timestamp must be nonnegative and finite");
    }
    if (this.previousTimestampMs !== null && timestampMs < this.previousTimestampMs) {
      throw new RangeError("Frame timestamp must be monotonic");
    }
    const elapsed = this.previousTimestampMs === null ? 0 : timestampMs - this.previousTimestampMs;
    this.previousTimestampMs = timestampMs;
    const delayed = elapsed > this.maximumFrameGapMilliseconds;
    if (delayed !== this.processingDelayed) {
      this.processingDelayed = delayed;
      this.accumulatedMilliseconds = 0;
      return { kind: "processing-delay", blocked: delayed };
    }
    if (delayed || this.suspended) return { kind: "ticks", count: 0 };
    this.accumulatedMilliseconds += elapsed;

    let ticks = 0;
    while (this.accumulatedMilliseconds + 1e-9 >= this.tickMilliseconds) {
      const shouldContinue = advanceTick();
      this.accumulatedMilliseconds -= this.tickMilliseconds;
      ticks += 1;
      if (!shouldContinue) break;
    }
    return { kind: "ticks", count: ticks };
  }

  suspend(): void {
    this.suspended = true;
    this.accumulatedMilliseconds = 0;
  }

  resume(): void {
    this.suspended = false;
    this.previousTimestampMs = null;
    this.accumulatedMilliseconds = 0;
  }

  reset(): void {
    this.previousTimestampMs = null;
    this.accumulatedMilliseconds = 0;
    this.suspended = false;
    this.processingDelayed = false;
  }
}
