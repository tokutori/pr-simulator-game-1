export class FixedTickClock {
  private previousTimestampMs: number | null = null;
  private accumulatedMilliseconds = 0;
  private suspended = false;

  constructor(private readonly tickMilliseconds: number) {
    if (!Number.isFinite(tickMilliseconds) || tickMilliseconds <= 0) {
      throw new RangeError("Tick duration must be positive and finite");
    }
  }

  advanceFrame(timestampMs: number, advanceTick: () => void): number {
    if (!Number.isFinite(timestampMs) || timestampMs < 0) {
      throw new RangeError("Frame timestamp must be nonnegative and finite");
    }
    if (this.previousTimestampMs === null) {
      this.previousTimestampMs = timestampMs;
      return 0;
    }
    const elapsed = Math.max(0, timestampMs - this.previousTimestampMs);
    this.previousTimestampMs = timestampMs;
    if (this.suspended) return 0;
    this.accumulatedMilliseconds += elapsed;

    let ticks = 0;
    while (this.accumulatedMilliseconds + 1e-9 >= this.tickMilliseconds) {
      advanceTick();
      this.accumulatedMilliseconds -= this.tickMilliseconds;
      ticks += 1;
    }
    return ticks;
  }

  suspend(): void {
    this.suspended = true;
    this.previousTimestampMs = null;
  }

  resume(): void {
    this.suspended = false;
    this.previousTimestampMs = null;
  }

  reset(): void {
    this.previousTimestampMs = null;
    this.accumulatedMilliseconds = 0;
    this.suspended = false;
  }
}
