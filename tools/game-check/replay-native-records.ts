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
assert.match(nativeLog, /^test result: ok\. 2 passed; 0 failed;/m);
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
const files = artifactFiles.filter(file => !file.startsWith("frame-"));
assert.equal(files.length, 12, "Expected four assistance levels and three longitudinal inputs");
const covered = new Set<string>();
for (const file of files) {
  const artifact = record(JSON.parse(await readFile(resolve(native, file), "utf8")) as unknown);
  assert.ok(typeof artifact.record_json === "string" && typeof artifact.csv === "string");
  const document = record(JSON.parse(artifact.record_json) as unknown);
  const header = record(document.header);
  const difficulty = record(header.difficulty);
  const demand = record(artifact.input);
  const assistance = difficulty.assistance;
  assert.ok(assistance === "manual" || assistance === "assisted" || assistance === "light" || assistance === "strong");
  const controlModeCode = assistance === "manual" ? 0 : assistance === "strong" ? 2 : 1;
  assert.ok(typeof header.seed === "number" && Number.isSafeInteger(header.seed));
  const seed = BigInt(header.seed);
  const bridge = new HybridGameSessionBridge(controlModeCode, Number(seed & 0xffff_ffffn), Number(seed >> 32n));
  const session = new TailAppSessionFacade(bridge, physics_hz());
  const archive = createAppSession();
  try {
    session.executeSharedSessionCommand({ kind: "open_setup" });
    session.executeSharedSessionCommand({ kind: "select_assistance", assistance });
    const weather = difficulty.weather;
    assert.ok(weather === "calm" || weather === "mild" || weather === "typical" || weather === "challenging" || weather === "near_limit");
    session.executeSharedSessionCommand({ kind: "select_weather", weather });
    session.executeSharedSessionCommand({ kind: "prepare" });
    session.executeSharedSessionCommand({ kind: "start_countdown" });
    session.observeCountdown(session.captureCountdownToken(), 3, true);
    assert.ok(Array.isArray(document.samples));
    const samples = document.samples.map((sample: unknown) => record(sample));
    assert.ok(samples.length > 1);
    assert.ok(typeof demand.nose_up === "number" && typeof demand.turn_right === "number"
      && typeof demand.desired_pitch_rate_rad_s === "number" && typeof demand.desired_yaw_rate_rad_s === "number");
    assert.ok([-1, 0, 1].includes(demand.nose_up));
    assert.deepEqual(demand.pilot_position_command, { kind: "hold" });
    const input = { controlLayout: "tail_incidence" as const, noseUp: demand.nose_up, turnRight: demand.turn_right,
      desiredPitchRateRadiansPerSecond: demand.desired_pitch_rate_rad_s, desiredYawRateRadiansPerSecond: demand.desired_yaw_rate_rad_s,
      pilotPositionCommand: { kind: "hold" as const } };
    assert.ok(typeof header.maximum_flight_ticks === "number");
    let attempts = 0;
    while (session.readLifecycle().phase === "flight_running" && attempts <= header.maximum_flight_ticks) {
      try { session.flightPort.advanceTick(input); }
      catch (cause) {
        if (session.readLifecycle().phase !== "result" || record(document.finalization).failure === null) throw cause;
      }
      attempts += 1;
    }
    assert.equal(session.readLifecycle().phase, "result", file);
    const wasmJson = session.exportRecordJson();
    const actual = record(JSON.parse(wasmJson) as unknown);
    assert.equal(actual.schema_version, 7);
    assert.deepEqual(actual.header, header, `${file}: identity and comparison key`);
    assert.deepEqual(actual.control_identity, document.control_identity);
    compare(actual.samples, document.samples, `${file}.samples`);
    compare(actual.finalization, document.finalization, `${file}.finalization`);
    const nativeMetadata = record(document.identity_metadata);
    const wasmMetadata = record(actual.identity_metadata);
    assert.deepEqual(wasmMetadata.conditions, nativeMetadata.conditions);
    assert.equal(wasmMetadata.comparison_version, nativeMetadata.comparison_version);
    assert.deepEqual(record(wasmMetadata.build).source_fingerprints, record(nativeMetadata.build).source_fingerprints);
    session.executeSharedSessionCommand({ kind: "enter_replay" });
    archive.openArchive(artifact.record_json);
    assert.equal(archive.readFlightLog("json"), artifact.record_json);
    assert.equal(archive.readFlightLog("csv"), artifact.csv);
    assert.ok(Array.isArray(artifact.replay) && artifact.replay.length === 5);
    for (const value of artifact.replay) {
      const projection = record(value);
      const time = projection.time;
      assert.ok(typeof time === "number");
      compare(session.queryRecordSample(time), projection.sample, `${file}.nativeQuery@${String(time)}`);
      compare(JSON.parse(bridge.playback_camera_view_json(time, "chase", true)) as unknown, projection.camera, `${file}.nativeCamera@${String(time)}`);
      compare(session.queryRecordSample(time), archive.queryRecordSample(time), `${file}.archiveQuery@${String(time)}`);
      compare(session.queryReplayCamera(time, "chase", true), archive.queryReplayCamera(time, "chase", true), `${file}.archiveCamera@${String(time)}`);
    }
    assert.equal(archive.readFlightLog("json"), artifact.record_json);
    await writeFile(resolve(output, file), wasmJson, "utf8");
    covered.add(`${String(difficulty.assistance)}:${String(demand.nose_up)}`);
    cases.push({ file, attempts, samples: samples.length, finalization: document.finalization, nativeRecordSha256: createHash("sha256").update(artifact.record_json).digest("hex"),
      wasmRecordSha256: createHash("sha256").update(wasmJson).digest("hex") });
  } finally { archive.dispose(); session.dispose(); }
}
assert.equal(covered.size, 12);
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
  try {
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "open_setup" }));
    bridge.apply_game_ui_session_command_json(JSON.stringify({ kind: "select_assistance", assistance: artifact.assistance }));
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
    compare(actual.samples, document.samples, `${file}.samples`);
    compare(actual.finalization, document.finalization, `${file}.finalization`);
    cases.push({ file, frames: artifact.observations.length, samples: Array.isArray(document.samples) ? document.samples.length : 0 });
  } finally { bridge.free(); }
}
assert.equal(execFileSync("git", ["diff", "--binary", "HEAD"], gitOptions), patch);
assert.equal(execFileSync("git", ["rev-parse", "HEAD"], gitOptions).trim(), head);
assert.equal(execFileSync("git", ["ls-files", "--others", "--exclude-standard"], gitOptions), "");
const result = { status: "passed", sourceHead: head, sourceDiffSha256: patch.length === 0 ? null : createHash("sha256").update(patch).digest("hex"),
  wasmSha256: createHash("sha256").update(wasmBytes).digest("hex"), absoluteTolerance, relativeTolerance,
  maximumAbsoluteError, maximumScaledError, cases };
await writeFile(resolve(output, "result.json"), `${JSON.stringify(result, null, 2)}\n`, "utf8");
console.log(JSON.stringify(result, null, 2));
