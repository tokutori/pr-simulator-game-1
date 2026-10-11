import { record, nonEmpty } from "../shared/validation.js";

export const HOST_NUMERICAL_TEST_TARGET = 'cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))';
export const NATIVE_SCREEN_TARGET = 'cfg(target_os = "windows")';

const allowed = new Map<string, readonly string[]>([
  ["birdman-game-core", ["libm"]],
  ["birdman-game-format", ["birdman-game-core", "serde", "serde_json", "sha2"]],
  ["birdman-game-cli", ["birdman-game-core", "birdman-game-format", "birdman-game-session", "serde", "serde_json"]],
  ["birdman-game-wasm", ["birdman-game-core", "birdman-game-format", "birdman-game-session", "serde", "serde_json", "sha2", "wasm-bindgen"]],
  ["birdman-game-session", ["birdman-game-core", "birdman-game-format", "serde", "serde_json", "sha2"]],
  ["birdman-game-bevy", ["birdman-game-core", "birdman-game-format", "birdman-game-session", "bevy", "serde", "serde_json"]]
]);

const excludedNativeFeatures = new Set([
  "default", "default_platform", "3d", "ui", "audio", "bevy_audio",
  "bevy_gilrs", "web", "webgl2", "webgpu"
]);

export function assertNativeTextPatch(workspace: unknown): void {
  const manifest = record(workspace);
  const patch = record(record(manifest.patch)["crates-io"]);
  const replacement = record(patch.parley);
  const excluded = record(manifest.workspace).exclude;
  if (replacement.path !== "vendor/parley" || Object.keys(replacement).length !== 1
    || !Array.isArray(excluded) || !excluded.includes("vendor/parley")) {
    throw new Error("Invalid native text segmentation backport");
  }
}

function isNativeEngineDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-bevy"
    && dependency.kind === null
    && dependency.target === NATIVE_SCREEN_TARGET
    && dependency.req === "=0.19.1"
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.every((feature) => typeof feature === "string" && !excludedNativeFeatures.has(feature));
}

function isNativeTextDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-bevy"
    && dependency.kind === null
    && dependency.target === NATIVE_SCREEN_TARGET
    && dependency.req === "=0.9.0"
    && dependency.source === "registry+https://github.com/rust-lang/crates.io-index"
    && dependency.registry === null
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 2
    && dependency.features.includes("std")
    && dependency.features.includes("complex-scripts");
}

function isHostNumericalTestDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-core"
    && dependency.name === "nalgebra"
    && dependency.kind === "dev"
    && dependency.target === HOST_NUMERICAL_TEST_TARGET
    && dependency.req === "=0.35.0"
    && dependency.source === "registry+https://github.com/rust-lang/crates.io-index"
    && dependency.registry === null
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 1
    && dependency.features[0] === "std";
}

function hasSchemaFeature(features: unknown, expected: readonly string[]): boolean {
  if (typeof features !== "object" || features === null || Array.isArray(features)) return false;
  const values = record(features);
  const schema = values.schema;
  return Object.keys(values).length === 1
    && Array.isArray(schema)
    && schema.length === expected.length
    && expected.every((feature) => schema.includes(feature));
}

function isSchemaGenerationDependency(crate: string, dependency: Record<string, unknown>, features: unknown): boolean {
  const expected = crate === "birdman-game-format" ? ["dep:schemars"]
    : crate === "birdman-game-session" ? ["dep:schemars", "birdman-game-format/schema"] : undefined;
  return expected !== undefined
    && hasSchemaFeature(features, expected)
    && dependency.name === "schemars"
    && dependency.kind === null
    && dependency.target === null
    && dependency.req === "=1.2.2"
    && dependency.source === "registry+https://github.com/rust-lang/crates.io-index"
    && dependency.registry === null
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === true
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 1
    && dependency.features[0] === "derive";
}

function assertSchemaFeatureBoundary(crate: string, features: unknown): void {
  if (features === undefined) return;
  if (crate === "birdman-game-cli"
    && !hasSchemaFeature(features, ["birdman-game-session/schema"])) {
    throw new Error(`Forbidden schema feature boundary in ${crate}`);
  }
  if (crate !== "birdman-game-wasm" && crate !== "birdman-game-bevy") return;
  const values = record(features);
  if ("schema" in values || Object.values(values).some((value) => Array.isArray(value)
    && value.some((feature: unknown) => typeof feature === "string"
      && (feature === "schema" || feature.endsWith("/schema"))))) {
    throw new Error(`Forbidden runtime schema feature in ${crate}`);
  }
}

export function assertDependencyBoundary(packages: readonly unknown[]): void {
  for (const value of packages) {
    const pkg = record(value);
    const name = nonEmpty(pkg.name);
    const permitted = allowed.get(name);
    if (permitted === undefined || !Array.isArray(pkg.dependencies)) throw new Error(`Unexpected crate: ${name}`);
    assertSchemaFeatureBoundary(name, pkg.features);
    for (const value of pkg.dependencies) {
      const dependency = record(value);
      const dependencyName = nonEmpty(dependency.name);
      if (dependencyName === "bevy" && !isNativeEngineDependency(name, dependency)) {
        throw new Error(`Forbidden native engine dependency in ${name}`);
      }
      if (["birdman-game-format", "birdman-game-session"].includes(dependencyName)
        && Array.isArray(dependency.features) && dependency.features.includes("schema")) {
        throw new Error(`Forbidden normal schema activation in ${name}: ${dependencyName}`);
      }
      if (!permitted.includes(dependencyName)
        && !isNativeTextDependency(name, dependency)
        && !isHostNumericalTestDependency(name, dependency)
        && !isSchemaGenerationDependency(name, dependency, pkg.features)) {
        throw new Error(`Forbidden dependency in ${name}: ${dependencyName}`);
      }
    }
  }
}
