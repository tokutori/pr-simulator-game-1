import { describe, expect, it } from "vitest";
import { assertRenderImportBoundary } from "./render-boundary.js";

describe("engine import boundary", () => {
  it("allows Three.js imports inside its adapter and a factory import from the composition root", () => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/render/engines/three/three-renderer.ts", text: "import * as THREE from 'three';" },
      { path: "web/src/main.ts", text: "import { createThreeRenderer } from './render/engines/three/three-renderer.js';" }
    ]); }).not.toThrow();
  });

  it.each([
    ["type import", "import type { WebGLRenderer } from 'three';"],
    ["dynamic import", "const engine = await import('three/webgpu');"],
    ["re-export", "export { Scene } from 'three';"],
    ["variable specifier", "const engine = await import(packageName);"]
  ])("rejects %s outside the adapter", (_kind, text) => {
    expect(() => { assertRenderImportBoundary([{ path: "web/src/presentation/app.ts", text }]); }).toThrow();
  });

  it("allows only the composition root to import the concrete adapter", () => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/presentation/runtime.ts", text: "import { createThreeRenderer } from '../render/engines/three/three-renderer.js';" }
    ]); }).toThrow("composition root");
  });
});
