import type { SurfaceDrawingContext, SurfaceTextMetrics } from "../../web/src/presentation/browser-canvas.js";
import { prepareHeadHudPresentation } from "../../web/src/presentation/head-hud-canvas.js";
import { DEFAULT_XR_HEAD_HUD_PROFILE } from "../../web/src/generated/xr-head-hud-profile.js";
import type { ViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { HeadHudLayer } from "../../web/src/render/contracts/head-hud.js";
import { withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import type { SharedGameUiFixtureOptions } from "../game-check/shared-game-ui-fixture.js";

export function prepareHeadHudFixture(viewer: ViewerFrame, options: SharedGameUiFixtureOptions = {}): HeadHudLayer {
  return withSharedGameUiSession("flight_running", { ...options, platform: { ...options.platform, presentation_mode: "hmd" } }, (session, platform) => {
    const view = session.readSharedUi(platform);
    const reply = prepareHeadHudPresentation(new HudCanvasFixture(), { viewRevision: view.view_revision, viewer,
      profile: DEFAULT_XR_HEAD_HUD_PROFILE, locale: "ja", frameRateLabel: null,
      font: { family: "Arial, sans-serif", weight: 600, style: "normal", generation: 0 } },
      (input, measure) => session.prepareSharedHeadHud(input, platform, measure));
    if (reply.kind === "unavailable") throw new Error(JSON.stringify(reply));
    return reply.kind === "absent" ? reply : { kind: "visible", ...reply.projection };
  });
}

export class HudCanvasFixture implements SurfaceDrawingContext {
  readonly locales: string[] = [];
  readonly painted: string[] = [];
  readonly clears: number[][] = [];
  readonly fills: number[][] = [];
  readonly fonts: string[] = [];
  invalidMetrics = false;
  private fontSize = 1;
  private readonly states: number[] = [];
  clearRect(...dimensions: [number, number, number, number]): void { this.clears.push(dimensions); }
  fillRect(...dimensions: [number, number, number, number]): void { this.fills.push(dimensions); }
  fillGradientRect(): void {}
  fillText(value: string): void { this.painted.push(value); }
  measureText(value: string): SurfaceTextMetrics {
    const width = Array.from(value).reduce((sum, character) => sum + this.fontSize * (character.charCodeAt(0) > 255 ? 1 : 0.65), 0);
    return { width, left: 0, right: width, ascent: this.invalidMetrics ? Number.NaN : this.fontSize * 0.7, descent: this.fontSize * 0.2 };
  }
  save(): void { this.states.push(this.fontSize); }
  restore(): void { this.fontSize = this.states.pop() ?? 1; }
  setFont(value: string): void { this.fonts.push(value); this.fontSize = Number(value.match(/([\d.]+)px/)?.[1] ?? 1); }
  setFillStyle(): void {}
  setLocale(locale: string): void { this.locales.push(locale); }
  setStrokeStyle(): void {}
  setTextBaseline(): void {}
  setTextAlign(): void {}
  translate(): void {}
  rotate(): void {}
  setGlobalAlpha(): void {}
  setLineWidth(): void {}
  strokeRect(): void {}
  beginPath(): void {}
  closePath(): void {}
  rect(): void {}
  clip(): void {}
  fill(): void {}
  moveTo(): void {}
  lineTo(): void {}
  stroke(): void {}
}

export function convexQuadsOverlap(first: readonly Readonly<{ x: number; y: number }>[], second: readonly Readonly<{ x: number; y: number }>[]): boolean {
  for (const polygon of [first, second]) for (const [index, point] of polygon.entries()) {
    const next = polygon[(index + 1) % polygon.length];
    if (next === undefined) throw new Error("Missing polygon corner");
    const normal = { x: next.y - point.y, y: point.x - next.x };
    const project = (corners: typeof first) => corners.map((corner) => normal.x * corner.x + normal.y * corner.y);
    const firstValues = project(first);
    const secondValues = project(second);
    if (Math.max(...firstValues) <= Math.min(...secondValues) || Math.max(...secondValues) <= Math.min(...firstValues)) return false;
  }
  return true;
}
