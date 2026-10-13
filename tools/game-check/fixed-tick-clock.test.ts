import { describe, expect, it, vi } from "vitest";
import { FixedTickClock } from "../../web/src/game/fixed-tick-clock.js";

describe("bounded fixed flight clock", () => {
  it.each([30, 60, 120])("retains 500 physics ticks at %s FPS", (fps) => {
    const clock = new FixedTickClock(10, 150);
    const tick = vi.fn(() => true);
    expect(clock.advanceFrame(10_000, tick)).toEqual({ kind: "ticks", count: 0 });
    for (let frame = 1; frame <= fps * 5; frame += 1) clock.advanceFrame(10_000 + frame * 1_000 / fps, tick);
    expect(tick).toHaveBeenCalledTimes(500);
  });

  it.each([149.999, 150, 150.001])("classifies the strict 150 ms boundary at %s ms before ticking", (gap) => {
    const clock = new FixedTickClock(10, 150);
    const tick = vi.fn(() => true);
    clock.advanceFrame(0, tick);
    const result = clock.advanceFrame(gap, tick);
    if (gap > 150) {
      expect(result).toEqual({ kind: "processing-delay", blocked: true });
      expect(tick).not.toHaveBeenCalled();
    } else {
      expect(result).toEqual({ kind: "ticks", count: Math.floor(gap / 10) });
      expect(tick).toHaveBeenCalledTimes(Math.floor(gap / 10));
    }
  });

  it("observes repeated gaps while suspended and resets debt only at explicit boundaries", () => {
    const clock = new FixedTickClock(10, 150);
    const tick = vi.fn(() => true);
    clock.advanceFrame(0, tick);
    clock.advanceFrame(5, tick);
    expect(clock.advanceFrame(1_005, tick)).toEqual({ kind: "processing-delay", blocked: true });
    clock.suspend();
    expect(clock.advanceFrame(2_005, tick)).toEqual({ kind: "ticks", count: 0 });
    expect(clock.advanceFrame(2_015, tick)).toEqual({ kind: "processing-delay", blocked: false });
    expect(clock.advanceFrame(2_025, tick)).toEqual({ kind: "ticks", count: 0 });
    expect(tick).not.toHaveBeenCalled();
    clock.resume();
    expect(clock.advanceFrame(9_000, tick)).toEqual({ kind: "ticks", count: 0 });
    expect(clock.advanceFrame(9_005, tick)).toEqual({ kind: "ticks", count: 0 });
    expect(clock.advanceFrame(9_010, tick)).toEqual({ kind: "ticks", count: 1 });
    clock.reset();
    expect(clock.advanceFrame(0, tick)).toEqual({ kind: "ticks", count: 0 });
    expect(clock.advanceFrame(10, tick)).toEqual({ kind: "ticks", count: 1 });
  });

  it.each([-1, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY])("rejects invalid timestamps without consuming a tick: %s", (timestamp) => {
    const clock = new FixedTickClock(10, 150);
    const tick = vi.fn(() => true);
    clock.advanceFrame(0, tick);
    expect(() => clock.advanceFrame(timestamp, tick)).toThrow(RangeError);
    expect(clock.advanceFrame(10, tick)).toEqual({ kind: "ticks", count: 1 });
    expect(tick).toHaveBeenCalledTimes(1);
  });

  it("rejects backward time without moving the observation baseline", () => {
    const clock = new FixedTickClock(10, 150);
    const tick = vi.fn(() => true);
    clock.advanceFrame(100, tick);
    expect(() => clock.advanceFrame(99, tick)).toThrow("monotonic");
    expect(clock.advanceFrame(110, tick)).toEqual({ kind: "ticks", count: 1 });
  });

  it("ends catch-up at the terminal callback and clears the remainder on reset", () => {
    const clock = new FixedTickClock(10, 150);
    const terminal = vi.fn(() => false);
    clock.advanceFrame(0, terminal);
    expect(clock.advanceFrame(150, terminal)).toEqual({ kind: "ticks", count: 1 });
    expect(terminal).toHaveBeenCalledTimes(1);
    clock.reset();
    const tick = vi.fn(() => true);
    clock.advanceFrame(1_000, tick);
    expect(clock.advanceFrame(1_010, tick)).toEqual({ kind: "ticks", count: 1 });
  });
});
