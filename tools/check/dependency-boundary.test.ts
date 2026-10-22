import { describe, expect, it } from "vitest";
import { assertDependencyBoundary, assertNativeTextPatch, HOST_NUMERICAL_TEST_TARGET, NATIVE_LINUX_WINDOW_TARGET, NATIVE_SCREEN_TARGET } from "./dependency-boundary.js";

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

const jsonNumberDependency = {
  name: "ryu-js", kind: null, target: null, req: "=1.0.3",
  source: "registry+https://github.com/rust-lang/crates.io-index", registry: null,
  rename: null, optional: false, uses_default_features: false, features: []
};

function schemaFeatures(name: "birdman-game-format" | "birdman-game-app" | "birdman-game-presentation"): readonly string[] {
  return name === "birdman-game-format" ? ["dep:schemars"]
    : name === "birdman-game-app" ? ["dep:schemars", "birdman-game-format/schema"]
    : ["dep:schemars", "birdman-game-app/schema", "birdman-game-format/schema"];
}

function schemaProvider(
  name: "birdman-game-format" | "birdman-game-app" | "birdman-game-presentation",
  dependency: unknown = schemaDependency,
  features: unknown = { schema: schemaFeatures(name) }
): unknown[] {
  return [{ name, dependencies: [dependency], features }];
}

function core(dependency: unknown): unknown[] {
  return [{ name: "birdman-game-core", dependencies: [dependency] }];
}

it("keeps the registered triangulator in the shared scene layer", () => {
  const earcut = { name: "earcut" };
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-scene", dependencies: [earcut] }]); }).not.toThrow();
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-presentation", dependencies: [earcut] }]); }).toThrow("Forbidden dependency");
  expect(() => { assertDependencyBoundary(core(earcut)); }).toThrow("Forbidden dependency");
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-wasm", dependencies: [earcut] }]); }).toThrow("Forbidden dependency");
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-bevy", dependencies: [earcut] }]); }).toThrow("Forbidden dependency");
});

describe("core proof dependency boundary", () => {
  const proof = { ...schemaDependency, name: "vstd", req: "=0.0.0-2026-10-04-0306", features: ["alloc"] };
  const features = { "verus-proof": ["dep:vstd"] };
  const provider = (dependency: unknown = proof, definitions: unknown = features): unknown[] => [
    { name: "birdman-game-core", dependencies: [dependency], features: definitions }
  ];
  it("allows an explicit alloc-only optional proof dependency", () => {
    expect(() => { assertDependencyBoundary(provider()); }).not.toThrow();
  });
  it.each([{ optional: false }, { uses_default_features: true }, { features: ["std"] }, { target: "cfg(verus_only)" }])(
    "rejects runtime or implicit proof dependencies %j", (difference) => {
      expect(() => { assertDependencyBoundary(provider({ ...proof, ...difference })); }).toThrow("Forbidden dependency");
    }
  );
  it.each([{ default: ["verus-proof"] }, { default: ["dep:vstd"] }, { runtime: ["vstd/std"] }])(
    "rejects indirect proof activation %j", (difference) => {
      expect(() => { assertDependencyBoundary(provider(proof, { ...features, ...difference })); }).toThrow("Forbidden dependency");
    }
  );
  it("rejects application activation of the proof feature", () => {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-format", dependencies: [
      { name: "birdman-game-core", features: ["verus-proof"] }
    ] }]); }).toThrow("Forbidden normal proof activation");
  });
  it.each(["birdman-game-core/verus-proof", "birdman-game-core?/verus-proof"])(
    "rejects indirect application activation through %s", (feature) => {
      expect(() => { assertDependencyBoundary([{ name: "birdman-game-format", dependencies: [
        { name: "birdman-game-core", features: [] }
      ], features: { default: [feature] } }]); }).toThrow("Forbidden normal proof activation");
    }
  );
});

describe("core polynomial roots boundary", () => {
  const polynomial = { ...jsonNumberDependency, name: "polycool", req: "=0.4.0", features: ["libm"] };
  it("allows the pinned no_std root solver only in core", () => {
    expect(() => { assertDependencyBoundary(core(polynomial)); }).not.toThrow();
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-presentation", dependencies: [polynomial] }]); })
      .toThrow("Forbidden dependency");
  });
  it.each([{ uses_default_features: true }, { features: ["std"] }, { features: ["libm", "arbitrary"] }])(
    "rejects default/std/testing activation %j", (difference) => {
      expect(() => { assertDependencyBoundary(core({ ...polynomial, ...difference })); })
        .toThrow("Forbidden dependency");
    }
  );
});

describe("shared image operations boundary", () => {
  const image = { ...jsonNumberDependency, name: "image", req: "=0.25.10" };
  it("allows codec-free operations only in the scene crate", () => {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-scene", dependencies: [image] }]); }).not.toThrow();
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-presentation", dependencies: [image] }]); }).toThrow("Forbidden dependency");
    expect(() => { assertDependencyBoundary(core(image)); }).toThrow("Forbidden dependency");
  });
  it.each([{ uses_default_features: true }, { features: ["png"] }, { features: ["rayon"] }])(
    "rejects codec/default/threading activation %j", (difference) => {
      expect(() => { assertDependencyBoundary([{ name: "birdman-game-scene", dependencies: [{ ...image, ...difference }] }]); })
        .toThrow("Forbidden dependency");
    }
  );
});

function versionedPackages(difference: Readonly<Record<string, unknown>>): unknown[] {
  return [
    { name: "birdman-game-content", dependencies: [{ ...numericalDependency, ...difference }] },
    { name: "birdman-game-presentation", dependencies: [{ ...jsonNumberDependency, ...difference }] },
    { name: "birdman-game-bevy", dependencies: [{ ...nativeEngineDependency, ...difference }] },
    ...schemaProvider("birdman-game-format", { ...schemaDependency, ...difference }),
    ...schemaProvider("birdman-game-app", { ...schemaDependency, ...difference }),
    ...schemaProvider("birdman-game-presentation", { ...schemaDependency, ...difference })
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

  it.each(["birdman-game-format", "birdman-game-presentation"])("allows the fixed ECMAScript formatter in the shared JSON producer %s", (name) => {
    expect(() => { assertDependencyBoundary([{ name, dependencies: [jsonNumberDependency] }]); }).not.toThrow();
  });

  it.each(["birdman-game-core", "birdman-game-app", "birdman-game-scene", "birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"])(
    "rejects a direct JSON-number dependency in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [jsonNumberDependency] }]); }).toThrow("Forbidden dependency");
    }
  );

  it.each([{ req: "^1.0.3" }, { kind: "dev" }, { kind: "build" },
    { target: 'cfg(target_arch = "wasm32")' }, { features: ["no-panic"] }, { uses_default_features: true },
    { optional: true }, { rename: "formatter" }, { source: "git+https://example.invalid/ryu-js" }, { path: "../ryu-js" }])(
    "rejects broader JSON-number dependencies %j", (difference) => {
      for (const name of ["birdman-game-format", "birdman-game-presentation"]) {
        expect(() => { assertDependencyBoundary([{ name, dependencies: [{ ...jsonNumberDependency, ...difference }] }]); }).toThrow("Forbidden dependency");
      }
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
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-content", dependencies: [numericalDependency] }
    ]); }).not.toThrow();
  });

  it.each(["birdman-game-format", "birdman-game-app", "birdman-game-presentation"] as const)(
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
    for (const name of ["birdman-game-format", "birdman-game-app", "birdman-game-presentation"] as const) {
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
    for (const name of ["birdman-game-format", "birdman-game-app", "birdman-game-presentation"] as const) {
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
      name: "birdman-game-cli", dependencies: [{ name: "birdman-game-presentation", optional: true, features: [] }],
      features: { schema: ["dep:birdman-game-presentation", "birdman-game-presentation/schema"] }
    }]); }).not.toThrow();
  });

  it("allows unrelated features without duplicating the entire manifest feature table", () => {
    for (const name of ["birdman-game-format", "birdman-game-app", "birdman-game-presentation"] as const) {
      expect(() => { assertDependencyBoundary(schemaProvider(name, schemaDependency, {
        schema: schemaFeatures(name),
        default: [], audit: []
      })); }).not.toThrow();
    }
    for (const name of ["birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"]) {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [], features: {
        ...(name === "birdman-game-cli" ? { schema: ["dep:birdman-game-presentation", "birdman-game-presentation/schema"] } : {}), default: [], audit: []
      } }]); }).not.toThrow();
    }
  });

  it.each(["schema", "schemars", "dep:schemars", "schemars/derive", "schemars?/derive",
    "birdman-game-format/schema", "birdman-game-format?/schema", "birdman-game-app/schema", "birdman-game-presentation/schema",
    "dep:birdman-game-presentation", "birdman-game-presentation"])(
    "rejects schema generation through another feature (%s)", (feature) => {
      for (const name of ["birdman-game-format", "birdman-game-app", "birdman-game-presentation"] as const) {
        expect(() => { assertDependencyBoundary(schemaProvider(name, schemaDependency, {
          schema: schemaFeatures(name),
          audit: [feature]
        })); }).toThrow("Forbidden dependency");
      }
      expect(() => { assertDependencyBoundary([{ name: "birdman-game-cli", dependencies: [], features: {
        schema: ["dep:birdman-game-presentation", "birdman-game-presentation/schema"], audit: [feature]
      } }]); }).toThrow("Forbidden schema feature boundary");
      for (const name of ["birdman-game-wasm", "birdman-game-bevy"]) {
        expect(() => { assertDependencyBoundary([{ name, dependencies: [], features: { audit: [feature] } }]); })
          .toThrow("Forbidden runtime schema feature");
      }
    }
  );

  it.each([
    { schema: [] }, { schema: ["birdman-game-format/schema"] },
    { schema: ["birdman-game-presentation/schema"], default: ["schema"] },
    { schema: ["birdman-game-presentation/schema"], runtime: ["schema"] }
  ])("rejects broader CLI schema activation %j", (features) => {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-cli", dependencies: [], features }]); })
      .toThrow("Forbidden schema feature boundary");
  });

  it.each([
    { schema: [] }, { default: ["schema"] }, { runtime: ["birdman-game-presentation/schema"] },
    { default: ["birdman-game-format/schema"] }, { runtime: ["birdman-game-presentation?/schema"] }
  ])("rejects runtime schema feature forwarding %j", (features) => {
    for (const name of ["birdman-game-wasm", "birdman-game-bevy"]) {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [], features }]); })
        .toThrow("Forbidden runtime schema feature");
    }
  });

  it.each(["birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy"])(
    "rejects schema requested by normal shared dependencies in %s", (name) => {
      for (const dependency of ["birdman-game-format", "birdman-game-app", "birdman-game-presentation"]) {
        expect(() => { assertDependencyBoundary([{
          name, dependencies: [{ name: dependency, kind: null, features: ["schema"] }]
        }]); }).toThrow("Forbidden normal schema activation");
      }
    }
  );

  it("keeps native engine and shared session dependencies outside core and format", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-presentation", dependencies: [{ name: "birdman-game-core" }, { name: "birdman-game-format" }] },
      { name: "birdman-game-wasm", dependencies: [{ name: "birdman-game-presentation" }] },
      { name: "birdman-game-bevy", dependencies: [nativeEngineDependency, { name: "birdman-game-presentation" }] }
    ]); }).not.toThrow();
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-presentation", "birdman-game-scene", "birdman-game-wasm"])(
    "rejects the native engine in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [nativeEngineDependency] }]); }).toThrow();
    }
  );

  it("allows the pinned desktop native complex-script segmentation backport", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [nativeTextDependency] }
    ]); }).not.toThrow();
  });

  it("limits the additional Linux engine declaration to its two window backends", () => {
    const dependency = { ...nativeEngineDependency, target: NATIVE_LINUX_WINDOW_TARGET, features: ["x11", "wayland"] };
    const check = (difference: object): void => {
      assertDependencyBoundary([
        { name: "birdman-game-bevy", dependencies: [{ ...dependency, ...difference }] }
      ]);
    };
    expect(() => { check({}); }).not.toThrow();
    for (const difference of [
      { target: null }, { target: 'cfg(target_arch = "wasm32")' }, { kind: "dev" },
      { features: ["x11"] }, { features: ["x11", "wayland", "bevy_audio"] }
    ]) expect(() => { check(difference); }).toThrow("Forbidden native engine dependency");
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-presentation", "birdman-game-scene", "birdman-game-wasm"])(
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
    { features: ["audio"] }, { features: ["web"] },
    { features: ["webgl2"] }, { features: ["webgpu"] }
  ])("rejects a broader native engine dependency %j", (difference) => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [{ ...nativeEngineDependency, ...difference }] }
    ]); }).toThrow("Forbidden native engine dependency");
  });

  it("accepts desktop gamepad observation through the native engine", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [{ ...nativeEngineDependency, features: ["bevy_gilrs"] }] }
    ]); }).not.toThrow();
  });

  it("rejects a dev-only native font feature beside the product native engine", () => {
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-bevy", dependencies: [nativeEngineDependency, nativeTestFontDependency] }
    ]); }).toThrow("Forbidden native engine dependency");
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-presentation", "birdman-game-cli", "birdman-game-wasm"])(
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
    expect(() => { assertDependencyBoundary([
      { name: "birdman-game-content", dependencies: [{ ...numericalDependency, ...difference }] }
    ]); }).toThrow("Forbidden dependency");
  });

  it.each(["birdman-game-core", "birdman-game-format", "birdman-game-presentation", "birdman-game-cli", "birdman-game-wasm", "birdman-game-bevy", "unexpected-crate"])(
    "rejects nalgebra in %s", (name) => {
      expect(() => { assertDependencyBoundary([{ name, dependencies: [numericalDependency] }]); }).toThrow();
    }
  );

  it("rejects unexpected crates even without dependencies", () => {
    expect(() => { assertDependencyBoundary([{ name: "unexpected-crate", dependencies: [] }]); }).toThrow("Unexpected crate");
  });
});

it.each(["birdman-game-scene", "birdman-game-presentation", "image", "earcut", "ryu-js", "time", "unicode-width", "bevy", "wasm-bindgen"])(
  "keeps presentation and platform dependencies outside the application crate (%s)", (name) => {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-app", dependencies: [{ name }] }]); }).toThrow();
  }
);

it("keeps the CLI presentation dependency optional", () => {
  const dependency = { name: "birdman-game-presentation", features: [] };
  const features = { schema: ["dep:birdman-game-presentation", "birdman-game-presentation/schema"] };
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-cli", features, dependencies: [dependency] }]); })
    .toThrow("Forbidden normal presentation dependency");
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-cli", features, dependencies: [{ ...dependency, optional: true }] }]); })
    .not.toThrow();
});

it("limits scene access from presentation to camera/geometry tests", () => {
  const dependency = { name: "birdman-game-scene", kind: "dev", target: null, optional: false, rename: null, features: [] };
  expect(() => { assertDependencyBoundary([{ name: "birdman-game-presentation", dependencies: [dependency] }]); }).not.toThrow();
  for (const difference of [{ kind: null }, { kind: "build" }, { target: HOST_NUMERICAL_TEST_TARGET }]) {
    expect(() => { assertDependencyBoundary([{ name: "birdman-game-presentation", dependencies: [{ ...dependency, ...difference }] }]); }).toThrow("Forbidden dependency");
  }
  for (const name of ["birdman-game-cli", "birdman-game-app", "birdman-game-scene"]) {
    const dependencyName = name === "birdman-game-scene" ? "birdman-game-presentation" : "birdman-game-scene";
    expect(() => { assertDependencyBoundary([{ name, dependencies: [{ name: dependencyName, kind: null }] }]); }).toThrow("Forbidden dependency");
  }
});
