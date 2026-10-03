import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { beforeAll, expect, it } from "vitest";
import {
  GameSessionBridge,
  compare_personal_best_json,
  initSync
} from "../../web/pkg/birdman_game_wasm.js";
import { record } from "../shared/validation.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
const demoMetadata = Object.freeze([
  4, 2, 3, 0,
  1, 1, 1, 1, 1, 1,
  0xD3A0, 0,
  1, 0, 0, 0, 0, 0
]);

beforeAll(() => {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
});

function selectedMetadata(session: GameSessionBridge) {
  return {
    preset: session.difficulty_preset_code(),
    information: session.information_level_code(),
    assistance: session.assistance_level_code(),
    weather: session.weather_class_code(),
    cues: Array.from(session.information_profile_codes())
  };
}

function expectPackedSamplesEqual(actual: Float64Array, expected: Float64Array): void {
  expect(actual.constructor).toBe(expected.constructor);
  expect(actual.length).toBe(expected.length);
  const mismatch = actual.findIndex((value, index) => !Object.is(value, expected[index]));
  expect(mismatch).toBe(-1);
}

function startFlight(session: GameSessionBridge): void {
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
}

it("compares packed values with SameValue semantics for NaN payloads and bounded views", () => {
  const firstPayload = new Float64Array(new BigUint64Array([0x7ff8000000000001n]).buffer);
  const secondPayload = new Float64Array(new BigUint64Array([0x7ff8000000000002n]).buffer);
  expect(firstPayload).toEqual(secondPayload);
  expectPackedSamplesEqual(firstPayload, secondPayload);
  expectPackedSamplesEqual(
    new Float64Array([NaN, Infinity, -Infinity, -0]),
    new Float64Array([NaN, Infinity, -Infinity, -0])
  );
  expectPackedSamplesEqual(
    new Float64Array([99, 1, 2, 99]).subarray(1, 3),
    new Float64Array([1, 2])
  );
});

it("rejects changed packed positions, lengths, constructors and distinct numeric values", () => {
  const original = new Float64Array([1, 2, 3, 4, 5]);
  for (const index of [0, 2, 4]) {
    const changed = original.slice();
    changed[index] = 99;
    expect(() => {
      expectPackedSamplesEqual(changed, original);
    }).toThrow();
  }
  const shorter = original.subarray(0, original.length - 1);
  expect(() => {
    expectPackedSamplesEqual(shorter, original);
  }).toThrow();
  expect(() => {
    expectPackedSamplesEqual(original, shorter);
  }).toThrow();
  class OtherFloat64Array extends Float64Array {}
  expect(() => {
    expectPackedSamplesEqual(new OtherFloat64Array([1]), new Float64Array([1]));
  }).toThrow();
  for (const [actual, expected] of [[0, -0], [NaN, 0], [Infinity, -Infinity]] as const) {
    expect(() => {
      expectPackedSamplesEqual(new Float64Array([actual]), new Float64Array([expected]));
    }).toThrow();
  }
});

it("reports the declared Custom/Minimal demo policy instead of Automatic/NearLimit player settings", () => {
  const session = new GameSessionBridge(2);
  try {
    session.open_setup();
    session.set_weather_class(4);
    session.return_to_title();
    session.enter_attract();
    expect(Array.from(session.configuration_metadata())).toEqual(demoMetadata);
    const projection = record(record(JSON.parse(session.environment_snapshot_json()) as unknown).projection);
    expect(projection.source).toBe("attract");
    expect(projection.identity).toEqual({
      catalog_version: 1, scenario_id: 1, scenario_version: 1,
      aircraft_model_version: 1, environment_version: 1,
      controller_profile_version: 1, seed_low: 0xD3A0, seed_high: 0
    });
    expect(record(projection.metadata).representative_velocity_ned_mps).toEqual([0, 0, 0]);
    expect(() => session.export_flight_record_json()).toThrow("flight record is unavailable");
  } finally {
    session.free();
  }
});

it.each([0, 1, 2, 3, 4])("keeps player Information %s and all six cues independent across repeated demo queries", (information) => {
  const session = new GameSessionBridge(2);
  try {
    session.open_setup();
    session.set_information_level(information);
    session.set_assistance_level(information % 4);
    session.set_weather_class(4);
    if (information === 4) {
      [true, false, true, false, true, false].forEach((visible, cue) => {
        session.set_information_cue(cue, visible);
      });
    }
    const selected = selectedMetadata(session);
    session.return_to_title();
    let firstDemoSamples: Float64Array | null = null;
    for (let entry = 0; entry < 2; entry++) {
      session.enter_attract();
      session.seek_playback(0.75);
      const clock = session.playback_clock_state();
      const snapshot = session.snapshot();
      const samples = session.flight_record_samples_packed();
      const finalization = session.flight_record_finalization();
      if (firstDemoSamples === null) firstDemoSamples = samples;
      else expectPackedSamplesEqual(samples, firstDemoSamples);
      for (let query = 0; query < 3; query++) {
        expect(Array.from(session.configuration_metadata())).toEqual(demoMetadata);
        expect(selectedMetadata(session)).toEqual(selected);
        expect(session.playback_clock_state()).toEqual(clock);
        expect(session.snapshot()).toEqual(snapshot);
        expectPackedSamplesEqual(session.flight_record_samples_packed(), samples);
        expect(session.flight_record_finalization()).toEqual(finalization);
        expect(session.phase_code()).toBe(10);
      }
      session.leave_attract();
      expect(session.phase_code()).toBe(0);
      expect(selectedMetadata(session)).toEqual(selected);
      expect(session.flight_record_sample_count()).toBe(0);
    }
    session.open_setup();
    expect(selectedMetadata(session)).toEqual(selected);
    session.prepare();
    const metadata = Array.from(session.configuration_metadata());
    expect(metadata.slice(0, 4)).toEqual([selected.preset, selected.information, selected.assistance, selected.weather]);
    expect(metadata.slice(12)).toEqual(selected.cues);
  } finally {
    session.free();
  }
});

it.each([0, 1, 2, 3])("does not replace player named preset %s with the demo declaration", (preset) => {
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.set_difficulty_preset(preset);
    const selected = selectedMetadata(session);
    session.return_to_title();
    session.enter_attract();
    expect(Array.from(session.configuration_metadata())).toEqual(demoMetadata);
    expect(selectedMetadata(session)).toEqual(selected);
    session.leave_attract();
    session.open_setup();
    expect(selectedMetadata(session)).toEqual(selected);
  } finally {
    session.free();
  }
});

it("preserves non-Attract metadata, public phase errors, export and PB queries", () => {
  const session = new GameSessionBridge(0);
  try {
    expect(() => session.configuration_metadata()).toThrow("resolved configuration is unavailable before Briefing");
    session.open_setup();
    expect(() => session.configuration_metadata()).toThrow("resolved configuration is unavailable before Briefing");
    session.set_information_cue(2, false);
    session.prepare();
    const metadata = Array.from(session.configuration_metadata());
    expect(metadata.slice(0, 4)).toEqual([4, 4, 3, 0]);
    expect(metadata.slice(12)).toEqual([1, 1, 0, 1, 1, 1]);
    const stableQuery = () => {
      const phase = session.phase_code();
      const snapshot = session.snapshot();
      const samples = session.flight_record_sample_count();
      expect(Array.from(session.configuration_metadata())).toEqual(metadata);
      expect(session.phase_code()).toBe(phase);
      expect(session.snapshot()).toEqual(snapshot);
      expect(session.flight_record_sample_count()).toBe(samples);
    };
    stableQuery();
    session.fail_briefing(0);
    expect(session.phase_code()).toBe(8);
    stableQuery();
    session.retry_briefing();
    stableQuery();
    session.mark_briefing_ready();
    stableQuery();
    session.start_countdown(1);
    stableQuery();
    session.advance_countdown();
    session.launch();
    stableQuery();
    session.pause(0);
    stableQuery();
    session.resume();
    for (let tick = 0; tick < 4000 && session.phase_code() === 5; tick++) {
      session.advance_tick(0, 0, 0, 0);
    }
    expect(session.phase_code()).toBe(7);
    stableQuery();
    const json = session.export_flight_record_json();
    expect(compare_personal_best_json(json, json)).toBe(2);
    stableQuery();
    expect(session.export_flight_record_json()).toBe(json);
    session.enter_replay();
    session.seek_playback(0.5);
    const clock = session.playback_clock_state();
    stableQuery();
    expect(session.playback_clock_state()).toEqual(clock);
    expect(session.export_flight_record_json()).toBe(json);
    session.leave_replay();
    expect(session.phase_code()).toBe(7);
    stableQuery();
  } finally {
    session.free();
  }
});

it("retains archived named-preset overrides and never attributes them to the demo", () => {
  const player = new GameSessionBridge(0);
  const archive = new GameSessionBridge(2);
  try {
    player.open_setup();
    player.set_difficulty_preset(1);
    startFlight(player);
    player.advance_tick(0, 0, 0, 0);
    player.abort();
    const metadata = Array.from(player.configuration_metadata());
    expect(metadata[0]).toBe(1);
    const json = player.export_flight_record_json();
    archive.open_archived_flight_record(json);
    expect(archive.is_archived_replay()).toBe(true);
    const samples = archive.flight_record_samples_packed();
    const clock = archive.playback_clock_state();
    expect(Array.from(archive.configuration_metadata())).toEqual(metadata);
    expect(archive.difficulty_preset_code()).toBe(1);
    expect(archive.flight_record_samples_packed()).toEqual(samples);
    expect(archive.playback_clock_state()).toEqual(clock);
    archive.leave_replay();
    expect(archive.phase_code()).toBe(0);
    const selected = selectedMetadata(archive);
    archive.enter_attract();
    expect(Array.from(archive.configuration_metadata())).toEqual(demoMetadata);
    expect(selectedMetadata(archive)).toEqual(selected);
    archive.leave_attract();
    archive.open_archived_flight_record(json);
    expect(Array.from(archive.configuration_metadata())).toEqual(metadata);
    expect(archive.flight_record_samples_packed()).toEqual(samples);
    expect(player.export_flight_record_json()).toBe(json);
    expect(compare_personal_best_json(json, json)).toBe(4);
  } finally {
    archive.free();
    player.free();
  }
});
