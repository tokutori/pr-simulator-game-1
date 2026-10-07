import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IDBFactory } from "fake-indexeddb";
import { GameSessionBridge, HybridGameSessionBridge, initSync, PersonalBestSelectionBridge, physics_hz, TailPersonalBestSelectionBridge } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession, initializeAppSession } from "../../web/src/app/session-factory.js";
import type { AppSessionConfiguration } from "../../web/src/app/session-factory.js";
import type { AppSessionFacade } from "../../web/src/app/session-facade.js";
import { createArchivedPersonalBestSelection } from "../../web/src/game/archived-personal-best.js";
import { FlightRecordRepository, IndexedDbFlightRecordPersistence } from "../../web/src/game/flight-record-store.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";
import { initializeGameSession } from "../../web/src/game/wasm-flight.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
afterEach(() => { vi.restoreAllMocks(); });
const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });

function launch(session: AppSessionFacade): void {
  session.executeOperation("open-setup");
  session.executeOperation("prepare");
  session.executeOperation("start-flight");
  while (session.advanceCountdown() > 0) continue;
  session.launch();
}

function tick(session: AppSessionFacade): void {
  if (session.controlLayout === "legacy_three_axis") session.flightPort.advance_tick(0, 0, 0, 0);
  else session.flightPort.advance_tick_json(neutral);
}

interface RecordFixture {
  schema_version: number;
  header: Record<string, unknown>;
  finalization: Record<string, unknown>;
  samples: Record<string, unknown>[];
  control_identity?: Record<string, unknown>;
}

function recordFixture(layout: AppSessionConfiguration["controlLayout"], eligible = true): RecordFixture {
  const configuration: AppSessionConfiguration = layout === "legacy_three_axis"
    ? { controlLayout: layout, controlModeCode: 0 }
    : { controlLayout: layout, controlModeCode: 0, seedLow: 21, seedHigh: 22 };
  const session = createAppSession(configuration);
  try {
    launch(session);
    tick(session);
    session.executeOperation("abort");
    const fixture = JSON.parse(session.exportRecordJson()) as RecordFixture;
    if (eligible) {
      fixture.finalization.reason = "water_contact";
      fixture.finalization.disposition = "complete";
      fixture.header.personal_best_key = Array.from({ length: 32 }, () => layout === "legacy_three_axis" ? 7 : 8);
    }
    return fixture;
  } finally {
    session.dispose();
  }
}

describe("layout-owned application session factory", () => {
  it.each([0, 1, 2] as const)("constructs both layouts in mode %s and keeps Rust snapshots and query generations", (mode) => {
    const legacy = createAppSession({ controlLayout: "legacy_three_axis", controlModeCode: mode });
    const tail = createAppSession({ controlLayout: "tail_incidence", controlModeCode: mode, seedLow: 0xffff_ffff, seedHigh: 22 });
    const legacyFree = vi.spyOn(GameSessionBridge.prototype, "free");
    const tailFree = vi.spyOn(HybridGameSessionBridge.prototype, "free");
    try {
      for (const session of [legacy, tail]) {
        expect(session.readLifecycle()).toMatchObject({ phaseCode: 0, controlModeCode: mode });
        expect(session.physicsHz).toBe(physics_hz());
        const observed = session.captureQueryToken();
        launch(session);
        expect(session.acceptQuery(observed, "old Title").kind).toBe("stale");
        tick(session);
      }
      expect(legacy.readSnapshot()).toMatchObject({ tick: 1, actuatorDeflectionRadians: { roll: 0, pitch: 0, yaw: 0 } });
      expect(tail.readSnapshot()).toMatchObject({ frame: { kind: "flight", state: { tick: 1, pilotPositionTargetNormalized: 0 } } });
      const tailState = tail.readSnapshot();
      if (tailState.frame.kind !== "flight") throw new Error("Expected a Rust flight frame");
      expect(tailState.frame.state.physicalIncidence).not.toHaveProperty("roll");
      if (tailState.identity.kind !== "prepared") throw new Error("Expected a sealed Rust identity");
      expect(tailState.identity.scenario.seedLow).toBe(0xffff_ffff);
      const old = tail.captureQueryToken();
      tail.executeOperation("abort");
      const record = tail.exportRecordJson();
      tail.executeOperation("retry");
      expect(tail.acceptQuery(old, record).kind).toBe("stale");
      const disposed = tail.captureQueryToken();
      legacy.flightPort.free();
      tail.flightPort.free();
      expect(tail.acceptQuery(disposed, record).kind).toBe("stale");
      expect(() => tail.readLifecycle()).toThrow("disposed");
      expect(() => legacy.flightPort.snapshot()).toThrow("disposed");
      expect(() => tail.flightPort.snapshot_json()).toThrow("disposed");
    } finally {
      legacy.dispose();
      tail.dispose();
    }
    expect(legacyFree).toHaveBeenCalledTimes(1);
    expect(tailFree).toHaveBeenCalledTimes(1);
  });

  it("shares WASM initialization and retains the public legacy factory until final consumer integration", async () => {
    const [legacy, tail] = await Promise.all([
      initializeAppSession({ controlLayout: "legacy_three_axis", controlModeCode: 1 }),
      initializeAppSession({ controlLayout: "tail_incidence", controlModeCode: 2, seedLow: 31, seedHigh: 32 })
    ]);
    const previous = await initializeGameSession();
    try {
      expect(legacy.controlLayout).toBe("legacy_three_axis");
      expect(tail.controlLayout).toBe("tail_incidence");
      expect(legacy.readLifecycle().controlModeCode).toBe(1);
      expect(tail.readLifecycle().controlModeCode).toBe(2);
      expect(previous.session).toBeInstanceOf(GameSessionBridge);
      expect(previous.session.control_mode_code()).toBe(0);
    } finally {
      legacy.dispose();
      tail.dispose();
      previous.session.free();
    }
  });

  it.each(["legacy_three_axis", "tail_incidence"] as const)("releases a newly constructed %s resource when its first projection fails", (layout) => {
    const prototype = layout === "legacy_three_axis" ? GameSessionBridge.prototype : HybridGameSessionBridge.prototype;
    const free = vi.spyOn(prototype, "free");
    vi.spyOn(prototype, "phase_code").mockReturnValue(99);
    const configuration: AppSessionConfiguration = layout === "legacy_three_axis"
      ? { controlLayout: layout, controlModeCode: 0 }
      : { controlLayout: layout, controlModeCode: 0, seedLow: 31, seedHigh: 32 };
    expect(() => createAppSession(configuration)).toThrow(RangeError);
    expect(free).toHaveBeenCalledTimes(1);
  });

  it.each([-1, 0.5, 3, NaN, Infinity])("rejects invalid mode %s before WASM integer coercion", (mode) => {
    const configuration = { controlLayout: "legacy_three_axis", controlModeCode: mode } as AppSessionConfiguration;
    expect(() => createAppSession(configuration)).toThrow(RangeError);
  });

  it.each([-1, 0.5, 0x1_0000_0000, NaN, Infinity])("rejects invalid seed component %s before WASM integer coercion", (seed) => {
    expect(() => createAppSession({ controlLayout: "tail_incidence", controlModeCode: 0, seedLow: seed, seedHigh: 0 })).toThrow(RangeError);
    expect(() => createAppSession({ controlLayout: "tail_incidence", controlModeCode: 0, seedLow: 0, seedHigh: seed })).toThrow(RangeError);
  });

  it("rejects unknown layout values", () => {
    const configuration = { controlLayout: "unknown", controlModeCode: 0 } as unknown as AppSessionConfiguration;
    expect(() => createAppSession(configuration)).toThrow("Unknown application session control layout");
  });
});

describe("Rust-owned mixed-layout Personal Best factory", () => {
  it.each([1, 2, 3, 4, 5])("validates saved legacy schema %s through its original Rust decoder", (version) => {
    const record = recordFixture("legacy_three_axis");
    record.schema_version = version;
    if (version < 5) delete record.header.personal_best_key;
    if (version < 4) delete record.header.physics_model_version;
    if (version < 3) delete record.header.score_definition_version;
    if (version < 2) delete (record.header.difficulty as Record<string, unknown>).hud_profile;
    const json = JSON.stringify(record);
    const selection = createArchivedPersonalBestSelection(json);
    const original = new PersonalBestSelectionBridge(json);
    try {
      expect(selection.is_eligible()).toBe(original.is_eligible());
      expect(selection.key_hex()).toBe(original.key_hex());
      expect(selection.candidate_is_best()).toBe(original.candidate_is_best());
    } finally {
      selection.free();
      original.free();
    }
  });

  it.each(["legacy_three_axis", "tail_incidence"] as const)("validates both layouts while comparing only compatible %s records", (layout) => {
    const candidate = recordFixture(layout);
    const other = recordFixture(layout === "legacy_three_axis" ? "tail_incidence" : "legacy_three_axis");
    const json = JSON.stringify(candidate);
    const selection = createArchivedPersonalBestSelection(json);
    try {
      expect(selection.is_eligible()).toBe(true);
      const key = selection.key_hex();
      other.header.personal_best_key = candidate.header.personal_best_key;
      selection.consider_existing(1, JSON.stringify(other));
      expect(selection.key_hex()).toBe(key);
      expect(selection.candidate_is_best()).toBe(true);
      if (candidate.control_identity !== undefined) {
        const mismatch = structuredClone(candidate);
        const identity = mismatch.control_identity;
        if (identity === undefined) throw new Error("Expected a tail control identity");
        identity.controller_profile_id = "different-controller";
        selection.consider_existing(2, JSON.stringify(mismatch));
        expect(selection.candidate_is_best()).toBe(true);
      }
      selection.consider_existing(3, json);
      selection.consider_existing(4, json);
      expect(selection.candidate_is_best()).toBe(false);
      expect(selection.selected_existing_id()).toBe(3);
      other.samples = [];
      expect(() => { selection.consider_existing(5, JSON.stringify(other)); }).toThrow();
      expect(selection.selected_existing_id()).toBe(3);
      expect(() => { selection.consider_existing(0, json); }).toThrow(RangeError);
      expect(() => { selection.consider_existing(6, '{"schema_version":7}'); }).toThrow(RangeError);
    } finally {
      selection.free();
      selection.free();
    }
    expect(() => selection.candidate_is_best()).toThrow("disposed");
    expect(() => { selection.consider_existing(1, json); }).toThrow("disposed");
  });

  it("rejects malformed same-layout records even when the candidate is ineligible", () => {
    const json = JSON.stringify(recordFixture("tail_incidence", false));
    const selection = createArchivedPersonalBestSelection(json);
    try {
      expect(selection.is_eligible()).toBe(false);
      expect(() => { selection.consider_existing(1, '{"schema_version":6}'); }).toThrow();
      expect(() => { selection.consider_existing(1, '{"schema_version":5}'); }).toThrow();
    } finally {
      selection.free();
    }
  });

  it.each(["null", "[]", "{}", "{", '{"schema_version":0}', '{"schema_version":7}', '{"schema_version":1.5}', '{"schema_version":"6"}', '{"schema_version":6}'])("rejects unknown or malformed candidate %s", (json) => {
    expect(() => createArchivedPersonalBestSelection(json)).toThrow();
  });

  it("releases an opposite-layout validation resource and its candidate once", () => {
    const legacy = JSON.stringify(recordFixture("legacy_three_axis"));
    const tail = JSON.stringify(recordFixture("tail_incidence"));
    const legacyFree = vi.spyOn(PersonalBestSelectionBridge.prototype, "free");
    const tailFree = vi.spyOn(TailPersonalBestSelectionBridge.prototype, "free");
    const selection = createArchivedPersonalBestSelection(legacy);
    selection.consider_existing(1, tail);
    selection.free();
    selection.free();
    expect(legacyFree).toHaveBeenCalledTimes(1);
    expect(tailFree).toHaveBeenCalledTimes(1);
  });

  it("rebuilds and scans a mixed IndexedDB store without merging layout keys", async () => {
    const legacy = JSON.stringify(recordFixture("legacy_three_axis"));
    const tail = JSON.stringify(recordFixture("tail_incidence"));
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(new IDBFactory(), "layout-factory-records"),
      () => new Date("2026-10-08T00:00:00Z"), createArchivedPersonalBestSelection);
    const first = await repository.saveFrom({ export_flight_record_json: () => legacy });
    const second = await repository.saveFrom({ export_flight_record_json: () => tail });
    const third = await repository.saveFrom({ export_flight_record_json: () => legacy });
    expect(first.personalBest.kind).toBe("candidate");
    expect(second.personalBest.kind).toBe("candidate");
    expect(third.personalBest).toMatchObject({ kind: "existing", id: first.id });
    expect(await repository.list()).toEqual([
      { id: third.id, savedAt: third.savedAt, personalBest: false },
      { id: second.id, savedAt: second.savedAt, personalBest: true },
      { id: first.id, savedAt: first.savedAt, personalBest: true }
    ]);
  });
});
