import { describe, expect, it } from "vitest";
import { assertDependencyBoundary, assertNativeTextPatch, HOST_NUMERICAL_TEST_TARGET, NATIVE_SCREEN_TARGET } from "./dependency-boundary.js";

const numericalDependency = {
  name: "nalgebra", kind: "dev", target: HOST_NUMERICAL_TEST_TARGET, req: "=0.35.0",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false, uses_default_features: false, features: ["std"]
};

const nativeEngineDependency = {
  name: "bevy", kind: null, target: NATIVE_SCREEN_TARGET, req: "=0.19.1", uses_default_features: false,
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false,
  features: ["std", "bevy_pbr", "bevy_winit", "bevy_ui_render"]
};

const nativeTextDependency = {
  name: "parley", kind: null, target: NATIVE_SCREEN_TARGET, req: "=0.9.0",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false, uses_default_features: false, features: ["std", "complex-scripts"]
};

const nativeTestFontDependency = {
  ...nativeEngineDependency, kind: "dev", features: ["default_font"]
};

const schemaDependency = {
  name: "schemars", kind: null, target: null, req: "=1.2.2",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: true, uses_default_features: false, features: ["derive"]
};

const chartNumberDependency = {
  name: "ryu-js", kind: null, target: null, req: "=1.0.3",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false, uses_default_features: false, features: []
};

function schemaProvider(
  name: "birdman-game-format" | "birdman-game-session",
  dependency: unknown = schemaDependency,
  features: unknown = { schema: name === "birdman-game-format"
    ? ["dep:schemars"] : ["dep:schemars", "birdman-game-format/schema"] }
): unknown[] {
  return [{ name, dependencies: [dependency], features }];
}

function core(dependency: unknown): unknown[] {
  return [{ name: "birdman-game-core", dependencies: [dependency] }];
}

function versionedPackages(difference: Readonly<Record<string, unknown>>): unknown[] {
  return [
    ...core({ ...numericalDependency, ...difference }),
    { name: "birdman-game-session", dependencies: [{ ...chartNumberDependency, ...difference }] },
    { name: "birdman-game-bevy", dependencies: [{ ...nativeEngineDependency, ...difference }] },
    { name: "birdman-game-bevy", dependencies: [{ ...nativeTestFontDependency, ...difference }] },
    ...schemaProvider("birdman-game-format", { ...schemaDependency, ...difference }),
    ...schemaProvider("birdman-game-session", { ...schemaDependency, ...difference })
  ];
}

describe("Cargo dependency boundary", () => {
  it.each(["=9.8.7", "=9.8.7-rc.1"])("accepts a different complete manifest pin %s without changing the design boundary", (req) => {
    expect(() => { assertDependencyBoundary(versionedPackages({ req })); }).not.toThrow();
  });

  it.each(["^9.8.7", "~9.8.7", "9.8.7", "=9.8", "=9", "*", "=9.8.7, =9.8.8"])(
    "rejects an incomplete or non-exact pin %s", (req) => {
      for (const pkg of versionedPackages({ req })) expect(() => { assertDependencyBoundary([pkg]); }).toThrow();
    }
  );

  it.each(["registry+https://index.crates.io/opaque-source-id", "sparse+https://index.crates.io/"])(
    "accepts the documented default-registry source kind without fixing its opaque ID (%s)", (source) => {
      expect(() => { assertDependencyBoundary([
        ...versionedPackages({ source }),
        { name: "birdman-game-bevy", dependencies: [{ ...nativeTextDependency, source }] }
      ]); }).not.toThrow();
    }
  );

  it.each([{ source: "git+https://example.invalid/dependency" }, { source: "local+unknown" }, { source: "registry+" },
    { source: null }, { registry: "other" }, { path: "../dependency" }, { rename: "other" }])(
    "rejects a non-default, substituted or unknown source %j", (difference) => {
      for (const pkg of [...versionedPackages(difference),
        { name: "birdman-game-bevy", dependencies: [{ ...nativeTextDependency, ...difference }] }]) {
        expect(() => { assertDependencyBoundary([pkg]); }).toThrow();
      }
    }
  );

  it("rejects a WASM ABI dependency in no_std core", () => {
    expect(() => { assertDependencyBoundary(core({ name: "wasm-bindgen" })); }).toThrow("Forbidden dependency");
  });

  it("allows the fixed ECMAScript number formatter only in shared session", () => {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-session", dependencies: [chartNumberDependency] }]); }).not.toThrow();
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"])(
    "rejects a direct chart-number dependency in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [chartNumberDependency] }]); }).toThrow("Forbidden dependency");
    }
  );

  it.each([{ req: "^1.0.3" }, { kind: "dev" }, { kind: "build" },
    { target: 'cfg(target_arch = "wasm32")' }, { features: ["no-panic"] }, { uses_default_features: true },
    { optional: true }, { rename: "formatter" }, { source: "git+https://example.invalid/ryu-js" }, { path: "../ryu-js" }])(
    "rejects broader chart-number dependencies %j", (difference) => {
      expect(() => { assertDependencyBoundary([{ name: "birdman-game-session", dependencies: [{ ...chartNumberDependency, ...difference }] }]); }).toThrow("Forbidden dependency");
    }
  );
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

  it.each(["birdman-game-format", "birdman-game-session"] as const)(
    "allows only the optional pinned schema generator in %s", (name) => {
      expect(() => { assertDependencyBoundary(schemaProvider(name)); }).not.toThrow();
    }
  );

  it.each([
    { req: "^1.2.2" }, { kind: "dev" }, { kind: "build" },
    { target: HOST_NUMERICAL_TEST_TARGET }, { optional: false }, { uses_default_features: true },
    { source: "git+https://example.invalid/schemars" }, { registry: "other" },
    { path: "../schemars" }, { rename: "other" }, { features: [] }, { features: ["std"] },
    { features: ["derive", "std"] }, { name: "other-generator" }
  ])("rejects a broader schema generator %j", (difference) => {
    for (const name of ["birdman-game-format", "birdman-game-session"] as const) {
      expect(() => { assertDependencyBoundary(schemaProvider(name, { ...schemaDependency, ...difference })); })
        .toThrow("Forbidden dependency");
    }
  });

  it.each([
    {}, { schema: [] }, { schema: ["schemars"] }, { schema: ["dep:schemars", "dep:schemars"] },
    { schema: ["dep:schemars"], default: ["schema"] },
    { schema: ["dep:schemars", "birdman-game-format/schema"], default: ["schema"] },
    { schema: ["dep:schemars"], runtime: ["schema"] },
    { schema: ["dep:schemars", "birdman-game-format/schema"], runtime: ["schema"] }
  ])("rejects broader schema activation wiring %j", (features) => {
    for (const name of ["birdman-game-format", "birdman-game-session"] as const) {
      expect(() => { assertDependencyBoundary(schemaProvider(name, schemaDependency, features)); })
        .toThrow("Forbidden dependency");
    }
  });

  it.each(["birdman-game-core", "birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"])(
    "rejects a direct schema generator in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [schemaDependency] }]); })
        .toThrow("Forbidden dependency");
    }
  );

  it("allows schema activation only through the CLI exporter feature", () => {
    expect(() => { assertDependencyBoundary([{
      name: "birdman-game-cli", dependencies: [{ name: "birdman-game-session", features: [] }],
      features: { schema: ["birdman-game-session/schema"] }
    }]); }).not.toThrow();
  });

  it("allows unrelated features without duplicating the entire manifest feature table", () => {
    for (const name of ["birdman-game-format", "birdman-game-session"] as const) {
      expect(() => { assertDependencyBoundary(schemaProvider(name, schemaDependency, {
        schema: name === "birdman-game-format" ? ["dep:schemars"] : ["dep:schemars", "birdman-game-format/schema"],
        default: [], audit: []
      })); }).not.toThrow();
    }
    for (const name of ["birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"]) {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [], features: {
        ...(name === "birdman-game-cli" ? { schema: ["birdman-game-session/schema"] } : {}), default: [], audit: []
      } }]); }).not.toThrow();
    }
  });

  it.each(["schema", "schemars", "dep:schemars", "schemars/derive", "schemars?/derive",
    "birdman-game-format/schema", "birdman-game-format?/schema", "birdman-game-session/schema"])(
    "rejects schema generation through another feature (%s)", (feature) => {
      for (const name of ["birdman-game-format", "birdman-game-session"] as const) {
        expect(() => { assertDependencyBoundary(schemaProvider(name, schemaDependency, {
          schema: name === "birdman-game-format" ? ["dep:schemars"] : ["dep:schemars", "birdman-game-format/schema"],
          audit: [feature]
        })); }).toThrow("Forbidden dependency");
      }
      expect(() => { assertDependencyBoundary([{ name: "birdman-game-cli", dependencies: [], features: {
        schema: ["birdman-game-session/schema"], audit: [feature]
      } }]); }).toThrow("Forbidden schema feature boundary");
      for (const name of ["birdman-game-wasm", "birdman-game-bevy"]) {
        expect(() => { assertDependencyBoundary([{ name, dependencies: [], features: { audit: [feature] } }]); })
          .toThrow("Forbidden runtime schema feature");
      }
    }
  );

  it.each([
    { schema: [] }, { schema: ["birdman-game-format/schema"] },
    { schema: ["birdman-game-session/schema"], default: ["schema"] },
    { schema: ["birdman-game-session/schema"], runtime: ["schema"] }
  ])("rejects broader CLI schema activation %j", (features) => {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-cli", dependencies: [], features }]); })
      .toThrow("Forbidden schema feature boundary");
  });

  it.each([
    { schema: [] }, { default: ["schema"] }, { runtime: ["birdman-game-session/schema"] },
    { default: ["birdman-game-format/schema"] }, { runtime: ["birdman-game-session?/schema"] }
  ])("rejects runtime schema feature forwarding %j", (features) => {
    for (const name of ["birdman-game-wasm", "birdman-game-bevy"]) {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [], features }]); })
        .toThrow("Forbidden runtime schema feature");
    }
  });

  it.each(["birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"])(
    "rejects schema requested by normal shared dependencies in %s", (name) => {
      for (const dependency of ["birdman-game-format", "birdman-game-session"]) {
        expect(() => { assertDependencyBoundary([{
          name, dependencies: [{ name: dependency, kind: null, features: ["schema"] }]
        }]); }).toThrow("Forbidden normal schema activation");
      }
    }
  );

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
    { req: "^0.9.0" }, { req: "=0.9.1" }, { name: "other-text-engine" }, { kind: "dev" }, { kind: "build" }, { target: null },
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
    { features: ["3d"] }, { features: ["ui"] }, { features: ["default_platform"] }, { features: ["default_font"] },
    { features: ["audio"] }, { features: ["bevy_gilrs"] }, { features: ["web"] },
    { features: ["webgl2"] }, { features: ["webgpu"] }
  ])("rejects a broader native engine dependency %j", (difference) => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [{ ...nativeEngineDependency, ...difference }] }
    ]); }).toThrow("Forbidden native engine dependency");
  });

  it("allows only the pinned Windows test font beside the product native engine", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [nativeEngineDependency, nativeTestFontDependency] }
    ]); }).not.toThrow();
  });

  it.each([
    { req: "^0.19.1" }, { kind: null }, { kind: "build" }, { target: null },
    { target: 'cfg(target_arch = "wasm32")' }, { uses_default_features: true },
    { features: [] }, { features: ["std"] }, { features: ["default_font", "std"] },
    { source: "git+https://example.invalid/bevy" }, { registry: "other" },
    { path: "../bevy" }, { rename: "other" }, { optional: true }
  ])("rejects a broader native test font dependency %j", (difference) => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [{ ...nativeTestFontDependency, ...difference }] }
    ]); }).toThrow("Forbidden native engine dependency");
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-session", "birdman-game-cli", "birdman-game-wasm"])(
    "rejects the native test font in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [nativeTestFontDependency] }]); }).toThrow();
    }
  );

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
