import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, it } from "vitest";
import { readyApplicationFixture } from "./application-fixture.js";
import { record } from "../shared/validation.js";
import { parseCargoWasmBuildOutput, selectWasmBuildArtifact } from "../wasm-build/cargo-build-output.js";

it("runs the same real controller bytes and accepted flight to termination on Native and Web", () => {
  const root = resolve(import.meta.dirname, "../..");
  const cargoEnv = { ...process.env, CARGO_TARGET_DIR: resolve(process.env["CARGO_TARGET_DIR"] ?? resolve(root, "target")) };
  const run = (args: string[], env: NodeJS.ProcessEnv = cargoEnv): string => {
    const result = spawnSync("cargo", args, { cwd: root, encoding: "utf8", windowsHide: true,
      env, maxBuffer: 16 * 1024 * 1024 });
    if (result.error !== undefined) throw result.error;
    expect(result.status, result.stderr).toBe(0);
    return result.stdout;
  };
  const manifest = resolve(root, "tools/controller-sample-guest/Cargo.toml");
  run(["fmt", "--manifest-path", manifest, "--", "--check"]);
  run(["clippy", "--manifest-path", manifest, "--all-targets", "--locked", "--", "-D", "warnings"]);
  const stdio = run(["test", "--manifest-path", manifest, "--test", "stdio", "--release", "--locked"]);
  expect(stdio).toMatch(/test result: ok\. 1 passed; 0 failed;/u);
  const output = parseCargoWasmBuildOutput(run(["build", "--manifest-path", manifest,
    "--config", resolve(root, "tools/controller-sample-guest/.cargo/config.toml"), "--lib",
    "--release", "--target", "wasm32-unknown-unknown", "--locked", "--message-format=json"],
    cargoEnv),
  manifest, "controller_sample_guest");
  const artifact = selectWasmBuildArtifact(output, 0);
  const referencePath = `${artifact}.native-record.json`;
  const bytes = readFileSync(artifact);
  rmSync(referencePath, { force: true });
  const native = spawnSync("cargo", ["test", "-p", "birdman-game-app", "--lib", "--locked",
    "controller::execution::guest_acceptance::actual_sample_guest_runs_under_production_wasmi_limits",
    "--", "--ignored", "--exact"], {
    cwd: root, encoding: "utf8", windowsHide: true, maxBuffer: 16 * 1024 * 1024,
    env: { ...cargoEnv, BIRDMAN_CONTROLLER_GUEST_PATH: artifact, BIRDMAN_CONTROLLER_REFERENCE_PATH: referencePath }
  });
  if (native.error !== undefined) throw native.error;
  expect(native.status, native.stderr).toBe(0);
  expect(native.stdout).toMatch(/test result: ok\. 1 passed; 0 failed; 0 ignored;/u);
  const runner = resolve(cargoEnv.CARGO_TARGET_DIR, "release",
    `controller-sample-stdio${process.platform === "win32" ? ".exe" : ""}`);
  const sitl = run(["test", "-p", "birdman-game-bevy", "--bin", "birdman-game-bevy",
    "--features", "verify-harness,openxr", "--locked",
    "app::native_controller::tests::actual_stdio_sample_matches_reference_through_natural_contact",
    "--", "--ignored", "--exact"], { ...cargoEnv,
    BIRDMAN_CONTROLLER_SITL_PROGRAM: runner, BIRDMAN_CONTROLLER_REFERENCE_PATH: referencePath });
  expect(sitl).toMatch(/test result: ok\. 1 passed; 0 failed; 0 ignored;/u);
  const referenceBytes = readFileSync(referencePath);
  const reference = record(JSON.parse(referenceBytes.toString("utf8")) as unknown);
  const execution = record(record(reference["control_identity"])["execution"]);
  expect(record(execution["artifact"])["sha256"]).toBe(createHash("sha256").update(bytes).digest("hex"));
  const fixture = readyApplicationFixture();
  try {
    fixture.activate({ kind: "session", command: { kind: "open_setup" } });
    const picked = fixture.activate({ kind: "platform", command: {
      kind: "load_configuration", document: "controller_execution"
    } }).requests.find(request => request.kind === "pick_configuration");
    if (picked?.kind !== "pick_configuration") throw new Error("Expected the shared execution file request");
    const loaded = fixture.observe({ kind: "configuration_loaded", document: "controller_execution",
      request_id: picked.request_id, completion: { kind: "found", bytes: Array.from(bytes) } });
    const save = loaded.requests.find(request => request.kind === "write_configuration");
    if (save?.kind !== "write_configuration") throw new Error("Expected atomic source persistence");
    fixture.observe({ kind: "configuration_saved", document: "controller_execution", request_id: save.request_id, failure: null });
    for (const command of [{ kind: "select_weather", weather: "calm" }, { kind: "prepare" }, { kind: "start_countdown" }] as const) {
      fixture.activate({ kind: "session", command });
    }
    let observedMs = 0;
    for (let now = 0; now <= 43_020 && fixture.view.scene !== "Result"; now += 10) {
      observedMs = now;
      fixture.observe({ kind: "frame", timestamp_ms: now, gamepads: [], display_resource_id: null,
        xr: null, spatial: { gamepads: [], sources: [], presented_id: null } }, now);
    }
    expect(fixture.view.scene).toBe("Result");
    fixture.activate({ kind: "toggle_technical" }, observedMs);
    const download = fixture.activate({ kind: "platform", command: { kind: "download_record_json" } }, observedMs)
      .requests.find(request => request.kind === "download_file");
    if (download?.kind !== "download_file") throw new Error("Expected the shared finalized JSON export");
    const actual = record(JSON.parse(new TextDecoder().decode(new Uint8Array(download.bytes))) as unknown);
    expect(actual["control_identity"]).toEqual(reference["control_identity"]);
    const compare = (actual: unknown, expected: unknown): void => {
      if (typeof expected === "number") {
        expect(typeof actual === "number" && Number.isFinite(actual) && Number.isFinite(expected)).toBe(true);
        const observed = actual as number;
        expect(Math.abs(observed - expected)).toBeLessThanOrEqual(1e-9 + 1e-10 * Math.max(Math.abs(observed), Math.abs(expected)));
      }
      else if (Array.isArray(expected)) {
        expect(Array.isArray(actual)).toBe(true);
        const values = actual as unknown[];
        expect(values).toHaveLength(expected.length);
        expected.forEach((value: unknown, index) => { compare(values[index], value); });
      } else if (expected !== null && typeof expected === "object") {
        const values = record(actual);
        const expectedFields = record(expected);
        expect(Object.keys(values).sort()).toEqual(Object.keys(expectedFields).sort());
        for (const [key, value] of Object.entries(expectedFields)) compare(values[key], value);
      } else expect(actual).toEqual(expected);
    };
    compare(actual["samples"], reference["samples"]);
    compare(actual["finalization"], reference["finalization"]);
    writeFileSync(`${artifact}.acceptance.json`, JSON.stringify({
      artifact, reference: referencePath, runner,
      runnerSha256: createHash("sha256").update(readFileSync(runner)).digest("hex"),
      artifactSha256: createHash("sha256").update(bytes).digest("hex"),
      referenceSha256: createHash("sha256").update(referenceBytes).digest("hex"),
      mainWasmSha256: createHash("sha256").update(readFileSync(resolve(root, "web/pkg/birdman_game_wasm_bg.wasm"))).digest("hex")
    }) + "\n", "utf8");
  } finally { fixture.owner.free(); }
}, 600_000);
