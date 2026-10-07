import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { GameSessionBridge, HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { parseNamedRecordSummary, parseNamedWindGrid } from "../../web/src/game/named-record-analysis.js";
import type { NamedWindGridRequest } from "../../web/src/game/named-record-analysis.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const request: NamedWindGridRequest = { northMinimumMeters: -10, eastMinimumMeters: -10, altitudeMeters: 10, spacingMeters: 5 };
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function jsonObject(json: string): Record<string, unknown> {
  const parsed: unknown = JSON.parse(json);
  return parsed as Record<string, unknown>;
}

function nested(parent: Record<string, unknown>, key: string): Record<string, unknown> {
  return parent[key] as Record<string, unknown>;
}

function result(ticks = 1): HybridGameSessionBridge {
  const bridge = new HybridGameSessionBridge(0, 41, 42);
  bridge.open_setup();
  bridge.prepare();
  bridge.mark_briefing_ready();
  bridge.start_countdown(1);
  bridge.advance_countdown();
  bridge.launch();
  for (let index = 0; index < ticks; index += 1) bridge.advance_tick_json(neutral);
  bridge.abort();
  return bridge;
}

function windJson(bridge: HybridGameSessionBridge, grid = request): string {
  return bridge.flight_wind_grid_json(grid.northMinimumMeters, grid.eastMinimumMeters, grid.altitudeMeters, grid.spacingMeters);
}

function legacyRecord(): Record<string, unknown> {
  const bridge = new GameSessionBridge(0);
  try {
    bridge.open_setup();
    bridge.set_difficulty_preset(1);
    bridge.prepare();
    bridge.mark_briefing_ready();
    bridge.start_countdown(1);
    bridge.advance_countdown();
    bridge.launch();
    bridge.advance_tick(0, 0, 0, 0);
    bridge.abort();
    return jsonObject(bridge.export_flight_record_json());
  } finally {
    bridge.free();
  }
}

function legacyVersion(version: number): Record<string, unknown> {
  const saved = legacyRecord();
  saved.schema_version = version;
  const header = nested(saved, "header");
  if (version < 5) delete header.personal_best_key;
  if (version < 4) delete header.physics_model_version;
  if (version < 3) delete header.score_definition_version;
  if (version < 2) delete nested(header, "difficulty").hud_profile;
  return saved;
}

describe("named Rust record summary and wind boundary", () => {
  it("preserves Rust Result metrics and provider values through Replay without moving the cursor", () => {
    const bridge = result();
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    try {
      const raw = jsonObject(bridge.flight_record_summary_json());
      const summary = facade.readRecordSummary();
      expect(summary.context.phase).toBe("result");
      expect(summary.sampleCount).toBe(nested(raw, "summary").sample_count);
      expect(summary.maximumAltitudeMeters).toBe(nested(raw, "summary").maximum_altitude_m);
      expect(summary.maximumAirspeedMetersPerSecond).toBe(nested(raw, "summary").maximum_airspeed_mps);
      expect(summary.maximumGroundspeedMetersPerSecond).toBe(nested(raw, "summary").maximum_groundspeed_mps);
      expect(summary.scoreMeters.kind).toBe("available");
      const wind = facade.queryWindGrid(request);
      expect(wind).toMatchObject({ context: summary.context, grid: { rows: 5, columns: 5 }, projection: { kind: "available", source: "record" } });
      if (wind.projection.kind !== "available") throw new Error("Expected registered provider samples");
      const rawWind = nested(jsonObject(windJson(bridge)), "projection").samples as Record<string, unknown>[];
      expect(wind.projection.samples.map((sample) => sample.velocityNedMetersPerSecond)).toEqual(rawWind.map((sample) => sample.velocity_ned_mps));
      expect(wind.projection.samples).toHaveLength(25);
      facade.executeOperation("enter-replay");
      facade.seekPlayback(0.005);
      const clock = facade.readPlaybackClock();
      const replaySummary = facade.readRecordSummary();
      expect({ ...replaySummary, context: summary.context }).toEqual(summary);
      expect(replaySummary.context.phase).toBe("replay");
      expect(facade.queryWindGrid(request).projection).toEqual(wind.projection);
      expect(facade.readPlaybackClock()).toEqual(clock);
      expect(facade.queryRecordDisplay(0.005).stamp.timeSeconds).toBe(0.005);
      expect(replaySummary.durationSeconds).toBe(0.01);
    } finally {
      facade.dispose();
    }
  });

  it.each([1, 2, 3, 4, 5])("preserves saved v%d legacy layout, score and environment values", (version) => {
    const saved = legacyVersion(version);
    const facade = new TailAppSessionFacade(new HybridGameSessionBridge(2, 91, 92), physics_hz());
    try {
      facade.openArchive(JSON.stringify(saved));
      const summary = facade.readRecordSummary();
      expect(summary.context).toMatchObject({ phase: "replay", controlLayout: "legacy_three_axis", controlIdentity: null });
      expect(summary.context.finalization).not.toHaveProperty("failure");
      expect(summary.context.finalization.scoreMeters).toEqual(nested(saved, "finalization").score_m);
      const grid = facade.queryWindGrid(request);
      expect(grid.projection).toMatchObject({ kind: "available", source: "archive", identity: summary.context.scenario });
      expect(facade.queryRecordSample(0.005).controls.layout).toBe("legacy_three_axis");
      expect(summary.scoreMeters.kind).toBe("available");
    } finally {
      facade.dispose();
    }
  });

  it("preserves missing legacy metrics as reasoned tags, including a zero-duration record", () => {
    const saved = legacyVersion(1);
    nested(saved, "finalization").score_m = null;
    for (const sample of saved.samples as Record<string, unknown>[]) nested(sample, "telemetry").angle_of_attack_rad = null;
    const viewer = new TailAppSessionFacade(new HybridGameSessionBridge(2, 91, 92), physics_hz());
    const zero = new TailAppSessionFacade(result(0), physics_hz());
    try {
      viewer.openArchive(JSON.stringify(saved));
      expect(viewer.readRecordSummary()).toMatchObject({ maximumAngleOfAttackRadians: { kind: "unavailable", reason: "no_defined_sample" },
        scoreMeters: { kind: "unavailable", reason: "score_not_recorded" } });
      expect(zero.readRecordSummary()).toMatchObject({ sampleCount: 1, durationSeconds: 0 });
    } finally {
      viewer.dispose();
      zero.dispose();
    }
  });

  it("retains saved v6 original failure and separates unknown identity from outside registered domain", () => {
    const source = result();
    const facade = new TailAppSessionFacade(source, physics_hz());
    const viewer = new TailAppSessionFacade(new HybridGameSessionBridge(2, 91, 92), physics_hz());
    try {
      const outside = facade.queryWindGrid({ ...request, northMinimumMeters: 1e6, eastMinimumMeters: 1e6 });
      expect(outside.projection).toMatchObject({ kind: "unavailable", reason: "outside_registered_domain" });
      expect(outside.projection).not.toHaveProperty("samples");
      const saved = jsonObject(facade.exportRecordJson());
      nested(saved, "header").environment_version = 99;
      Object.assign(nested(saved, "finalization"), { reason: "out_of_valid_envelope", disposition: "failed", failure: {
        dynamics: { load: { aerodynamic: { hybrid: { site: { proxy: { surface: "horizontal_tail", index: 1 } },
          cause: { wind: "outside_grid" }, limit: null, stage: "second" } } } }
      } });
      viewer.openArchive(JSON.stringify(saved));
      const summary = viewer.readRecordSummary();
      if (summary.context.controlLayout !== "tail_incidence") throw new Error("Expected the original tail record");
      expect(summary.context.finalization.failure).toMatchObject({ kind: "dynamics", cause: { kind: "load", cause: { kind: "aerodynamic" } } });
      const unknown = viewer.queryWindGrid(request);
      expect(unknown.projection).toMatchObject({ kind: "unavailable", source: "archive", reason: "unregistered_environment_identity",
        identity: summary.context.scenario });
      expect(unknown.projection).not.toHaveProperty("samples");
      expect(unknown.context).toEqual(summary.context);
    } finally {
      facade.dispose();
      viewer.dispose();
    }
  });

  it("queries Attract metrics and zero wind from the actual demo provider without live snapshots", () => {
    const bridge = new HybridGameSessionBridge(0, 91, 92);
    const facade = new TailAppSessionFacade(bridge, physics_hz());
    const liveSnapshot = vi.spyOn(bridge, "snapshot_json");
    try {
      facade.executeOperation("enter-attract");
      const clock = facade.readPlaybackClock();
      const summary = facade.readRecordSummary();
      expect(summary.context).toEqual(facade.readAttractContext());
      const wind = facade.queryWindGrid(request);
      expect(wind.projection).toMatchObject({ kind: "available", source: "attract" });
      if (wind.projection.kind !== "available") throw new Error("Expected the registered Calm demo wind");
      expect(wind.projection.samples.every((sample) => sample.velocityNedMetersPerSecond.every((component) => component === 0))).toBe(true);
      expect(facade.readPlaybackClock()).toEqual(clock);
      expect(liveSnapshot).not.toHaveBeenCalled();
    } finally {
      facade.dispose();
    }
  });

  it("rejects malformed summary frequency, phase, availability, finite values and terminal correspondence", () => {
    const bridge = result();
    try {
      const original = bridge.flight_record_summary_json();
      const mutations: readonly ((document: Record<string, unknown>) => void)[] = [
        (document) => { document.schema_version = 1; }, (document) => { document.physics_hz = 60; },
        (document) => { nested(document, "context").phase = "flight"; },
        (document) => { nested(document, "summary").sample_count = 0; },
        (document) => { nested(document, "summary").duration_seconds = 0.03; },
        (document) => { nested(document, "summary").maximum_altitude_m = null; },
        (document) => { nested(document, "summary").maximum_groundspeed_mps = -1; },
        (document) => { nested(document, "summary").maximum_absolute_roll_rad = -1; },
        (document) => { nested(document, "summary").maximum_angle_of_attack_rad = null; },
        (document) => { nested(document, "summary").maximum_angle_of_attack_rad = { kind: "unavailable", reason: "unknown" }; },
        (document) => { nested(document, "summary").score_m = { kind: "unavailable", reason: "score_not_recorded" }; },
        (document) => { nested(nested(nested(document, "summary"), "score_m"), "value").net_horizontal_m = -1; }
      ];
      for (const mutate of mutations) {
        const document = jsonObject(original);
        mutate(document);
        expect(() => parseNamedRecordSummary(JSON.stringify(document), physics_hz())).toThrow(RangeError);
      }
      expect(() => parseNamedRecordSummary(original, 0)).toThrow(RangeError);
      expect(() => parseNamedRecordSummary(" ".repeat(65_537), physics_hz())).toThrow(RangeError);
    } finally {
      bridge.free();
    }
  });

  it("rejects empty, reordered, nonfinite, mismatched source/identity/context and partial wind grids", () => {
    const bridge = result();
    try {
      const context = parseNamedRecordSummary(bridge.flight_record_summary_json(), physics_hz()).context;
      const original = windJson(bridge);
      const mutations: readonly ((document: Record<string, unknown>) => void)[] = [
        (document) => { document.schema_version = 1; }, (document) => { nested(document, "grid").rows = 4; },
        (document) => { nested(document, "grid").spacing_m = 0; }, (document) => { nested(document, "grid").altitude_m = 11; },
        (document) => { nested(nested(document, "context"), "scenario").seed_low = 0; },
        (document) => { nested(nested(document, "projection"), "identity").environment_version = 99; },
        (document) => { nested(document, "projection").source = "archive"; },
        (document) => { nested(document, "projection").samples = []; },
        (document) => { const samples = nested(document, "projection").samples as unknown[]; samples.reverse(); },
        (document) => { const samples = nested(document, "projection").samples as Record<string, unknown>[];
          const first = samples[0];
          if (first === undefined) throw new Error("Expected the first provider sample");
          first.velocity_ned_mps = [0, null, 0]; },
        (document) => { nested(document, "projection").kind = "unavailable"; nested(document, "projection").reason = "outside_registered_domain"; }
      ];
      for (const mutate of mutations) {
        const document = jsonObject(original);
        mutate(document);
        expect(() => parseNamedWindGrid(JSON.stringify(document), context, request, "record")).toThrow(RangeError);
      }
      const unknown = jsonObject(original);
      const projection = nested(unknown, "projection");
      projection.kind = "unavailable";
      projection.reason = "unknown_reason";
      delete projection.samples;
      expect(() => parseNamedWindGrid(JSON.stringify(unknown), context, request, "record")).toThrow(RangeError);
    } finally {
      bridge.free();
    }
  });

  it("rejects stale same-identity Summary and Wind observations after Retry and disposed resources", async () => {
    const facade = new TailAppSessionFacade(result(), physics_hz());
    try {
      const token = facade.captureQueryToken();
      const summary = facade.readRecordSummary();
      const wind = Promise.resolve(facade.queryWindGrid(request));
      facade.executeOperation("retry");
      const snapshot = facade.readSnapshot();
      if (snapshot.identity.kind !== "prepared") throw new Error("Expected the same Retry blueprint");
      expect(snapshot.identity.scenario).toEqual(summary.context.scenario);
      expect(facade.acceptQuery(token, await wind).kind).toBe("stale");
      expect(facade.acceptQuery(token, summary).kind).toBe("stale");
      expect(() => facade.readRecordSummary()).toThrow();
      facade.dispose();
      expect(() => facade.queryWindGrid(request)).toThrow("disposed");
    } finally {
      facade.dispose();
    }
  });
});
