import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
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

function run(command: string, arguments_: string[], log: string, environment = process.env): string {
  const text = execFileSync(command, arguments_, { ...gitOptions, env: environment, stdio: ["ignore", "pipe", "pipe"] });
  execLogs.push(writeFile(resolve(output, log), text, "utf8"));
  return text;
}

const execLogs: Promise<void>[] = [];
const nativeLog = run("cargo", ["test", "-p", "birdman-game-bevy", "--locked",
  "native_acceptance_", "--", "--nocapture"],
"native.log", { ...process.env, BPG_NATIVE_ACCEPTANCE_DIR: native });
assert.match(nativeLog, /^test result: ok\. 1 passed; 0 failed;/m);
run(process.execPath, [resolve(root, "node_modules/tsx/dist/cli.mjs"), "tools/wasm-build/main.ts"], "wasm-build.log");
await Promise.all(execLogs);
const wasmBytes = await readFile(resolve(root, "web/pkg/birdman_game_wasm_bg.wasm"));
const { HybridGameSessionBridge, initSync, physics_hz } = await import("../../web/pkg/birdman_game_wasm.js");
const { createAppSession } = await import("../../web/src/app/session-factory.js");
const { TailAppSessionFacade } = await import("../../web/src/app/session-facade.js");
initSync({ module: new Uint8Array(wasmBytes) });
const cases: Record<string, unknown>[] = [];
let maximumScaledError = 0;
let maximumAbsoluteError = 0;
const absoluteTolerance = 1.0e-9;
const relativeTolerance = 1.0e-10;

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
    assert.deepEqual(Object.keys(actualFields).sort(), Object.keys(expectedFields).sort(), path);
    for (const key of Object.keys(expectedFields)) compare(actualFields[key], expectedFields[key], `${path}.${key}`);
  } else assert.equal(actual, expected, path);
}

const artifactFiles = (await readdir(native)).filter(file => file.endsWith(".json")).sort();
const frameFiles = artifactFiles.filter(file => file.startsWith("frame-"));
assert.equal(frameFiles.length, 4);
for (const file of frameFiles) {
  const artifact = record(JSON.parse(await readFile(resolve(native, file), "utf8")) as unknown);
  assert.ok(typeof artifact.record_json === "string" && Array.isArray(artifact.observations));
  const document = record(JSON.parse(artifact.record_json) as unknown);
  const header = record(document.header);
  assert.ok(typeof header.seed === "number");
  const seed = BigInt(header.seed);
  const bridge = new HybridGameSessionBridge(0, Number(seed & 0xffff_ffffn), Number(seed >> 32n));
  const session = new TailAppSessionFacade(bridge, physics_hz());
  try {
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "open_setup" }));
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "select_assistance", assistance: artifact.assistance }));
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "select_weather", weather: record(header.difficulty).weather }));
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "prepare" }));
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "start_countdown" }));
    bridge.observe_countdown(bridge.countdown_token(), 3, true);
    for (const value of artifact.observations) {
      const observation = record(value);
      assert.ok(Array.isArray(observation.commands) && Array.isArray(observation.keys));
      assert.ok(typeof observation.timestamp_ms === "number");
      for (const command of observation.commands) bridge.apply_game_ui_session_command_json(JSON.stringify(command));
      for (const key of observation.keys) bridge.observe_flight_key_json(JSON.stringify(key));
      const frame = record(JSON.parse(bridge.advance_flight_frame_json(observation.timestamp_ms, JSON.stringify(observation.gamepads), true)) as unknown);
      compare(frame.snapshot, observation.snapshot, `${file}.snapshot@${String(observation.timestamp_ms)}`);
      compare(frame.render_pose, observation.render_pose, `${file}.pose@${String(observation.timestamp_ms)}`);
    }
    const actual = record(JSON.parse(bridge.export_current_flight_record_json()) as unknown);
    assert.deepEqual(actual.header, document.header);
    assert.deepEqual(actual.control_identity, document.control_identity);
    const nativeMetadata = record(document.identity_metadata), wasmMetadata = record(actual.identity_metadata);
    assert.deepEqual(wasmMetadata.conditions, nativeMetadata.conditions);
    assert.equal(wasmMetadata.comparison_version, nativeMetadata.comparison_version);
    assert.deepEqual(record(wasmMetadata.build).source_fingerprints, record(nativeMetadata.build).source_fingerprints);
    const archive = createAppSession();
    try {
      archive.openArchive(artifact.record_json);
      assert.equal(archive.readFlightLog("json"), artifact.record_json);
      assert.equal(archive.readFlightLog("csv"), artifact.csv);
      bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "enter_replay" }));
      assert.ok(Array.isArray(artifact.replay));
      for (const value of artifact.replay) {
        const projection = record(value);
        assert.ok(typeof projection.time === "number");
        compare(archive.queryRecordSample(projection.time), projection.sample, file + ".replay.sample");
        compare(JSON.parse(bridge.playback_camera_view_json(projection.time, "chase", true)) as unknown, projection.camera, file + ".replay.camera");
        compare(archive.queryReplayCamera(projection.time, "chase", true), session.queryReplayCamera(projection.time, "chase", true), file + ".archive.camera");
      }
    } finally { archive.dispose(); }
    compare(actual.samples, document.samples, `${file}.samples`);
    compare(actual.finalization, document.finalization, `${file}.finalization`);
    cases.push({ file, frames: artifact.observations.length, samples: Array.isArray(document.samples) ? document.samples.length : 0 });
  } finally { session.dispose(); }
}
assert.equal(execFileSync("git", ["diff", "--binary", "HEAD"], gitOptions), patch);
assert.equal(execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim(), head);
assert.equal(execFileSync("git", ["ls-files", "--others", "--exclude-standard"], gitOptions), "");
const result = { status: "passed", sourceHead: head, sourceDiffSha256: patch.length === 0 ? null : createHash("sha256").update(patch).digest("hex"),
  wasmSha256: createHash("sha256").update(wasmBytes).digest("hex"), absoluteTolerance, relativeTolerance,
  maximumAbsoluteError, maximumScaledError, cases };
await writeFile(resolve(output, "result.json"), `${JSON.stringify(result, null, 2)}\n`, "utf8");
console.log(JSON.stringify(result, null, 2));
