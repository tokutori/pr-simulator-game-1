import { resolve } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";
import { assertRenderImportBoundary } from "./render-boundary.js";

describe("engine import boundary", () => {
  it("accepts an acyclic graph with shared type dependencies", () => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/app/consumer.ts", text: "import type { Selection } from './selection.js'; import type { Snapshot } from './snapshot.js';" },
      { path: "web/src/app/selection.ts", text: "export interface Selection { readonly code: number; }" },
      { path: "web/src/app/snapshot.ts", text: "import type { Selection } from './selection.js'; export interface Snapshot { readonly selection: Selection; }" }
    ]); }).not.toThrow();
  });

  it.each([
    "import type { Snapshot } from './snapshot.js';",
    "export type { Snapshot } from './snapshot.js';",
    "type Snapshot = import('./snapshot.js').Snapshot;",
    "const snapshot = import('./snapshot.js');"
  ])("rejects a cycle including %s", (text) => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/app/selection.ts", text },
      { path: "web/src/app/snapshot.ts", text: "import type { Consumer } from './consumer.js'; export interface Snapshot { readonly consumer: Consumer; }" },
      { path: "web/src/app/consumer.ts", text: "import type { Selection } from './selection.js'; export interface Consumer { readonly selection: Selection; }" }
    ]); }).toThrow(/dependency cycle: .*selection\.ts -> .*snapshot\.ts -> .*consumer\.ts -> .*selection\.ts/);
  });

  it("allows Three.js imports inside its adapter and a factory import from the composition root", () => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/render/engines/three/three-renderer.ts", text: "import * as THREE from 'three';" },
      { path: "web/src/browser-application.ts", text: "import { createThreeRenderer } from './render/engines/three/three-renderer.js';" }
    ]); }).not.toThrow();
  });

  it.each([
    ["type import", "import type { WebGLRenderer } from 'three';"],
    ["dynamic import", "const engine = await import('three/webgpu');"],
    ["re-export", "export { Scene } from 'three';"],
    ["type query", "export type Vector = import('three').Vector3;"],
    ["typeof type query", "export type Engine = typeof import('three');"],
    ["type re-export", "export type { Vector3 } from 'three';"],
    ["star re-export", "export * from 'three';"],
    ["CommonJS require", "const engine = require('three');"],
    ["import assignment", "import engine = require('three');"],
    ["template literal", "const engine = await import(`three`);"],
    ["type reference", "/// <reference types='three' />"],
    ["relative package path", "import * as THREE from '../../../node_modules/three/build/three.module.js';"],
    ["variable specifier", "const engine = await import(packageName);"]
  ])("rejects %s outside the adapter", (_kind, text) => {
    expect(() => { assertRenderImportBoundary([{ path: "web/src/presentation/app.ts", text }]); }).toThrow();
  });

  it("allows only the composition root to import the concrete adapter", () => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/presentation/runtime.ts", text: "import { createThreeRenderer } from '../render/engines/three/three-renderer.js';" }
    ]); }).toThrow("composition root");
  });

  it.each([
    "import { createThreeRenderer } from '../engines/three/three-renderer.js';",
    "export type { ThreeRenderer } from '../engines/three/three-renderer.js';",
    "export * from '../engines/three/three-renderer.js';",
    "type Renderer = import('../engines/three/three-renderer.js').ThreeRenderer;",
    "const renderer = import('../engines/./three/../three/three-renderer.js');",
    "const renderer = require('../engines/three/three-renderer.js');",
    "import renderer = require('../engines/three/three-renderer.js');",
    "/// <reference path='../engines/three/three-renderer.ts' />"
  ])("rejects importer-relative adapter dependencies: %s", (text) => {
    expect(() => { assertRenderImportBoundary([{ path: "web/src/render/contracts/leak.ts", text }]); }).toThrow("composition root");
  });

  it.each([
    "web\\src\\render\\contracts\\leak.ts",
    resolve("web/src/render/contracts/leak.ts").replaceAll("/", "\\"),
    "web/src/render/engines/three/../../contracts/leak.ts"
  ])("normalizes source paths before classifying dependencies: %s", (path) => {
    expect(() => { assertRenderImportBoundary([{ path, text: "type Vector = import('three').Vector3;" }]); }).toThrow("adapter");
    expect(() => { assertRenderImportBoundary([{ path, text: "import '../engines/three/three-renderer.js';" }]); }).toThrow("composition root");
  });

  it("resolves configured aliases through the same compiler API", () => {
    const options: ts.CompilerOptions = {
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      module: ts.ModuleKind.ESNext,
      baseUrl: process.cwd(),
      paths: {
        "@adapter": ["web/src/render/engines/three/virtual.ts"],
        "@contract": ["web/src/render/contracts/virtual.ts"]
      }
    };
    const targets = [
      { path: "web/src/render/engines/three/virtual.ts", text: "export const value = 1;" },
      { path: "web/src/render/contracts/virtual.ts", text: "export const value = 1;" }
    ];
    expect(() => { assertRenderImportBoundary([
      ...targets,
      { path: "web/src/presentation/leak.ts", text: "import { value } from '@adapter';" }
    ], options); }).toThrow("composition root");
    expect(() => { assertRenderImportBoundary([
      ...targets,
      { path: "web/src/presentation/valid.ts", text: "import { value } from '@contract';" }
    ], options); }).not.toThrow();
  });

  it("allows normalized adapter-internal and composition-root paths", () => {
    expect(() => { assertRenderImportBoundary([
      { path: resolve("web/src/render/engines/three/three-renderer.ts").replaceAll("/", "\\"), text: "export type Vector = import('three').Vector3;" },
      { path: "web/src/render/engines/three/resource.ts", text: "import './three-renderer.js';" },
      { path: resolve("web/src/./browser-application.ts"), text: "const renderer = import('./render/contracts/../engines/three/three-renderer.js');" },
      { path: "web/src/presentation/valid.ts", text: "import '../render/engines/three/../../contracts/runtime.js';" },
      { path: "web/src/presentation/valid.ts", text: "import '../render/engines/three-compatible/runtime.js';" }
    ]); }).not.toThrow();
  });

  it.each([
    "/** @type {import('three').Vector3} */ let vector;",
    "/** @typedef {import('three').Vector3} Vector */ let vector;",
    "/** @import { Vector3 } from 'three' */ let vector;"
  ])("checks JavaScript type dependencies in JSDoc: %s", (text) => {
    expect(() => { assertRenderImportBoundary([{ path: "web/src/presentation/type-query.js", text }]); }).toThrow("adapter");
    expect(() => { assertRenderImportBoundary([{ path: "web/src/render/engines/three/type-query.js", text }]); }).not.toThrow();
  });

  it("does not interpret ordinary strings or explanatory comments as imports", () => {
    expect(() => { assertRenderImportBoundary([
      { path: "web/src/presentation/text.ts", text: "const example = `import('three')`; /* import('three') */" }
    ]); }).not.toThrow();
  });

  it.each([
    "const engine = require(packageName);",
    "const engine = import(`three/${name}`);",
    "const engine = import('three/' + name);"
  ])("rejects unresolvable dynamic expressions in every source: %s", (text) => {
    expect(() => { assertRenderImportBoundary([{ path: "web/src/render/engines/three/dynamic.ts", text }]); }).toThrow("Dynamic module specifier");
  });
});
