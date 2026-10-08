import { record, nonEmpty } from "../shared/validation.js";

export const HOST_NUMERICAL_TEST_TARGET = 'cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))';

const allowed = new Map<string, readonly string[]>([
  ["birdman-game-core", ["libm"]],
  ["birdman-game-format", ["birdman-game-core", "serde", "serde_json", "sha2"]],
  ["birdman-game-cli", ["birdman-game-core", "birdman-game-format", "serde", "serde_json"]],
  ["birdman-game-wasm", ["birdman-game-core", "birdman-game-format", "serde", "serde_json", "sha2", "wasm-bindgen"]]
]);

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

export function assertDependencyBoundary(packages: readonly unknown[]): void {
  for (const value of packages) {
    const pkg = record(value);
    const name = nonEmpty(pkg.name);
    const permitted = allowed.get(name);
    if (permitted === undefined || !Array.isArray(pkg.dependencies)) throw new Error(`Unexpected crate: ${name}`);
    for (const value of pkg.dependencies) {
      const dependency = record(value);
      const dependencyName = nonEmpty(dependency.name);
      if (!permitted.includes(dependencyName) && !isHostNumericalTestDependency(name, dependency)) {
        throw new Error(`Forbidden dependency in ${name}: ${dependencyName}`);
      }
    }
  }
}
