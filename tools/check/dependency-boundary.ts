import { record, nonEmpty } from "../shared/validation.js";

export const HOST_NUMERICAL_TEST_TARGET = 'cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))';
export const NATIVE_SCREEN_TARGET = 'cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))';
export const NATIVE_LINUX_WINDOW_TARGET = 'cfg(target_os = "linux")';

const allowed = new Map<string, readonly string[]>([
  ["birdman-game-core", ["libm"]],
  ["birdman-game-app", ["birdman-game-content", "birdman-game-core", "birdman-game-format", "serde", "serde_json", "sha2", "time"]],
  ["birdman-game-content", ["birdman-game-core", "birdman-game-format", "libm"]],
  ["birdman-game-format", ["birdman-game-core", "serde", "serde_json", "sha2"]],
  ["birdman-game-cli", ["birdman-game-app", "birdman-game-content", "birdman-game-core", "birdman-game-format", "serde", "serde_json"]],
  ["birdman-game-wasm", ["birdman-game-app", "birdman-game-presentation", "birdman-game-runtime", "serde_json", "wasm-bindgen"]],
  ["birdman-game-runtime", ["birdman-game-app", "birdman-game-core", "birdman-game-format", "birdman-game-presentation", "birdman-game-scene", "serde", "serde_json", "time"]],
  ["birdman-game-presentation", ["birdman-game-app", "birdman-game-core", "birdman-game-format", "serde", "serde_json", "time", "unicode-width"]],
  ["birdman-game-scene", ["birdman-game-app", "birdman-game-content", "birdman-game-core", "birdman-game-format", "serde", "serde_json", "sha2", "earcut"]],
  ["birdman-game-bevy", ["birdman-game-app", "birdman-game-core", "birdman-game-format", "birdman-game-presentation", "birdman-game-runtime", "birdman-game-scene", "bevy", "serde", "serde_json"]]
]);

const excludedNativeFeatures = new Set([
  "default", "default_platform", "default_font", "3d", "ui", "audio", "bevy_audio",
  "web", "webgl2", "webgpu"
]);

function isCompleteVersionPin(value: unknown): boolean {
  return typeof value === "string" && /^=\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(value);
}

function isDefaultRegistrySource(dependency: Record<string, unknown>): boolean {
  return dependency.registry === null && typeof dependency.source === "string"
    && /^(registry|sparse)\+.+$/.test(dependency.source);
}

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
    && (dependency.target === NATIVE_SCREEN_TARGET
      || dependency.target === NATIVE_LINUX_WINDOW_TARGET && Array.isArray(dependency.features)
      && dependency.features.length === 2 && dependency.features.includes("x11") && dependency.features.includes("wayland"))
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.kind === null
    && dependency.features.every((feature) => typeof feature === "string" && !excludedNativeFeatures.has(feature));
}

function isNativeGpuIoDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-bevy"
    && dependency.name === "wgpu"
    && dependency.kind === null
    && dependency.target === NATIVE_SCREEN_TARGET
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 0;
}

function isNativeTextDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-bevy"
    && dependency.name === "parley"
    && dependency.kind === null
    && dependency.target === NATIVE_SCREEN_TARGET
    && dependency.req === "=0.9.0"
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 2
    && dependency.features.includes("std")
    && dependency.features.includes("complex-scripts");
}

function isSharedJsonNumberDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return ["birdman-game-format", "birdman-game-presentation"].includes(crate)
    && dependency.name === "ryu-js"
    && dependency.kind === null
    && dependency.target === null
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 0;
}

function isSharedImageOperationsDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-scene"
    && dependency.name === "image"
    && dependency.kind === null
    && dependency.target === null
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 1
    && dependency.features[0] === "png";
}

function isPresentationSceneTestDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-presentation" && dependency.name === "birdman-game-scene"
    && dependency.kind === "dev" && dependency.target === null
    && dependency.optional === false && dependency.rename === null
    && Array.isArray(dependency.features) && dependency.features.length === 0;
}

function isHostNumericalTestDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-content"
    && dependency.name === "nalgebra"
    && dependency.kind === "dev"
    && dependency.target === HOST_NUMERICAL_TEST_TARGET
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 1
    && dependency.features[0] === "std";
}

function isCorePolynomialDependency(crate: string, dependency: Record<string, unknown>): boolean {
  return crate === "birdman-game-core"
    && dependency.name === "polycool"
    && dependency.kind === null
    && dependency.target === null
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
    && dependency.path === undefined
    && dependency.rename === null
    && dependency.optional === false
    && dependency.uses_default_features === false
    && Array.isArray(dependency.features)
    && dependency.features.length === 1
    && dependency.features[0] === "libm";
}

function proofFeatures(crate: string): readonly string[] | undefined {
  return crate === "birdman-game-core" ? ["dep:vstd"]
    : crate === "birdman-game-presentation" ? ["dep:vstd", "birdman-game-core/verus-proof"]
    : crate === "birdman-game-runtime" ? ["dep:vstd", "birdman-game-core/verus-proof", "birdman-game-presentation/verus-proof"]
    : undefined;
}

function activatesProof(feature: unknown): boolean {
  return typeof feature === "string" && (["verus-proof", "dep:vstd", "vstd"].includes(feature)
    || feature.startsWith("vstd/") || feature.startsWith("vstd?/") || feature.endsWith("/verus-proof"));
}

function isProofDependency(crate: string, dependency: Record<string, unknown>, features: unknown): boolean {
  const expected = proofFeatures(crate);
  if (expected === undefined || features === undefined) return false;
  const definitions = record(features);
  const proof = definitions["verus-proof"];
  return dependency.name === "vstd" && dependency.kind === null && dependency.target === null
    && isCompleteVersionPin(dependency.req) && isDefaultRegistrySource(dependency)
    && dependency.path === undefined && dependency.rename === null && dependency.optional === true
    && dependency.uses_default_features === false && Array.isArray(dependency.features)
    && dependency.features.length === 1 && dependency.features[0] === "alloc"
    && Array.isArray(proof) && proof.length === expected.length && expected.every(feature => proof.includes(feature));
}

function assertProofFeatureBoundary(crate: string, features: unknown, dependencies: readonly unknown[]): void {
  if (features === undefined) return;
  const definitions = record(features);
  if (Object.entries(definitions).some(([name, enabled]) => name !== "verus-proof"
    && Array.isArray(enabled) && enabled.some(activatesProof))) {
    throw new Error(`Forbidden normal proof activation in ${crate}`);
  }
  if (!("verus-proof" in definitions)) return;
  const expected = proofFeatures(crate);
  const proof = definitions["verus-proof"];
  if (expected === undefined || !Array.isArray(proof) || proof.length !== expected.length
    || !expected.every(feature => proof.includes(feature))
    || !dependencies.some(value => record(value).name === "vstd")
    || expected.filter(feature => feature !== "dep:vstd").some(feature => {
      const target = feature.split("/")[0];
      return !dependencies.some(value => {
        const dependency = record(value);
        return dependency.name === target && dependency.kind === null && dependency.target === null
          && dependency.rename === null && dependency.optional === false;
      });
    })) {
    throw new Error(`Forbidden proof feature boundary in ${crate}`);
  }
}

function activatesSchema(feature: unknown): boolean {
  return typeof feature === "string" && (feature === "dep:birdman-game-presentation" || feature === "birdman-game-presentation" || feature === "schema" || feature === "schemars" || feature === "dep:schemars"
    || feature.startsWith("schemars/") || feature.startsWith("schemars?/") || feature.endsWith("/schema"));
}

function hasSchemaFeature(features: unknown, expected: readonly string[]): boolean {
  if (typeof features !== "object" || features === null || Array.isArray(features)) return false;
  const values = record(features);
  const schema = values.schema;
  return Array.isArray(schema)
    && schema.length === expected.length
    && expected.every((feature) => schema.includes(feature))
    && Object.entries(values).every(([name, enabled]) => name === "schema" || Array.isArray(enabled)
      && enabled.every((feature: unknown) => typeof feature === "string" && !activatesSchema(feature)));
}

function isSchemaGenerationDependency(crate: string, dependency: Record<string, unknown>, features: unknown): boolean {
  const expected = crate === "birdman-game-format" || crate === "birdman-game-scene" ? ["dep:schemars"]
    : crate === "birdman-game-app" ? ["dep:schemars", "birdman-game-format/schema"]
    : crate === "birdman-game-presentation" ? ["dep:schemars", "birdman-game-app/schema", "birdman-game-format/schema"]
    : crate === "birdman-game-runtime" ? ["dep:schemars", "birdman-game-app/schema", "birdman-game-presentation/schema", "birdman-game-scene/schema"] : undefined;
  return expected !== undefined
    && hasSchemaFeature(features, expected)
    && dependency.name === "schemars"
    && dependency.kind === null
    && dependency.target === null
    && isCompleteVersionPin(dependency.req)
    && isDefaultRegistrySource(dependency)
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
    && ("schema" in record(features) || Object.values(record(features)).some((value) => Array.isArray(value) && value.some(activatesSchema)))) {
    throw new Error(`Forbidden schema feature boundary in ${crate}`);
  }
  if (crate !== "birdman-game-wasm" && crate !== "birdman-game-bevy") return;
  const values = record(features);
  if ("schema" in values || Object.values(values).some((value) => Array.isArray(value)
    && value.some(activatesSchema))) {
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
    assertProofFeatureBoundary(name, pkg.features, pkg.dependencies);
    for (const value of pkg.dependencies) {
      const dependency = record(value);
      const dependencyName = nonEmpty(dependency.name);
      if (dependencyName === "bevy" && !isNativeEngineDependency(name, dependency)) {
        throw new Error(`Forbidden native engine dependency in ${name}`);
      }
      if (["birdman-game-format", "birdman-game-app", "birdman-game-presentation", "birdman-game-scene", "birdman-game-runtime"].includes(dependencyName)
        && Array.isArray(dependency.features) && dependency.features.includes("schema")) {
        throw new Error(`Forbidden normal schema activation in ${name}: ${dependencyName}`);
      }
      if (Array.isArray(dependency.features) && dependency.features.includes("verus-proof")) {
        throw new Error(`Forbidden normal proof activation in ${name}: ${dependencyName}`);
      }
      if (!permitted.includes(dependencyName)
        && !isNativeGpuIoDependency(name, dependency)
        && !isNativeTextDependency(name, dependency)
        && !isSharedJsonNumberDependency(name, dependency)
        && !isSharedImageOperationsDependency(name, dependency)
        && !isPresentationSceneTestDependency(name, dependency)
        && !isHostNumericalTestDependency(name, dependency)
        && !isCorePolynomialDependency(name, dependency)
        && !isProofDependency(name, dependency, pkg.features)
        && !isSchemaGenerationDependency(name, dependency, pkg.features)) {
        throw new Error(`Forbidden dependency in ${name}: ${dependencyName}`);
      }
    }
  }
}
