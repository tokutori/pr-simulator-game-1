import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";

const root = fileURLToPath(new URL("../..", import.meta.url));

afterEach(() => {
  vi.restoreAllMocks();
  vi.doUnmock("node:child_process");
  vi.doUnmock("node:fs/promises");
});

function fixture(exitStatus: number | null = 0, buildError?: Error) {
  vi.resetModules();
  const legacyArtifact = resolve(root, "target/wasm32-unknown-unknown/release/birdman_game_wasm.wasm");
  const currentArtifact = resolve(root, "custom target/wasm32-unknown-unknown/release/birdman_game_wasm.wasm");
  const files = new Map([[legacyArtifact, "stale WASM"], [currentArtifact, "current WASM"]]);
  const calls: { readonly command: string; readonly args: readonly string[]; readonly options: Record<string, unknown> }[] = [];
  const consumed: string[] = [];
  const diagnostic = "error: current WASM build failed\n";
  const output = [
    { reason: "compiler-artifact", manifest_path: resolve(root, "crates/birdman-game-wasm/Cargo.toml"),
      target: { name: "birdman_game_wasm", crate_types: ["cdylib", "rlib"] }, filenames: [currentArtifact] },
    ...(exitStatus === 0 ? [] : [{ reason: "compiler-message", message: { rendered: diagnostic } }]),
    { reason: "build-finished", success: exitStatus === 0 }
  ].map((message) => JSON.stringify(message)).join("\n");
  const mkdir = vi.fn();
  const rm = vi.fn();
  vi.doMock("node:fs/promises", () => ({ mkdir, rm }));
  vi.doMock("node:child_process", () => ({
    spawnSync: (command: string, args: readonly string[], options: Record<string, unknown>) => {
      calls.push({ command, args, options });
      if (command === "cargo") return { status: exitStatus, stdout: output, error: buildError };
      if (args[0] === "--version") return { status: 0, stdout: "wasm-bindgen 0.2.129\n" };
      const input = args.at(-1);
      if (input === undefined) throw new Error("wasm-bindgen requires an input artifact");
      const contents = files.get(input);
      if (contents === undefined) throw new Error(`Artifact is missing: ${input}`);
      consumed.push(contents);
      return { status: 0, stdout: "" };
    }
  }));
  const stderr = vi.spyOn(process.stderr, "write").mockReturnValue(true);
  return { calls, consumed, mkdir, rm, stderr, currentArtifact, legacyArtifact, diagnostic };
}

describe("WASM build command artifact ownership", () => {
  it("uses the custom target artifact while an old default-target artifact exists", async () => {
    const trial = fixture();
    await import("./main.js");
    const cargo = trial.calls.find((call) => call.command === "cargo");
    const bindgen = trial.calls.find((call) => call.command === "wasm-bindgen" && call.args[0] !== "--version");
    expect(cargo?.args).toContain("--message-format=json");
    expect(cargo?.args).not.toContain("--target-dir");
    expect(cargo?.options.maxBuffer).toBeGreaterThan(1_024 * 1_024);
    expect(bindgen?.args.at(-1)).toBe(trial.currentArtifact);
    expect(bindgen?.args.at(-1)).not.toBe(trial.legacyArtifact);
    expect(trial.consumed).toEqual(["current WASM"]);
    expect(trial.rm).toHaveBeenCalledTimes(1);
    expect(trial.mkdir).toHaveBeenCalledTimes(1);
  });

  it.each([101, null])("preserves build diagnostics and skips generation after status %s", async (status) => {
    const trial = fixture(status);
    await expect(import("./main.js")).rejects.toThrow(`cargo exited with status ${String(status)}`);
    expect(trial.stderr).toHaveBeenCalledWith(trial.diagnostic);
    expect(trial.calls.filter((call) => call.command === "wasm-bindgen" && call.args[0] !== "--version")).toHaveLength(0);
    expect(trial.consumed).toEqual([]);
    expect(trial.rm).not.toHaveBeenCalled();
    expect(trial.mkdir).not.toHaveBeenCalled();
  });

  it("preserves a process launch error without selecting any old artifact", async () => {
    const failure = new Error("Injected cargo launch failure");
    const trial = fixture(null, failure);
    await expect(import("./main.js")).rejects.toBe(failure);
    expect(trial.consumed).toEqual([]);
    expect(trial.rm).not.toHaveBeenCalled();
    expect(trial.mkdir).not.toHaveBeenCalled();
  });
});
