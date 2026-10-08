import { describe, expect, it } from "vitest";
import { chartTextPosition, deriveScreenChartLayout } from "../../web/src/presentation/screen-chart-layout.js";

describe("screen chart pixel layout", () => {
  it.each([{ width: 1144, height: 190 }, { width: 298, height: 160 }, { width: 1144, height: 160 }])(
    "keeps readable fonts and equal map scale at $width × $height",
    (viewport) => {
      const layout = deriveScreenChartLayout(viewport, true);
      expect(layout.viewport).toEqual(viewport);
      expect(layout.tickFontSize).toBeGreaterThanOrEqual(14);
      expect(layout.axisFontSize).toBeGreaterThanOrEqual(16);
      expect(layout.plot.right - layout.plot.left).toBeCloseTo(layout.plot.bottom - layout.plot.top);
      expect(layout.plot.left).toBeGreaterThan(0);
      expect(layout.plot.top).toBeGreaterThan(0);
      expect(layout.plot.right).toBeLessThan(viewport.width);
      expect(layout.plot.bottom).toBeLessThan(layout.xAxisY);
      expect(layout.xAxisY).toBeLessThan(viewport.height);
      expect(layout.xTickIntervals).toBeLessThanOrEqual(4);
      expect(layout.yTickIntervals).toBeLessThanOrEqual(4);
      const label = chartTextPosition(viewport.width, 0, "WaterContact", layout);
      expect(label.x).toBeGreaterThanOrEqual(4);
      expect(label.x + "WaterContact".length * layout.tickFontSize * 0.65).toBeLessThan(viewport.width);
      expect(label.y).toBeGreaterThanOrEqual(layout.tickFontSize);
    }
  );

  it("uses the available rectangle for time-series plots and rejects invalid viewport sizes", () => {
    const viewport = { width: 1144, height: 190 };
    const layout = deriveScreenChartLayout(viewport, false);
    expect(layout.plot.right - layout.plot.left).toBeGreaterThan(layout.plot.bottom - layout.plot.top);
    expect(layout).toEqual(deriveScreenChartLayout(viewport, false));
    expect(() => deriveScreenChartLayout({ width: Number.NaN, height: 160 }, true)).toThrow(RangeError);
    expect(() => deriveScreenChartLayout({ width: 298, height: 0 }, true)).toThrow(RangeError);
  });
});
