import { spawnSync } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const wasmBindgenVersion = "0.2.129";
const wasmOutputDirectory = resolve(root, "web/pkg");
const wasmInput = resolve(root, "target/wasm32-unknown-unknown/release/birdman_game_wasm.wasm");

const version = run("wasm-bindgen", ["--version"], true).trim();
if (version !== `wasm-bindgen ${wasmBindgenVersion}`) {
  throw new Error(`Expected wasm-bindgen ${wasmBindgenVersion}; found ${version}`);
}

run("cargo", ["build", "--release", "-p", "birdman-game-wasm", "--target", "wasm32-unknown-unknown", "--locked"]);
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
    stdio: captureOutput ? "pipe" : "inherit",
    windowsHide: true
  });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with status ${String(result.status)}`);
  return captureOutput ? result.stdout : "";
}
