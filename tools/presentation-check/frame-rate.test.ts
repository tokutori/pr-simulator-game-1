import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { FrameRateCounter, formatFrameRate, ScreenFrameRateDisplay } from "../../web/src/presentation/frame-rate.js";

describe("Presentation-only render frame rate", () => {
  it.each([30, 60, 80, 120])("measures %i rendered frames per second independently of the 100 Hz physics clock", (rate) => {
    const counter = new FrameRateCounter();
    for (let frame = 0; frame <= rate; frame++) counter.observe(frame * 1000 / rate);
    expect(counter.framesPerSecond).toBeCloseTo(rate, 10);
    expect(formatFrameRate(counter.framesPerSecond)).toBe(`FPS ${String(rate)}.0`);
  });

  it("uses elapsed time rather than averaging instantaneous reciprocal frame times", () => {
    const counter = new FrameRateCounter();
    for (const timestamp of [0, 100, 150, 250, 750]) counter.observe(timestamp);
    expect(counter.framesPerSecond).toBeCloseTo(4 * 1000 / 750, 10);
    expect(formatFrameRate(counter.framesPerSecond)).toBe("FPS 5.3");
  });

  it("does not invent values during warmup, duplicate timestamps, gaps or clock resets", () => {
    const counter = new FrameRateCounter();
    counter.observe(0);
    counter.observe(0);
    counter.observe(500);
    expect(counter.framesPerSecond).toBeNull();
    counter.observe(750);
    expect(counter.framesPerSecond).toBeCloseTo(2 * 1000 / 750, 10);
    counter.observe(5000);
    expect(counter.framesPerSecond).toBeNull();
    counter.observe(100);
    expect(counter.framesPerSecond).toBeNull();
    for (const invalid of [Number.NaN, Number.POSITIVE_INFINITY, -1]) {
      counter.observe(invalid);
      expect(counter.framesPerSecond).toBeNull();
    }
    counter.reset();
    expect(formatFrameRate(counter.framesPerSecond)).toBe("FPS —");
  });

  it("does not present non-finite or non-positive numbers as successful FPS measurements", () => {
    for (const value of [null, Number.NaN, Number.POSITIVE_INFINITY, -1, 0]) expect(formatFrameRate(value)).toBe("FPS —");
  });

  it("shows a separate Screen readout and hides the DOM overlay in VR", async () => {
    const window = new Window();
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("section");
    const display = new ScreenFrameRateDisplay(root);
    const output = root.querySelector("output");
    if (output === null) throw new Error("FPS output missing");
    expect(output.getAttribute("aria-label")).toBe("描画フレームレート");
    display.render(60, true);
    expect(output.textContent).toBe("FPS 60.0");
    expect(output.hidden).toBe(false);
    display.render(90, false);
    expect(output.textContent).toBe("FPS 90.0");
    expect(output.hidden).toBe(true);
    display.render(null, true);
    expect(output.textContent).toBe("FPS —");
    await window.happyDOM.abort();
  });
});
