import { readFileSync } from "node:fs";
import { beforeAll, expect, it } from "vitest";
import { HybridGameSessionBridge, TailPersonalBestSelectionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { neutralTailInput } from "./current-session-fixture.js";
import { record } from "../shared/validation.js";

beforeAll(() => {
  initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
});

const demoMetadata = [4, 2, 0, 0, 3, 1, 3, 2, 1, 3, 0xD3A0, 0, 1, 0, 0, 0, 0, 0];

function selection(session: HybridGameSessionBridge) {
  return { preset: session.difficulty_preset_code(), information: session.information_level_code(),
    assistance: session.assistance_level_code(), weather: session.weather_class_code(),
    cues: Array.from(session.information_profile_codes()) };
}

function startFlight(session: HybridGameSessionBridge): void {
  session.prepare(); session.mark_briefing_ready(); session.start_countdown(1); session.advance_countdown(); session.launch();
}

it("reports the independent Custom/Minimal/Automatic demonstration identity", () => {
  const session = new HybridGameSessionBridge(2, 21, 22);
  try {
    session.open_setup(); session.set_weather_class(4); session.return_to_title(); session.enter_attract();
    expect(Array.from(session.configuration_metadata())).toEqual(demoMetadata);
    const projection = record(record(JSON.parse(session.environment_snapshot_json()) as unknown).projection);
    expect(projection).toMatchObject({ source: "attract", identity: {
      catalog_version: 3, scenario_id: 1, scenario_version: 3, aircraft_model_version: 2,
      environment_version: 1, controller_profile_version: 3, seed_low: 0xD3A0, seed_high: 0
    } });
    expect(record(projection.metadata).representative_velocity_ned_mps).toEqual([0, 0, 0]);
    expect(() => session.export_flight_record_json()).toThrow();
  } finally { session.free(); }
});

it.each([0, 1, 2, 3, 4])("preserves player Information %s and Custom cues across repeated demo entries", (information) => {
  const session = new HybridGameSessionBridge(2, 21, 22);
  try {
    session.open_setup(); session.set_information_level(information);
    session.set_assistance_level(information % 4); session.set_weather_class(4);
    if (information === 4) [true, false, true, false, true, false].forEach((visible, cue) => { session.set_information_cue(cue, visible); });
    const selected = selection(session);
    session.return_to_title();
    let firstSamples: string | null = null;
    for (let entry = 0; entry < 2; entry += 1) {
      session.enter_attract(); session.seek_playback(0.75);
      const clock = session.playback_clock_state();
      const context = session.playback_context_json();
      const samples = session.flight_analysis_samples_json();
      if (firstSamples === null) firstSamples = samples;
      else expect(samples).toBe(firstSamples);
      for (let query = 0; query < 3; query += 1) {
        expect(Array.from(session.configuration_metadata())).toEqual(demoMetadata);
        expect(session.playback_clock_state()).toEqual(clock);
        expect(session.playback_context_json()).toBe(context);
        expect(session.flight_analysis_samples_json()).toBe(samples);
        expect(session.phase_code()).toBe(10);
      }
      session.leave_attract();
      expect(session.phase_code()).toBe(0);
      expect(selection(session)).toEqual(selected);
    }
    session.open_setup(); expect(selection(session)).toEqual(selected);
    session.prepare();
    const metadata = Array.from(session.configuration_metadata());
    expect(metadata.slice(0, 4)).toEqual([selected.preset, selected.information, selected.assistance, selected.weather]);
    expect(metadata.slice(12)).toEqual(selected.cues);
  } finally { session.free(); }
});

it.each([0, 1, 2, 3])("retains player preset %s after Attract", (preset) => {
  const session = new HybridGameSessionBridge(0, 21, 22);
  try {
    session.open_setup(); session.set_difficulty_preset(preset);
    const selected = selection(session);
    session.return_to_title(); session.enter_attract();
    expect(Array.from(session.configuration_metadata())).toEqual(demoMetadata);
    session.leave_attract(); session.open_setup();
    expect(selection(session)).toEqual(selected);
  } finally { session.free(); }
});

it("keeps configuration queries observational through Briefing, pause, Result and Replay", () => {
  const session = new HybridGameSessionBridge(0, 21, 22);
  try {
    expect(() => session.configuration_metadata()).toThrow();
    session.open_setup(); expect(() => session.configuration_metadata()).toThrow();
    session.set_information_cue(2, false); session.prepare();
    const metadata = Array.from(session.configuration_metadata());
    expect(metadata.slice(0, 4)).toEqual([4, 4, 3, 2]);
    expect(metadata.slice(12)).toEqual([1, 1, 0, 1, 1, 1]);
    const stableQuery = () => {
      const phase = session.phase_code();
      const snapshot = phase === 9 ? session.playback_context_json() : session.snapshot_json();
      const samples = phase === 9 ? session.flight_analysis_samples_json() : null;
      const clock = phase === 9 ? Array.from(session.playback_clock_state()) : null;
      expect(Array.from(session.configuration_metadata())).toEqual(metadata);
      expect(session.phase_code()).toBe(phase);
      expect(phase === 9 ? session.playback_context_json() : session.snapshot_json()).toBe(snapshot);
      if (phase === 9) {
        expect(session.flight_analysis_samples_json()).toBe(samples);
        expect(Array.from(session.playback_clock_state())).toEqual(clock);
      }
    };
    stableQuery(); session.fail_briefing(0); stableQuery(); session.retry_briefing(); stableQuery();
    session.mark_briefing_ready(); stableQuery(); session.start_countdown(1); stableQuery();
    session.advance_countdown(); session.launch(); stableQuery(); session.pause(0); stableQuery();
    session.clear_pause_reason(0); session.resume(); session.advance_tick_json(neutralTailInput); session.abort(); stableQuery();
    const json = session.export_flight_record_json();
    const best = new TailPersonalBestSelectionBridge(json);
    try { expect(best.is_eligible()).toBe(false); } finally { best.free(); }
    session.enter_replay(); session.seek_playback(0.005);
    const clock = session.playback_clock_state();
    stableQuery(); expect(session.playback_clock_state()).toEqual(clock);
    expect(session.export_current_flight_record_json()).toBe(json);
    session.leave_replay(); stableQuery();
  } finally { session.free(); }
});

it("retains current archive preset metadata independently of the demo declaration", () => {
  const player = new HybridGameSessionBridge(0, 21, 22);
  const archived = new HybridGameSessionBridge(2, 31, 32);
  try {
    player.open_setup(); player.set_difficulty_preset(1); startFlight(player);
    player.advance_tick_json(neutralTailInput); player.abort();
    const metadata = Array.from(player.configuration_metadata());
    expect(metadata[0]).toBe(1);
    const json = player.export_flight_record_json();
    archived.open_archived_flight_record(json);
    const samples = archived.flight_analysis_samples_json();
    const clock = archived.playback_clock_state();
    expect(Array.from(archived.configuration_metadata())).toEqual(metadata);
    expect(archived.difficulty_preset_code()).toBe(1);
    expect(archived.flight_analysis_samples_json()).toBe(samples);
    expect(archived.playback_clock_state()).toEqual(clock);
    archived.leave_replay();
    const selected = selection(archived);
    archived.enter_attract(); expect(Array.from(archived.configuration_metadata())).toEqual(demoMetadata);
    archived.leave_attract(); expect(selection(archived)).toEqual(selected);
    archived.open_archived_flight_record(json);
    expect(Array.from(archived.configuration_metadata())).toEqual(metadata);
    expect(archived.flight_analysis_samples_json()).toBe(samples);
    expect(player.export_flight_record_json()).toBe(json);
  } finally { archived.free(); player.free(); }
});
