import { describe, expect, it } from "vitest";
import { parseCargoWasmBuildOutput, selectWasmBuildArtifact } from "./cargo-build-output.js";

const manifest = "C:/checkout/crates/birdman-game-wasm/Cargo.toml";

function artifact(filename: string, overrides: Record<string, unknown> = {}) {
  return { reason: "compiler-artifact", manifest_path: manifest,
    target: { name: "birdman_game_wasm", crate_types: ["cdylib", "rlib"] },
    filenames: [filename, filename.replace(/\.wasm$/, ".rlib")], fresh: false, ...overrides };
}

function messages(...values: readonly unknown[]): string {
  return values.map((value) => JSON.stringify(value)).join("\n");
}

const finished = { reason: "build-finished", success: true };

describe("Cargo WASM build artifact selection", () => {
  it.each([
    "C:/checkout/target/wasm32-unknown-unknown/release/birdman_game_wasm.wasm",
    "C:/shared cargo cache/wasm32-unknown-unknown/release/birdman_game_wasm.wasm",
    "/tmp/custom-target/wasm32-unknown-unknown/release/birdman_game_wasm.wasm"
  ])("selects Cargo's actual artifact filename %s", (filename) => {
    const output = parseCargoWasmBuildOutput(messages(artifact(filename), finished), manifest);
    expect(selectWasmBuildArtifact(output, 0)).toBe(filename);
    expect(output.artifactFilenames).toEqual([filename]);
  });

  it("accepts Cargo-confirmed fresh artifacts rather than requiring a redundant compiler invocation", () => {
    const filename = "C:/shared-cache/wasm32-unknown-unknown/release/birdman_game_wasm.wasm";
    const output = parseCargoWasmBuildOutput(messages(artifact(filename, { fresh: true }), finished), manifest);
    expect(selectWasmBuildArtifact(output, 0)).toBe(filename);
  });

  it("ignores artifacts from other manifests and non-cdylib targets", () => {
    const selected = "C:/shared-cache/wasm32-unknown-unknown/release/birdman_game_wasm.wasm";
    const ignored = "C:/checkout/target/wasm32-unknown-unknown/release/birdman_game_wasm.wasm";
    const output = parseCargoWasmBuildOutput(messages(
      artifact(ignored, { manifest_path: "C:/another-checkout/crates/birdman-game-wasm/Cargo.toml" }),
      artifact(ignored, { target: { name: "birdman_game_wasm", crate_types: ["rlib"] } }),
      artifact(selected), finished), manifest);
    expect(selectWasmBuildArtifact(output, 0)).toBe(selected);
  });

  it.each([101, null])("rejects failed process status %s even when an artifact was emitted", (status) => {
    const output = parseCargoWasmBuildOutput(messages(artifact("/tmp/current.wasm"), finished), manifest);
    expect(() => selectWasmBuildArtifact(output, status)).toThrow(`cargo exited with status ${String(status)}`);
  });

  it.each([
    { name: "missing", completion: [] },
    { name: "failed", completion: [{ reason: "build-finished", success: false }] }
  ])("requires the successful build-finished message when completion is $name", ({ completion }) => {
    const output = parseCargoWasmBuildOutput(messages(artifact("/tmp/current.wasm"), ...completion), manifest);
    expect(() => selectWasmBuildArtifact(output, 0)).toThrow("successful WASM build");
  });

  it.each([
    { name: "missing", artifacts: [] },
    { name: "ambiguous", artifacts: [artifact("/tmp/first.wasm"), artifact("/tmp/second.wasm")] }
  ])("rejects $name WASM artifacts", ({ artifacts }) => {
    const output = parseCargoWasmBuildOutput(messages(...artifacts, finished), manifest);
    expect(() => selectWasmBuildArtifact(output, 0)).toThrow("exactly one");
  });

  it("preserves rendered compiler diagnostics and non-JSON tool output", () => {
    const output = parseCargoWasmBuildOutput(`tool output\n${messages(
      { reason: "compiler-message", message: { rendered: "error: compile failed\n" } },
      { reason: "build-script-executed" }, { reason: "build-finished", success: false })}`, manifest);
    expect(output.diagnostics).toEqual(["tool output\n", "error: compile failed\n"]);
  });

  it("rejects malformed JSON and an invalid completion result", () => {
    expect(() => parseCargoWasmBuildOutput('{"reason":', manifest)).toThrow();
    expect(() => parseCargoWasmBuildOutput(messages({ reason: "build-finished", success: "true" }), manifest)).toThrow("boolean success");
  });
});
