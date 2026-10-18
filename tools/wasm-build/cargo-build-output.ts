import { realpath } from "node:fs/promises";
import { basename, dirname, isAbsolute, join, normalize, relative, sep } from "node:path";
import { nonEmpty, record, strings } from "../shared/validation.js";

export interface CargoWasmBuildOutput {
  readonly diagnostics: readonly string[];
  readonly artifactFilenames: readonly string[];
  readonly succeeded: boolean;
}

export async function assertSafeCargoBuildDirectories(stdout: string): Promise<void> {
  const metadata = record(JSON.parse(stdout) as unknown);
  const sourcePaths = [nonEmpty(metadata.workspace_root)];
  if (!Array.isArray(metadata.packages)) throw new Error("Cargo metadata requires workspace packages");
  for (const value of metadata.packages) {
    const member = record(value);
    sourcePaths.push(dirname(nonEmpty(member.manifest_path)));
    if (!Array.isArray(member.targets)) throw new Error("Cargo metadata requires package targets");
    for (const target of member.targets) sourcePaths.push(nonEmpty(record(target).src_path));
  }
  const sources = await Promise.all(sourcePaths.map(canonicalPath));
  for (const field of ["target_directory", "build_directory"] as const) {
    const directory = await canonicalPath(nonEmpty(metadata[field]));
    for (const source of sources) {
      const contained = relative(directory, source);
      if (contained === "" || (!isAbsolute(contained) && contained !== ".." && !contained.startsWith(`..${sep}`))) {
        throw new Error(`Unsafe Cargo ${field}: ${directory} contains workspace/source ${source}. Keep Cargo output outside source ancestors, using the workspace target subdirectory or a sibling cache.`);
      }
    }
  }
}

async function canonicalPath(path: string): Promise<string> {
  if (!isAbsolute(path)) throw new Error(`Cargo metadata requires an absolute path: ${path}`);
  try {
    return await realpath(path);
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT") || dirname(path) === path) throw error;
    return join(await canonicalPath(dirname(path)), basename(path));
  }
}

export function parseCargoWasmBuildOutput(stdout: string, manifestPath: string): CargoWasmBuildOutput {
  const diagnostics: string[] = [];
  const artifacts: string[] = [];
  let succeeded = false;
  for (const line of stdout.split(/\r?\n/)) {
    if (line.trim().length === 0) continue;
    if (!line.startsWith("{")) {
      diagnostics.push(`${line}\n`);
      continue;
    }
    const message = record(JSON.parse(line) as unknown);
    switch (message.reason) {
      case "compiler-message": {
        const diagnostic = record(message.message);
        if (typeof diagnostic.rendered === "string") diagnostics.push(diagnostic.rendered);
        break;
      }
      case "compiler-artifact": {
        if (typeof message.manifest_path !== "string" || normalize(message.manifest_path) !== normalize(manifestPath)) break;
        const target = record(message.target);
        if (target.name !== "birdman_game_wasm" || !strings(target.crate_types).includes("cdylib")) break;
        artifacts.push(...strings(message.filenames).filter((filename) => filename.endsWith(".wasm")));
        break;
      }
      case "build-finished":
        if (typeof message.success !== "boolean") throw new Error("Cargo build-finished requires a boolean success result");
        succeeded = message.success;
        break;
    }
  }
  return Object.freeze({ diagnostics: Object.freeze(diagnostics), artifactFilenames: Object.freeze(artifacts), succeeded });
}

export function selectWasmBuildArtifact(output: CargoWasmBuildOutput, exitStatus: number | null): string {
  if (exitStatus !== 0) throw new Error(`cargo exited with status ${String(exitStatus)}`);
  if (!output.succeeded) throw new Error("Cargo did not report a successful WASM build");
  const [artifact] = output.artifactFilenames;
  if (output.artifactFilenames.length !== 1 || artifact === undefined) throw new Error("Cargo must report exactly one birdman-game-wasm artifact");
  return artifact;
}
