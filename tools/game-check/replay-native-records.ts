import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import type { ApplicationDrawingOutput, ApplicationOutput } from "../../web/src/generated/application-contracts.js";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { record } from "../shared/validation.js";

const [destination, option] = process.argv.slice(2);
assert.ok(destination !== undefined && (option === undefined || option === "--allow-dirty"),
  "Usage: npm run test:cross-target -- OUTPUT_DIRECTORY [--allow-dirty]");
const root = resolve(import.meta.dirname, "../..");
const output = resolve(destination);
const native = resolve(output, "native");
const gitOptions = { cwd: root, encoding: "utf8" as const, maxBuffer: 16 * 1024 * 1024, windowsHide: true };
const head = execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim();
const patch = execFileSync("git", ["diff", "--binary", "HEAD"], gitOptions);
assert.equal(execFileSync("git", ["ls-files", "--others", "--exclude-standard"], gitOptions), "");
assert.ok(option === "--allow-dirty" || patch.length === 0, "Commit source changes or use --allow-dirty");
await mkdir(output, { recursive: true });
await writeFile(resolve(output, "result.json"), JSON.stringify({ status: "failed", sourceHead: head }), "utf8");
await mkdir(native);
if (patch.length > 0) await writeFile(resolve(output, "source.patch"), patch, "utf8");

function run(command: string, arguments_: string[], log: string): void {
  const result = spawnSync(command, arguments_, { ...gitOptions, stdio: ["ignore", "pipe", "pipe"], maxBuffer: 64 * 1024 * 1024 });
  execLogs.push(writeFile(resolve(output, log), result.stdout + result.stderr, "utf8"));
  if (result.error !== undefined) throw result.error;
  assert.equal(result.status, 0, result.stderr);
}

const execLogs: Promise<void>[] = [];
run(process.execPath, [resolve(root, "node_modules/tsx/dist/cli.mjs"), "tools/wasm-build/main.ts"], "wasm-build.log");
run("cargo", ["run", "-p", "birdman-game-bevy", "--locked", "--features", "verify-harness", "--",
  "--offscreen", "--verify", native, "--verify-adapter-input", "--verify-size", "1280x720", "--export-dir", resolve(native, "records")], "native.log");
const rustPatch = execFileSync("git", ["diff", "--binary", "HEAD", "--", "crates"], gitOptions);
await writeFile(resolve(native, "source-evidence.json"), JSON.stringify({ sourceHead: head,
  rustDiffSha256: createHash("sha256").update(rustPatch).digest("hex"), nativeExit: 0 }, null, 2) + "\n", "utf8");
await Promise.all(execLogs);
const wasmBytes = await readFile(resolve(root, "web/pkg/birdman_game_wasm_bg.wasm"));
const { ApplicationRuntimeBridge, initSync } = await import("../../web/pkg/birdman_game_wasm.js");
initSync({ module: new Uint8Array(wasmBytes) });
let maximumScaledError = 0;
let maximumAbsoluteError = 0;
const absoluteTolerance = 1.0e-9;
const relativeTolerance = 1.0e-10;
const buildObservations: Record<string, unknown>[] = [];

function newRecordBody(json: string, source: "native" | "wasm"): unknown {
  const document = record(JSON.parse(json) as unknown);
  const identity = record(document["identity_metadata"]);
  const { target, profile, ...sharedBuild } = record(identity["build"]);
  assert.ok(typeof target === "string");
  assert.equal(profile, source === "wasm" ? "release" : "debug");
  if (source === "wasm") assert.equal(target, "wasm32-unknown-unknown");
  else assert.notEqual(target, "wasm32-unknown-unknown");
  buildObservations.push({ source, target, profile });
  return { ...document, identity_metadata: { ...identity, build: sharedBuild } };
}

function compare(actual: unknown, expected: unknown, path: string): void {
  if (typeof expected === "number") {
    assert.ok(typeof actual === "number" && Number.isFinite(actual) && Number.isFinite(expected), path);
    const error = Math.abs(actual - expected);
    const tolerance = absoluteTolerance + relativeTolerance * Math.max(Math.abs(expected), Math.abs(actual));
    maximumAbsoluteError = Math.max(maximumAbsoluteError, error);
    maximumScaledError = Math.max(maximumScaledError, error / tolerance);
    assert.ok(error <= tolerance, `${path}: ${String(actual)} vs ${String(expected)}, tolerance ${String(tolerance)}`);
  } else if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), path);
    assert.equal(actual.length, expected.length, path);
    expected.forEach((value: unknown, index) => { compare(actual[index], value, `${path}[${String(index)}]`); });
  } else if (expected !== null && typeof expected === "object") {
    const expectedFields = record(expected);
    const actualFields = record(actual);
    if (expectedFields["kind"] === "insert_record" && typeof expectedFields["json"] === "string") {
      assert.ok(typeof actualFields["json"] === "string");
      compare({ ...actualFields, json: newRecordBody(actualFields["json"], "wasm") },
        { ...expectedFields, json: newRecordBody(expectedFields["json"], "native") }, path);
      return;
    }
    assert.deepEqual(Object.keys(actualFields).sort(), Object.keys(expectedFields).sort(), path);
    for (const key of Object.keys(expectedFields)) compare(actualFields[key], expectedFields[key], `${path}.${key}`);
  } else assert.equal(actual, expected, path);
}

let bridge: InstanceType<typeof ApplicationRuntimeBridge> | undefined;
let drawing: ApplicationDrawingOutput | undefined;
let pendingCapture: { serial: unknown; file: unknown; drawing: ApplicationDrawingOutput | undefined } | undefined;
let observations = 0, captures = 0, downloads = 0;
let loadedRecords = 0;
let lifecycle = "running";
const captureFiles: string[] = [];
try {
  const lines = createInterface({ input: createReadStream(resolve(native, "application-trace.jsonl")), crlfDelay: Infinity });
  for await (const line of lines) {
    const entry = record(JSON.parse(line) as unknown);
    if ("platform" in entry) {
      assert.equal(bridge, undefined);
      bridge = new ApplicationRuntimeBridge(JSON.stringify(entry["platform"]));
    } else if ("observation" in entry) {
      assert.ok(bridge !== undefined);
      assert.ok(Array.isArray(entry["fonts"]));
      const fonts = entry["fonts"];
      if (record(record(entry["observation"])["input"])["kind"] === "record_loaded") loadedRecords++;
      let index = 0;
      let actual: ApplicationOutput | undefined;
      let failure: string | undefined;
      let callbackFailure: Error | undefined;
      try {
        actual = JSON.parse(bridge.update(JSON.stringify(entry["observation"]), (probeJson) => {
          const font = record(fonts[index++]);
          let result: Record<string, unknown>;
          try {
            compare(JSON.parse(probeJson) as unknown, font["probe"], "font.probe");
            result = record(font["result"]);
          } catch (cause) {
            callbackFailure = cause instanceof Error ? cause : new Error("Font callback comparison failed", { cause });
            throw callbackFailure;
          }
          if ("Err" in result) {
            if (result["Err"] === "measurement-unavailable") throw new Error("Recorded font measurement unavailable");
            assert.equal(result["Err"], "invalid-text-metrics");
            return new Float64Array();
          }
          const metrics = record(result["Ok"]);
          return Float64Array.from(["advance_px", "left_px", "right_px", "ascent_px", "descent_px"].map((key) => {
            const value = metrics[key];
            assert.ok(value === null || typeof value === "number");
            return value === null ? Number.NaN : value;
          }));
        })) as ApplicationOutput;
      } catch (cause) { failure = String(cause); }
      if (callbackFailure !== undefined) throw callbackFailure;
      assert.equal(index, fonts.length);
      if ("error" in entry) {
        assert.equal(failure, entry["error"]);
      } else {
        assert.equal(failure, undefined);
        assert.ok(actual !== undefined);
        compare(actual.requests, entry["requests"], "requests@" + String(observations));
        assert.equal(actual.lifecycle, entry["lifecycle"]);
        assert.equal(actual.keyboard_consumed, entry["keyboard_consumed"]);
        for (const request of actual.requests) if (request.kind === "download_file") downloads++;
        if (actual.drawing !== null && actual.drawing !== undefined) drawing = actual.drawing;
        lifecycle = actual.lifecycle;
      }
      observations++;
    } else if ("capture_started" in entry) {
      pendingCapture = { serial: entry["capture_started"], file: entry["file"], drawing };
    } else {
      assert.ok("capture" in entry && typeof entry["file"] === "string");
      assert.ok(pendingCapture !== undefined);
      assert.equal(pendingCapture.serial, entry["serial"]);
      assert.equal(pendingCapture.file, entry["file"]);
      compare(pendingCapture.drawing, entry["capture"], entry["file"]);
      pendingCapture = undefined;
      captureFiles.push(entry["file"]);
      captures++;
    }
  }
  assert.ok(observations > 0 && captures > 0);
  assert.ok(captureFiles.includes("10-result.png") && captureFiles.includes("12-replay.png") && captureFiles.includes("17-stored-replay.png"));
  assert.ok(loadedRecords > 0 && buildObservations.length > 0);
  assert.equal(downloads, 2, "Original JSON and CSV downloads are compared byte-for-byte");
  assert.equal(lifecycle, "drained");
} finally {
  bridge?.free();
}
assert.equal(execFileSync("git", ["diff", "--binary", "HEAD"], gitOptions), patch);
assert.equal(execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim(), head);
assert.equal(execFileSync("git", ["ls-files", "--others", "--exclude-standard"], gitOptions), "");
const result = { status: "passed", sourceHead: head, sourceDiffSha256: patch.length === 0 ? null : createHash("sha256").update(patch).digest("hex"),
  wasmSha256: createHash("sha256").update(wasmBytes).digest("hex"), absoluteTolerance, relativeTolerance,
  maximumAbsoluteError, maximumScaledError, observations, captures, downloads, loadedRecords, captureFiles, buildObservations };
await writeFile(resolve(output, "result.json"), JSON.stringify(result, null, 2) + "\n", "utf8");
console.log(JSON.stringify(result, null, 2));
