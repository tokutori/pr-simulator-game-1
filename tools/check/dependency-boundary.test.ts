import { describe, expect, it } from "vitest";
import { assertDependencyBoundary, assertNativeTextPatch, HOST_NUMERICAL_TEST_TARGET, NATIVE_SCREEN_TARGET } from "./dependency-boundary.js";

const numericalDependency = {
  name: "nalgebra", kind: "dev", target: HOST_NUMERICAL_TEST_TARGET, req: "=0.35.0",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false, uses_default_features: false, features: ["std"]
};

const nativeEngineDependency = {
  name: "bevy", kind: null, target: NATIVE_SCREEN_TARGET, req: "=0.19.1", uses_default_features: false,
  features: ["std", "bevy_pbr", "bevy_winit", "bevy_ui_render"]
};

const nativeTextDependency = {
  name: "parley", kind: null, target: NATIVE_SCREEN_TARGET, req: "=0.9.0",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false, uses_default_features: false, features: ["std", "complex-scripts"]
};

function core(dependency: unknown): unknown[] {
  return [{ name: "birdman-game-core", dependencies: [dependency] }];
}

describe("Cargo dependency boundary", () => {
  it("limits the native text patch to its excluded vendor directory", () => {
    expect(() => { assertNativeTextPatch({
      workspace: { exclude: ["vendor/parley"] },
      patch: { "crates-io": { parley: { path: "vendor/parley" } } }
    }); }).not.toThrow();
  });

  it.each([
    { path: "../parley" }, { git: "https://example.invalid/parley" },
    { path: "vendor/parley", git: "https://example.invalid/parley" }
  ])("rejects a different native text replacement %j", (replacement) => {
    expect(() => { assertNativeTextPatch({
      workspace: { exclude: ["vendor/parley"] },
      patch: { "crates-io": { parley: replacement } }
    }); }).toThrow("Invalid native text segmentation backport");
  });

  it("keeps the native text dependency outside workspace members", () => {
    expect(() => { assertNativeTextPatch({
      workspace: { exclude: [] },
      patch: { "crates-io": { parley: { path: "vendor/parley" } } }
    }); }).toThrow("Invalid native text segmentation backport");
  });

  it("preserves the existing product dependency whitelist", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-core", dependencies: [{ name: "libm", kind: null, target: null }] },
      { name: "birdman-game-format", dependencies: [{ name: "birdman-game-core" }, { name: "serde" }] },
      { name: "birdman-game-cli", dependencies: [{ name: "serde_json" }] },
      { name: "birdman-game-wasm", dependencies: [{ name: "wasm-bindgen" }] }
    ]); }).not.toThrow();
  });

  it("allows only the pinned host numerical dev dependency", () => {
    expect(() => { assertDependencyBoundary(core(numericalDependency)); }).not.toThrow();
  });

  it("keeps native engine and shared session dependencies outside core and format", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-session", dependencies: [{ name: "birdman-game-core" }, { name: "birdman-game-format" }] },
      { name: "birdman-game-wasm", dependencies: [{ name: "birdman-game-session" }] },
      { name: "birdman-game-bevy", dependencies: [nativeEngineDependency, { name: "birdman-game-session" }] }
    ]); }).not.toThrow();
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-session", "birdman-game-wasm"])(
    "rejects the native engine in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [nativeEngineDependency] }]); }).toThrow();
    }
  );

  it("allows the pinned Windows native complex-script segmentation backport", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [nativeTextDependency] }
    ]); }).not.toThrow();
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-session", "birdman-game-wasm"])(
    "rejects the native text engine in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [nativeTextDependency] }]); }).toThrow();
    }
  );

  it.each([
    { req: "^0.9.0" }, { kind: "dev" }, { kind: "build" }, { target: null },
    { target: 'cfg(target_arch = "wasm32")' }, { uses_default_features: true },
    { features: ["std"] }, { features: ["complex-scripts"] },
    { features: ["std", "complex-scripts", "system"] }, { optional: true },
    { source: "git+https://example.invalid/parley" }, { path: "../parley" }
  ])("rejects a broader native text dependency %j", (difference) => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [{ ...nativeTextDependency, ...difference }] }
    ]); }).toThrow("Forbidden dependency");
  });

  it.each([
    { req: "^0.19.1" }, { kind: "build" }, { kind: "dev" }, { target: null },
    { target: 'cfg(target_arch = "wasm32")' }, { uses_default_features: true },
    { features: ["3d"] }, { features: ["ui"] }, { features: ["default_platform"] },
    { features: ["audio"] }, { features: ["bevy_gilrs"] }, { features: ["web"] },
    { features: ["webgl2"] }, { features: ["webgpu"] }
  ])("rejects a broader native engine dependency %j", (difference) => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [{ ...nativeEngineDependency, ...difference }] }
    ]); }).toThrow("Forbidden native engine dependency");
  });

  it.each([
    { kind: null }, { kind: "build" }, { kind: "unknown" }, { target: null },
    { target: 'cfg(target_arch = "wasm32")' }, { target: "wasm32-unknown-unknown" },
    { target: 'cfg(not(target_arch = "wasm32"))' }, { req: "^0.35.0" },
    { source: "git+https://example.invalid/nalgebra" }, { registry: "other" },
    { path: "../nalgebra" }, { rename: "other" }, { optional: true },
    { uses_default_features: true }, { features: [] }, { features: ["std", "macros"] },
    { name: "num-complex" }
  ])("rejects a broader or different numerical dependency %j", (difference) => {
    expect(() => { assertDependencyBoundary(core({ ...numericalDependency, ...difference })); }).toThrow("Forbidden dependency");
  });

  it.each(["birdman-game-format", "birdman-game-cli", "birdman-game-wasm", "unexpected-crate"])(
    "rejects nalgebra in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [numericalDependency] }]); }).toThrow();
    }
  );

  it("rejects unexpected crates even without dependencies", () => {
    expect(() => { assertDependencyBoundary([{ name: "unexpected-crate", dependencies: [] }]); }).toThrow("Unexpected crate");
  });
});
