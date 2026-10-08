import { fitPlotRectToEqualScale } from "../render/contracts/ui.js";

export interface ScreenChartViewport {
  readonly width: number;
  readonly height: number;
}

export interface ScreenChartLayout {
  readonly viewport: ScreenChartViewport;
  readonly plot: Readonly<{ left: number; top: number; right: number; bottom: number }>;
  readonly tickFontSize: number;
  readonly axisFontSize: number;
  readonly xTickIntervals: number;
  readonly yTickIntervals: number;
  readonly xAxisY: number;
  readonly yAxisX: number;
  readonly yAxisY: number;
}

export const INITIAL_SCREEN_CHART_VIEWPORT: ScreenChartViewport = Object.freeze({ width: 1000, height: 520 });

export function deriveScreenChartLayout(viewport: ScreenChartViewport, equalAxisScale: boolean): ScreenChartLayout {
  if (![viewport.width, viewport.height].every(Number.isFinite) || viewport.width < 140 || viewport.height < 100) {
    throw new RangeError("Screen chart viewport must have finite dimensions and space for its axes");
  }
  const plot = fitPlotRectToEqualScale(64, 26, viewport.width - 20, viewport.height - 48, equalAxisScale);
  return Object.freeze({
    viewport: Object.freeze({ ...viewport }),
    plot,
    tickFontSize: 14,
    axisFontSize: 16,
    xTickIntervals: Math.max(1, Math.min(4, Math.floor((plot.right - plot.left) / 80))),
    yTickIntervals: Math.max(1, Math.min(4, Math.floor((plot.bottom - plot.top) / 32))),
    xAxisY: viewport.height - 8,
    yAxisX: 16,
    yAxisY: (plot.top + plot.bottom) / 2
  });
}

export function chartTextPosition(
  preferredX: number,
  preferredY: number,
  text: string,
  layout: ScreenChartLayout
): Readonly<{ x: number; y: number }> {
  const estimatedWidth = text.length * layout.tickFontSize * 0.65;
  return Object.freeze({
    x: Math.max(4, Math.min(layout.viewport.width - estimatedWidth - 4, preferredX)),
    y: Math.max(layout.tickFontSize + 2, Math.min(layout.viewport.height - 4, preferredY))
  });
}
