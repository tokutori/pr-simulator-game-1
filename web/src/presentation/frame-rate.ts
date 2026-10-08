const SAMPLE_WINDOW_MS = 750;
const MAX_FRAME_GAP_MS = 1000;

export class FrameRateCounter {
  private windowStart: number | null = null;
  private previousTimestamp: number | null = null;
  private intervals = 0;
  private measuredRate: number | null = null;

  get framesPerSecond(): number | null {
    return this.measuredRate;
  }

  observe(timestampMs: number): void {
    if (!Number.isFinite(timestampMs) || timestampMs < 0) {
      this.reset();
      return;
    }
    const previous = this.previousTimestamp;
    if (previous === timestampMs) return;
    if (previous === null || timestampMs < previous || timestampMs - previous > MAX_FRAME_GAP_MS) {
      this.reset();
      this.windowStart = timestampMs;
      this.previousTimestamp = timestampMs;
      return;
    }
    this.previousTimestamp = timestampMs;
    this.intervals++;
    const elapsed = timestampMs - (this.windowStart ?? timestampMs);
    if (elapsed < SAMPLE_WINDOW_MS) return;
    this.measuredRate = this.intervals * 1000 / elapsed;
    this.windowStart = timestampMs;
    this.intervals = 0;
  }

  reset(): void {
    this.windowStart = null;
    this.previousTimestamp = null;
    this.intervals = 0;
    this.measuredRate = null;
  }
}

export function formatFrameRate(framesPerSecond: number | null): string {
  return framesPerSecond !== null && Number.isFinite(framesPerSecond) && framesPerSecond > 0
    ? `FPS ${framesPerSecond.toFixed(1)}`
    : "FPS —";
}

export class ScreenFrameRateDisplay {
  private readonly output: HTMLOutputElement;

  constructor(root: HTMLElement) {
    this.output = root.ownerDocument.createElement("output");
    this.output.className = "render-frame-rate";
    this.output.setAttribute("aria-label", "描画フレームレート");
    this.output.textContent = formatFrameRate(null);
    root.append(this.output);
  }

  render(framesPerSecond: number | null, visible: boolean): void {
    this.output.hidden = !visible;
    const label = formatFrameRate(framesPerSecond);
    if (this.output.textContent !== label) this.output.textContent = label;
  }
}
