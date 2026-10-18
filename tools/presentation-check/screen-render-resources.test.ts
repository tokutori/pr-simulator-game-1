import { DepthTexture, DirectionalLight, WebGLRenderTarget } from "three";
import { describe, expect, it, vi } from "vitest";
import { applyScreenRenderResources } from "../../web/src/render/engines/three/screen-render-resources.js";

function fixture() {
  const sun = new DirectionalLight();
  const previous = new WebGLRenderTarget(2048, 2048);
  previous.depthTexture = new DepthTexture(2048, 2048);
  sun.shadow.map = previous;
  sun.shadow.mapSize.set(2048, 2048);
  const retired = vi.fn();
  previous.addEventListener("dispose", retired);
  return { sun, previous, retired };
}

describe("production Screen shadow and resolution resource exchange", () => {
  it("prepares an independent target before applying resolution and commits both before cleanup", () => {
    const trial = fixture();
    const prepare = vi.fn((next: WebGLRenderTarget) => {
      expect(next).not.toBe(trial.previous);
      expect(next.depthTexture).not.toBe(trial.previous.depthTexture);
      expect([next.width, next.height]).toEqual([1024, 1024]);
      expect(trial.sun.shadow.map).toBe(trial.previous);
    });
    const resolution = vi.fn(() => {
      expect(trial.retired).not.toHaveBeenCalled();
      expect(trial.sun.shadow.map).toBe(trial.previous);
      return true;
    });
    expect(applyScreenRenderResources(trial.sun, { initRenderTarget: prepare }, 1024, resolution)).toEqual({});
    expect(prepare).toHaveBeenCalledOnce();
    expect(trial.sun.shadow.map?.width).toBe(1024);
    expect(trial.sun.shadow.mapSize.toArray()).toEqual([1024, 1024]);
    expect(trial.retired).toHaveBeenCalledOnce();
  });

  it("retains both old resources when shadow allocation fails before resolution application", () => {
    const trial = fixture();
    const abandoned = vi.fn();
    const resolution = vi.fn(() => true);
    const prepare = (next: WebGLRenderTarget): void => {
      next.addEventListener("dispose", abandoned);
      throw new Error("shadow framebuffer allocation failed");
    };
    expect(() => applyScreenRenderResources(trial.sun, { initRenderTarget: prepare }, 512, resolution))
      .toThrow("shadow framebuffer allocation failed");
    expect(resolution).not.toHaveBeenCalled();
    expect(trial.sun.shadow.map).toBe(trial.previous);
    expect(trial.sun.shadow.mapSize.toArray()).toEqual([2048, 2048]);
    expect(trial.retired).not.toHaveBeenCalled();
    expect(abandoned).toHaveBeenCalledOnce();
  });

  it.each([false, new Error("resolution allocation failed")])("retains shadow when resolution cannot commit: %s", (outcome) => {
    const trial = fixture();
    const abandoned = vi.fn();
    const prepare = (next: WebGLRenderTarget): void => { next.addEventListener("dispose", abandoned); };
    const apply = () => applyScreenRenderResources(trial.sun, { initRenderTarget: prepare }, 1024, () => {
      if (outcome instanceof Error) throw outcome;
      return outcome;
    });
    if (outcome instanceof Error) expect(apply).toThrow(outcome);
    else expect(apply()).toBe(false);
    expect(trial.sun.shadow.map).toBe(trial.previous);
    expect(trial.sun.shadow.mapSize.toArray()).toEqual([2048, 2048]);
    expect(trial.retired).not.toHaveBeenCalled();
    expect(abandoned).toHaveBeenCalledOnce();
  });

  it("attempts independent retired cleanup and reports failure after the new pair is committed", () => {
    const trial = fixture();
    const textureCleanup = vi.fn(() => { throw new Error("depth cleanup failed"); });
    trial.previous.depthTexture?.addEventListener("dispose", textureCleanup);
    trial.previous.addEventListener("dispose", () => {
      expect(trial.previous.depthTexture).toBeNull();
      throw new Error("map cleanup failed");
    });
    const result = applyScreenRenderResources(trial.sun, { initRenderTarget: vi.fn() }, 512, () => true);
    expect(result).toEqual({ cleanupFailure: "Shadow map cleanup failed: depth cleanup failed; map cleanup failed" });
    expect(trial.sun.shadow.map).not.toBe(trial.previous);
    expect(trial.sun.shadow.mapSize.toArray()).toEqual([512, 512]);
    expect(textureCleanup).toHaveBeenCalledOnce();
    expect(trial.retired).toHaveBeenCalledOnce();
  });

  it("keeps the existing target when its resolution is unchanged", () => {
    const trial = fixture();
    const prepare = vi.fn();
    expect(applyScreenRenderResources(trial.sun, { initRenderTarget: prepare }, 2048, () => true)).toEqual({});
    expect(prepare).not.toHaveBeenCalled();
    expect(trial.sun.shadow.map).toBe(trial.previous);
    expect(trial.retired).not.toHaveBeenCalled();
  });

  it("commits the prepared water after the shadow swap and attempts both retired cleanups", () => {
    const trial = fixture();
    let waterCommitted = false;
    trial.previous.addEventListener("dispose", () => {
      expect(waterCommitted).toBe(true);
      throw new Error("shadow cleanup failed");
    });
    const result = applyScreenRenderResources(trial.sun, { initRenderTarget: vi.fn() }, 512, () => true, () => {
      expect(trial.sun.shadow.map).not.toBe(trial.previous);
      expect(trial.sun.shadow.mapSize.toArray()).toEqual([512, 512]);
      waterCommitted = true;
      return "water cleanup failed";
    });
    expect(result).toEqual({ cleanupFailure: "water cleanup failed; Shadow map cleanup failed: shadow cleanup failed" });
    expect(trial.retired).toHaveBeenCalledOnce();
  });
});
