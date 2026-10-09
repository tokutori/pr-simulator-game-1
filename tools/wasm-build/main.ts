import { spawnSync } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseCargoWasmBuildOutput, selectWasmBuildArtifact } from "./cargo-build-output.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const wasmBindgenVersion = "0.2.129";
const wasmOutputDirectory = resolve(root, "web/pkg");

const version = run("wasm-bindgen", ["--version"], true).trim();
if (version !== `wasm-bindgen ${wasmBindgenVersion}`) {
  throw new Error(`Expected wasm-bindgen ${wasmBindgenVersion}; found ${version}`);
}

const build = spawnSync("cargo", ["build", "--release", "-p", "birdman-game-wasm", "--target", "wasm32-unknown-unknown", "--locked", "--message-format=json"], {
  cwd: root,
  encoding: "utf8",
  stdio: ["inherit", "pipe", "inherit"],
  windowsHide: true,
  maxBuffer: 16 * 1024 * 1024
});
if (build.error !== undefined) throw build.error;
const output = parseCargoWasmBuildOutput(build.stdout, resolve(root, "crates/birdman-game-wasm/Cargo.toml"));
for (const diagnostic of output.diagnostics) process.stderr.write(diagnostic);
const wasmInput = selectWasmBuildArtifact(output, build.status);
await rm(wasmOutputDirectory, { recursive: true, force: true });
await mkdir(wasmOutputDirectory, { recursive: true });
run("wasm-bindgen", [
  "--target", "web",
  "--out-dir", wasmOutputDirectory,
  "--out-name", "birdman_game_wasm",
  wasmInput
]);

function run(command: string, args: string[], captureOutput = false): string {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: captureOutput ? ["inherit", "pipe", "inherit"] : "inherit",
    windowsHide: true
  });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with status ${String(result.status)}`);
  return captureOutput ? result.stdout : "";
}
