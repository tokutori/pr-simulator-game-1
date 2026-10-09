import { normalize } from "node:path";
import { record, strings } from "../shared/validation.js";

export interface CargoWasmBuildOutput {
  readonly diagnostics: readonly string[];
  readonly artifactFilenames: readonly string[];
  readonly succeeded: boolean;
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
