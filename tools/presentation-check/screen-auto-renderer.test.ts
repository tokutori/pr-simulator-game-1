import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { initSync, ScreenRenderAutoBridge } from "../../web/pkg/birdman_game_wasm.js";
import { parseScreenRenderAutoSnapshot } from "../../web/src/render/contracts/screen-render-auto.js";
import { ScreenAutoRenderer } from "../../web/src/render/engines/three/screen-auto-renderer.js";
import { gpuTimerFixture } from "./gpu-timer-fixture.js";
import type { LakeWaterQuality } from "../../web/src/render/contracts/lake-water.js";
import type { GameScene } from "../../web/src/render/contracts/ui.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function fixture() {
  const gpu = gpuTimerFixture();
  const policy = new ScreenRenderAutoBridge();
  let now = 0;
  let scene: GameScene = "Flight";
  const applied = vi.fn<(factor: number, shadowMapSize: number, quality: LakeWaterQuality, screen: boolean) => false | { readonly cleanupFailure?: string }>(() => ({}));
  const snapshot = vi.fn();
  const renderer = new ScreenAutoRenderer(gpu.context, { policy, scene: () => scene, onSnapshot: snapshot }, applied, () => now++);
  return { ...gpu, policy, renderer, applied, snapshot, time: (value: number) => { now = value; },
    scene: (value: GameScene) => { scene = value; },
    state: () => parseScreenRenderAutoSnapshot(policy.snapshot_json()) };
}

describe("Screen renderer with the production Rust GPU policy", () => {
  it("keeps monotonic operation time across eligibility, interruption and disjoint recovery", () => {
    const trial = fixture();
    try {
      trial.renderer.selectAuto(true);
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(true);
      trial.time(1000);
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(true);
      trial.gl.getParameter.mockReturnValue(true);
      trial.renderer.boundary(true);
      expect(trial.state().status.kind).not.toBe("unsupported");
      trial.renderer.boundary(false);
      trial.renderer.boundary(true);
      expect(trial.state().status.kind).not.toBe("unsupported");
    } finally { trial.renderer.dispose(); }
  });

  it("applies a measured degradation, restores Manual and reports unsupported after reselection", () => {
    const trial = fixture();
    try {
      trial.renderer.selectAuto(true);
      for (let frame = 0; frame < 140; frame++) {
        trial.time(frame * 20 + 10);
        trial.renderer.boundary(true);
        trial.renderer.begin(); trial.renderer.end(true);
      }
      expect(trial.applied).toHaveBeenCalledWith(0.75, 2048, "high", true);
      expect(trial.state()).toMatchObject({ selection: { kind: "auto", manual_scale: "100" }, applied: "75" });
      trial.renderer.selectAuto(false);
      trial.renderer.boundary(true);
      expect(trial.state()).toMatchObject({ selection: { kind: "manual", scale: "100" }, applied: "100" });
      expect(trial.gl.deleteQuery).toHaveBeenCalled();
      trial.renderer.restored();
      trial.gl.getExtension.mockReturnValue(null);
      for (let repetition = 0; repetition < 2; repetition++) {
        trial.renderer.selectAuto(true);
        trial.renderer.boundary(true); trial.renderer.begin(); trial.renderer.end(true);
        trial.renderer.boundary(true); trial.renderer.begin(); trial.renderer.end(true);
        expect(trial.state().status.kind).toBe("unsupported");
        trial.renderer.selectAuto(false);
      }
    } finally { trial.renderer.dispose(); }
  });

  it("holds the applied scale until drawable resources acknowledge and preserves failure cause", () => {
    const trial = fixture();
    try {
      trial.applied.mockReturnValue(false);
      trial.renderer.selectManual(0.5);
      trial.renderer.boundary(true);
      expect(trial.state()).toMatchObject({ applied: "100", status: { kind: "applying", requested: "50" } });
      trial.applied.mockReturnValue({});
      trial.renderer.boundary(true);
      expect(trial.state()).toMatchObject({ applied: "50", status: { kind: "manual" } });
      trial.renderer.selectManual(0.75);
      trial.applied.mockImplementationOnce(() => { throw new Error("reflection allocation failed"); });
      trial.renderer.boundary(true);
      expect(trial.state()).toMatchObject({ applied: "50", status: { kind: "failed", cause: "reflection allocation failed" } });
    } finally { trial.renderer.dispose(); }
  });

  it("performs no GPU API work while Manual is selected", () => {
    const trial = fixture();
    try {
      trial.renderer.selectManual(1);
      trial.renderer.boundary(true); trial.renderer.begin(); trial.renderer.end(true);
      expect(trial.gl.getExtension).not.toHaveBeenCalled();
      expect(trial.gl.getQueryParameter).not.toHaveBeenCalled();
      expect(trial.gl.beginQuery).not.toHaveBeenCalled();
    } finally { trial.renderer.dispose(); }
  });

  it("defaults to Auto and restores retained Manual after Title background adaptation", () => {
    const trial = fixture();
    try {
      expect(trial.state().selection.kind).toBe("auto");
      trial.renderer.selectManual(0.75);
      trial.renderer.boundary(true);
      const retained = trial.state().selection;
      trial.scene("Title");
      for (let frame = 0; frame < 140; frame++) {
        trial.time(frame * 20 + 10);
        trial.renderer.boundary(true);
        trial.renderer.begin(); trial.renderer.end(true);
      }
      expect(trial.state()).toMatchObject({ selection: retained, applied: "50" });
      trial.scene("Flight");
      trial.renderer.boundary(true);
      expect(trial.state()).toMatchObject({ selection: retained, applied: "75", status: { kind: "manual" } });
    } finally { trial.renderer.dispose(); }
  });

  it("reduces water only after minimum resolution and shadow, and restores the Manual triple", () => {
    const trial = fixture();
    try {
      trial.renderer.selectAuto(true);
      for (let frame = 0; frame < 1200; frame++) {
        trial.time(frame * 20 + 10);
        trial.renderer.boundary(true);
        trial.renderer.begin(); trial.renderer.end(true);
      }
      expect(trial.applied.mock.calls).toEqual([[0.75, 2048, "high", true], [0.5, 2048, "high", true],
        [0.5, 1024, "high", true], [0.5, 512, "high", true], [0.5, 512, "medium", true], [0.5, 512, "low", true]]);
      expect(trial.state()).toMatchObject({ applied: "50", shadow: "quarter", water_quality: "low" });
      trial.renderer.selectAuto(false);
      trial.renderer.boundary(true);
      expect(trial.applied).toHaveBeenLastCalledWith(1, 2048, "high", true);
      expect(trial.state()).toMatchObject({ selection: { kind: "manual", shadow: "full", water_quality: "high" }, applied: "100", shadow: "full", water_quality: "high" });
    } finally { trial.renderer.dispose(); }
  });

  it("retains the new applied resources and original retired-map cleanup failure", () => {
    const trial = fixture();
    try {
      trial.applied.mockReturnValue({ cleanupFailure: "retired shadow texture cleanup failed" });
      trial.renderer.selectManual(0.5);
      trial.renderer.boundary(true);
      expect(trial.state()).toMatchObject({ applied: "50", shadow: "full", status: {
        kind: "failed", cause: "retired shadow texture cleanup failed"
      } });
    } finally { trial.renderer.dispose(); }
  });

  it("starts queries only after a complete frame and repeats preparation after interruption", () => {
    const trial = fixture();
    const invalidation = vi.spyOn(trial.policy, "invalidate");
    try {
      trial.renderer.selectAuto(true);
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(false);
      expect(trial.gl.beginQuery).not.toHaveBeenCalled();
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(true);
      expect(trial.gl.beginQuery).not.toHaveBeenCalled();
      expect(invalidation).toHaveBeenLastCalledWith("scene_preparation", expect.any(Number));
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(true);
      expect(trial.gl.beginQuery).toHaveBeenCalledTimes(1);
      const previous = trial.policy.epoch();
      trial.renderer.invalidate("quality_changed");
      expect(trial.policy.epoch()).toBeGreaterThan(previous);
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(true);
      expect(trial.gl.beginQuery).toHaveBeenCalledTimes(1);
      trial.renderer.boundary(true);
      trial.renderer.begin(); trial.renderer.end(true);
      expect(trial.gl.beginQuery).toHaveBeenCalledTimes(2);
    } finally { trial.renderer.dispose(); }
  });
});
