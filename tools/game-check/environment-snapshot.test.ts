import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import {
  HybridGameSessionBridge,
  environment_snapshot_for_identity_json,
  initSync
} from "../../web/pkg/birdman_game_wasm.js";
import { record } from "../shared/validation.js";
import { neutralTailInput } from "./current-session-fixture.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
const assetPath = fileURLToPath(new URL("../../assets/biwa-typical-july-environment-v6.json", import.meta.url));
const identity = Object.freeze({
  catalog_version: 3,
  scenario_id: 6,
  scenario_version: 3,
  aircraft_model_version: 2,
  environment_version: 6,
  controller_profile_version: 3,
  seed_low: 4_294_967_295,
  seed_high: 4_294_967_295
});

function parse(json: string): Record<string, unknown> {
  return record(JSON.parse(json) as unknown);
}

it("projects the exact immutable asset metadata through actual WASM without activating it", () => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const snapshot = parse(environment_snapshot_for_identity_json(JSON.stringify(identity)));
  const projection = record(snapshot.projection);
  const metadata = record(projection.metadata);
  expect(snapshot.schema_version).toBe(1);
  expect(snapshot.context).toEqual({ kind: "registry" });
  expect(projection).toMatchObject({ kind: "available", source: "registry", identity });
  expect(metadata.content_hash).toEqual({ kind: "asset_bytes", sha256: createHash("sha256").update(readFileSync(assetPath)).digest("hex") });
  expect(metadata.wind_domain).toEqual({ kind: "grid", minimum_ned_m: [-2000, -2000, -500], maximum_ned_m: [2000, 2000, 10] });
  expect(metadata.representative_position_ned_m).toEqual([0, 0, -10.5]);
  expect(metadata.representative_altitude_m).toBe(10.5);
  const wind = metadata.representative_velocity_ned_mps;
  if (!Array.isArray(wind)) throw new TypeError("Missing representative wind");
  expect(wind[0]).toBeCloseTo(-1.767766953, 12);
  expect(wind[1]).toBeCloseTo(1.767766953, 12);
  expect(wind[2]).toBe(0);
  expect(record(metadata.local_frame).kind).toBe("defined");
  expect(record(metadata.sky).kind).toBe("defined");
  expect(metadata.sources).toHaveLength(4);
  expect(metadata.ground_wind_normals).toHaveLength(1);
  expect(record(record(metadata.provenance).wind_grid).kind).toBe("assumed");
  expect(metadata).not.toHaveProperty("wind_grid");
  expect(metadata).not.toHaveProperty("velocities_ned_mps");
});

it("classifies JS envelope, bounded JSON and seed-word failures at the actual WASM boundary", () => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  for (const input of [null, undefined, 0, true, {}, []]) {
    expect(() => environment_snapshot_for_identity_json(input)).toThrow("InvalidInputType");
  }
  const duplicate = JSON.stringify(identity).replace("{", '{"catalog_version":3,');
  for (const input of ["null", "[]", "[1,1,1,1,1,1,0,0]", "{}", "{", duplicate, JSON.stringify({ ...identity, extra: 1 })]) {
    expect(() => environment_snapshot_for_identity_json(input)).toThrow("InvalidJson");
  }
  for (const field of ["seed_low", "seed_high"]) {
    for (const invalid of [-1, 4_294_967_296, 1.25, "1"]) {
      expect(() => environment_snapshot_for_identity_json(JSON.stringify({ ...identity, [field]: invalid }))).toThrow("InvalidJson");
    }
  }
  expect(() => environment_snapshot_for_identity_json(JSON.stringify({ ...identity, environment_version: 0 }))).toThrow("InvalidIdentity");
  expect(() => environment_snapshot_for_identity_json(" ".repeat(4097))).toThrow("InputTooLarge");
  const unknown = parse(environment_snapshot_for_identity_json(JSON.stringify({ ...identity, environment_version: 99 })));
  expect(unknown.projection).toEqual({ kind: "unavailable", source: "registry", identity: { ...identity, environment_version: 99 } });
});

it("requires the complete current registered aircraft and scenario identity", () => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  expect(parse(environment_snapshot_for_identity_json(JSON.stringify(identity))).projection).toMatchObject({ kind: "available", identity });
  for (const changed of [{ aircraft_model_version: 1 }, { aircraft_model_version: 3 }, { catalog_version: 2 }, { scenario_version: 1 }, { controller_profile_version: 4 }]) {
    const unknown = { ...identity, ...changed };
    expect(parse(environment_snapshot_for_identity_json(JSON.stringify(unknown))).projection).toEqual({
      kind: "unavailable", source: "registry", identity: unknown
    });
  }
});

it("uses selected, sealed, record and Attract identities without modifying the current flight", () => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const session = new HybridGameSessionBridge(0, 21, 22);
  try {
    expect(parse(session.environment_snapshot_json())).toMatchObject({ context: { kind: "session", phase_code: 0 }, projection: { kind: "no_selection" } });
    session.open_setup();
    session.set_weather_class(2);
    const selected = record(parse(session.environment_snapshot_json()).projection);
    expect(selected).toMatchObject({ kind: "available", source: "selected", identity: { catalog_version: 3, scenario_id: 6, environment_version: 6 } });
    expect(record(record(selected.metadata).sky).kind).toBe("defined");
    expect(record(record(selected.metadata).local_frame).kind).toBe("defined");
    expect(record(record(selected.metadata).waves).wind_velocity_ne_mps).toEqual([-1.767766953, 1.767766953]);
    expect(() => session.flight_record_summary_json()).toThrow();
    session.prepare();
    expect(record(parse(session.environment_snapshot_json()).projection)).toEqual({ ...selected, source: "sealed" });
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    const before = session.snapshot_json();
    session.environment_snapshot_json();
    expect(session.snapshot_json()).toEqual(before);
    session.abort();
    expect(record(parse(session.environment_snapshot_json()).projection).source).toBe("sealed");
    session.enter_replay();
    expect(record(parse(session.environment_snapshot_json()).projection)).toEqual({ ...selected, source: "record" });
    session.leave_replay();
    session.return_to_title();
    session.enter_attract();
    expect(parse(session.environment_snapshot_json()).projection).toMatchObject({ source: "attract", identity: { catalog_version: 3, scenario_id: 1, environment_version: 1 } });
  } finally {
    session.free();
  }
});

it("retains archived samples when environment metadata is unavailable", () => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const session = new HybridGameSessionBridge(0, 21, 22);
  const archive = new HybridGameSessionBridge(0, 21, 22);
  try {
    session.open_setup();
    session.set_weather_class(2);
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    session.advance_tick_json(neutralTailInput);
    session.abort();
    const document = parse(session.export_flight_record_json());
    const header = record(document.header);
    for (const version of [6, 99]) {
      archive.open_archived_flight_record(JSON.stringify({ ...document, header: { ...header, environment_version: version } }));
      const before = archive.flight_record_sample_at_seconds(0);
      expect(parse(archive.environment_snapshot_json()).projection).toMatchObject({ kind: version === 6 ? "available" : "unavailable", source: "archive", identity: { environment_version: version } });
      expect(archive.flight_record_sample_at_seconds(0)).toEqual(before);
      const currentSummary = record(parse(session.flight_record_summary_json()).summary);
      expect(record(parse(archive.flight_record_summary_json()).summary)).toEqual(currentSummary);
      expect(archive.phase_code()).toBe(9);
      expect(archive.is_archived_replay()).toBe(true);
      archive.leave_replay();
    }
  } finally {
    archive.free();
    session.free();
  }
});
