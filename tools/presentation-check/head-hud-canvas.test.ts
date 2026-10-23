import { describe, expect, it } from "vitest";
import { withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { prepareHeadHudPresentation, drawHeadHud } from "../../web/src/presentation/head-hud-canvas.js";
import { DEFAULT_XR_HEAD_HUD_PROFILE } from "../../web/src/generated/xr-head-hud-profile.js";
import { configuredViewerFixture } from "./viewer-fixture.js";
import { HudCanvasFixture } from "./hud-canvas-fixture.js";
import type { GameUiFrame } from "../../web/src/generated/game-ui-frame.js";

describe("Retained Head HUD surface", () => {
  it.each([["title", 1280, 720], ["flight_running", 1280, 720], ["flight_running", 720, 1280]] as const)("prepares and draws the current %s owner frame at %sx%s with cached platform font observations", (phase, width, height) => {
    withSharedGameUiSession(phase, { informationCode: 0, platform: { presentation_mode: "hmd" } }, (session, platform, bridge) => {
      const view = session.readSharedUi(platform);
      const context = new HudCanvasFixture();
      const observeText = context.measureText.bind(context);
      context.measureText = (value: string) => {
        const metric = observeText(value);
        return value.startsWith("FPS") ? { ...metric, descent: 0 } : metric;
      };
      const input = { viewRevision: view.view_revision, viewer: configuredViewerFixture(width, height),
        locale: "ja", profile: DEFAULT_XR_HEAD_HUD_PROFILE, frameRateLabel: "FPS 72.0",
        font: { family: "Arial, sans-serif", weight: 600, style: "normal" as const, generation: 0 } };
      const prepare = () => prepareHeadHudPresentation(context, input, (request, measure) => session.prepareSharedHeadHud(request, platform, measure));
      const reply = prepare();
      expect(reply.kind).toBe("ready");
      if (reply.kind !== "ready") throw new Error(JSON.stringify(reply));
      const measured = context.fonts.length;
      expect(prepare()).toEqual(reply);
      expect(context.fonts).toHaveLength(measured);
      const [canvasWidth, canvasHeight] = reply.projection.canvasSize;
      drawHeadHud(context, reply, canvasWidth, canvasHeight);
      expect(context.painted).toContain("FPS 72.0");
      expect(context.painted.length > 1).toBe(phase === "flight_running");
      expect(context.clears).toEqual([[0, 0, canvasWidth, canvasHeight]]);
      expect(context.locales.every(locale => locale === "ja")).toBe(true);
      const clips = reply.projection.items.filter(item => item.kind === "plot").map(item => item.clip);
      const reservation = reply.projection.clearRegion;
      const clear = { x: reservation.x * reply.projection.size.width, y: reservation.y * reply.projection.size.height,
        width: reservation.width * reply.projection.size.width, height: reservation.height * reply.projection.size.height };
      const overlaps = (first: typeof clear, second: typeof clear) => Math.min(first.x + first.width, second.x + second.width) - Math.max(first.x, second.x) > 1e-10
        && Math.min(first.y + first.height, second.y + second.height) - Math.max(first.y, second.y) > 1e-10;
      for (const [index, clip] of clips.entries()) {
        expect(overlaps(clip, clear)).toBe(false);
        for (const other of clips.slice(index + 1)) expect(overlaps(clip, other)).toBe(false);
      }
      if (phase === "flight_running") {
        const frame = JSON.parse(bridge.game_ui_frame_json(view.view_revision, JSON.stringify(platform))) as GameUiFrame;
        const nodes = frame.hud?.groups.flatMap(group => group.nodes) ?? [];
        const index = nodes.findIndex(node => node.content.kind === "heading_tape");
        const node = nodes[index];
        const plot = reply.projection.items[index + 1];
        if (node?.content.kind !== "heading_tape" || plot?.kind !== "plot") throw new Error("Missing heading projection");
        const texts = plot.items.filter(item => item.kind === "text");
        const font = texts[0]?.fontMeters;
        if (font === undefined) throw new Error("Missing measured font");
        const padding = font * 0.5;
        const textRows = texts.length - node.content.ticks.filter(tick => tick.label !== "").length;
        const top = plot.clip.y + padding + textRows * font * 1.5;
        const height = node.instrument_height_em * font;
        const lines = plot.items.filter(item => item.kind === "line");
        expect(lines).toHaveLength(node.content.ticks.length + 1);
        for (const [tickIndex, tick] of node.content.ticks.entries()) {
          const line = lines[tickIndex];
          const horizontal = plot.clip.x + padding + tick.position_ratio * (plot.clip.width - padding * 2);
          expect(line?.start.x).toBeCloseTo(horizontal, 12);
          expect(line?.end.x).toBeCloseTo(horizontal, 12);
          expect(line?.start.y).toBeCloseTo(top + height * (tick.major ? 0.5 : 0.75), 12);
          expect(line?.end.y).toBeCloseTo(top + height, 12);
          expect(line?.color).toBe(tick.major ? view.theme.text : view.theme.muted_text);
          expect(line?.opacity).toBe(input.profile.foregroundAlpha);
          expect(horizontal).toBeGreaterThanOrEqual(plot.clip.x);
          expect(horizontal).toBeLessThanOrEqual(plot.clip.x + plot.clip.width);
        }
      }
    });
  });

  it("projects the flight-path-only marker from the shared ratio into its clipped instrument surface", () => {
    withSharedGameUiSession("flight_running", { informationCode: 4, hudProfile: {
      telemetry: false, attitude: false, wind: false, flight_path: true, angle_of_attack: false, warnings: false
    }, platform: { presentation_mode: "hmd" } }, (session, platform) => {
      const view = session.readSharedUi(platform);
      const reply = prepareHeadHudPresentation(new HudCanvasFixture(), { viewRevision: view.view_revision,
        viewer: configuredViewerFixture(1280, 720), locale: "ja", profile: DEFAULT_XR_HEAD_HUD_PROFILE,
        frameRateLabel: null, font: { family: "Arial", weight: 600, style: "normal", generation: 0 } },
        (request, measure) => session.prepareSharedHeadHud(request, platform, measure));
      if (reply.kind !== "ready") throw new Error(JSON.stringify(reply));
      const node = view.hud?.groups.flatMap(group => group.nodes)[0];
      const plot = reply.projection.items[0];
      if (node?.content.kind !== "attitude" || node.content.geometry.kind !== "flight_path_only" || plot?.kind !== "plot") throw new Error("Missing path-only surface");
      expect(reply.projection.items).toHaveLength(1);
      const texts = plot.items.filter(item => item.kind === "text");
      const font = texts[0]?.fontMeters;
      if (font === undefined) throw new Error("Missing marker font scale");
      const center = plot.clip.x + plot.clip.width / 2;
      const baseline = plot.clip.y + font * 0.5 + texts.length * font * 1.5
        + node.content.geometry.y_ratio * node.instrument_height_em * font;
      const radius = font * 0.35;
      const lines = plot.items.filter(item => item.kind === "line");
      expect(lines).toHaveLength(2);
      for (const [index, line] of lines.entries()) {
        expect(line.start.x).toBeCloseTo(center + (index === 0 ? -radius : 0), 12);
        expect(line.start.y).toBeCloseTo(baseline + (index === 0 ? 0 : -radius), 12);
        expect(line.end.x).toBeCloseTo(center + (index === 0 ? 0 : radius), 12);
        expect(line.end.y).toBeCloseTo(baseline + (index === 0 ? -radius : 0), 12);
        expect(line.color).toBe("#7df4c5");
      }
    });
  });

  it.each(["nonfinite", "tiny"] as const)("rejects %s live font ink through the owner boundary", (kind) => {
    withSharedGameUiSession("flight_running", { platform: { presentation_mode: "hmd" } }, (session, platform) => {
      const view = session.readSharedUi(platform);
      const context = new HudCanvasFixture();
      context.invalidMetrics = kind === "nonfinite";
      const measureText = context.measureText.bind(context);
      context.measureText = (value: string) => {
        const measured = measureText(value);
        return kind === "tiny" ? { ...measured, ascent: measured.ascent / 7, descent: measured.descent / 4 } : measured;
      };
      const reply = prepareHeadHudPresentation(context, { viewRevision: view.view_revision,
        viewer: configuredViewerFixture(1280, 720), locale: "ja", profile: DEFAULT_XR_HEAD_HUD_PROFILE,
        frameRateLabel: null, font: { family: "Arial", weight: 600, style: "normal", generation: 0 } },
        (request, measure) => session.prepareSharedHeadHud(request, platform, measure));
      expect(reply).toEqual({ kind: "unavailable", reason: kind === "tiny" ? "insufficient-ink-angle" : "invalid-text-metrics" });
      drawHeadHud(context, reply, 1024, 512);
      expect(context.painted).toEqual([]);
      expect(context.clears).toEqual([[0, 0, 1024, 512]]);
    });
  });
});
