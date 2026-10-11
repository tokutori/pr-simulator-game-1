import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IDBFactory } from "fake-indexeddb";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { createAppSession, initializeAppSession } from "../../web/src/app/session-factory.js";
import type { AppSessionConfiguration } from "../../web/src/app/session-factory.js";
import { createArchivedPersonalBestSelection } from "../../web/src/game/archived-personal-best.js";
import { FlightRecordRepository, IndexedDbFlightRecordPersistence } from "../../web/src/game/flight-record-store.js";
import { currentRecordFixture, launchCurrentSession, neutralTailInput } from "./current-session-fixture.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
afterEach(() => { vi.restoreAllMocks(); });

describe("current application session factory", () => {
  it.each([0, 1, 2] as const)("constructs mode %s with Rust-owned state and query generations", (mode) => {
    const session = createAppSession({ controlModeCode: mode, seedLow: 0xffff_ffff, seedHigh: 22 });
    const free = vi.spyOn(HybridGameSessionBridge.prototype, "free");
    try {
      expect(session.readLifecycle()).toMatchObject({ phaseCode: 0, controlModeCode: mode });
      expect(session.physicsHz).toBe(physics_hz());
      const observed = session.captureQueryToken();
      launchCurrentSession(session);
      expect(session.acceptQuery(observed, "Title").kind).toBe("stale");
      session.flightPort.advance_tick_json(neutralTailInput);
      expect(session.readSnapshot()).toMatchObject({ frame: { kind: "flight", state: { tick: 1, pilotPositionTargetNormalized: 0 } } });
      const snapshot = session.readSnapshot();
      if (snapshot.frame.kind !== "flight" || snapshot.identity.kind !== "prepared") throw new Error("Expected a sealed Rust flight");
      expect(snapshot.frame.state.physicalIncidence).not.toHaveProperty("roll");
      expect(snapshot.identity.scenario.seedLow).toBe(0xffff_ffff);
      const beforeRetry = session.captureQueryToken();
      session.executeOperation("abort");
      const record = session.exportRecordJson();
      session.executeOperation("retry");
      expect(session.acceptQuery(beforeRetry, record).kind).toBe("stale");
      const beforeDispose = session.captureQueryToken();
      session.flightPort.free();
      expect(session.acceptQuery(beforeDispose, record).kind).toBe("stale");
      expect(() => session.readLifecycle()).toThrow("disposed");
      expect(() => session.flightPort.snapshot_json()).toThrow("disposed");
    } finally {
      session.dispose();
    }
    expect(free).toHaveBeenCalledTimes(1);
  });

  it("shares WASM initialization across current sessions", async () => {
    const sessions = await Promise.all([
      initializeAppSession({ controlModeCode: 1, seedLow: 31, seedHigh: 32 }),
      initializeAppSession({ controlModeCode: 2, seedLow: 31, seedHigh: 32 })
    ]);
    try {
      expect(sessions.map((session) => session.readLifecycle().controlModeCode)).toEqual([1, 2]);
    } finally {
      sessions.forEach((session) => { session.dispose(); });
    }
  });

  it("releases a resource whose first projection fails", () => {
    const free = vi.spyOn(HybridGameSessionBridge.prototype, "free");
    vi.spyOn(HybridGameSessionBridge.prototype, "phase_code").mockReturnValue(99);
    expect(() => createAppSession({ controlModeCode: 0, seedLow: 31, seedHigh: 32 })).toThrow(RangeError);
    expect(free).toHaveBeenCalledTimes(1);
  });

  it.each([-1, 0.5, 3, NaN, Infinity])("rejects invalid mode %s before WASM coercion", (mode) => {
    expect(() => createAppSession({ controlModeCode: mode, seedLow: 0, seedHigh: 0 } as AppSessionConfiguration)).toThrow(RangeError);
  });

  it.each([-1, 0.5, 0x1_0000_0000, NaN, Infinity])("rejects invalid seed %s before WASM coercion", (seed) => {
    expect(() => createAppSession({ controlModeCode: 0, seedLow: seed, seedHigh: 0 })).toThrow(RangeError);
    expect(() => createAppSession({ controlModeCode: 0, seedLow: 0, seedHigh: seed })).toThrow(RangeError);
  });
});

describe("current Personal Best selection", () => {
  it.each([1, 2, 3, 4, 5])("rejects retired schema %s without conversion", (schemaVersion) => {
    const document = JSON.parse(currentRecordFixture()) as Record<string, unknown>;
    document.schema_version = schemaVersion;
    expect(() => createArchivedPersonalBestSelection(JSON.stringify(document))).toThrow("Unsupported");
  });

  it("keeps first-winner ties, validates candidates, and releases ownership once", () => {
    const json = currentRecordFixture();
    const selection = createArchivedPersonalBestSelection(json);
    expect(selection.is_eligible()).toBe(true);
    selection.consider_existing(3, json);
    selection.consider_existing(4, json);
    expect(selection.candidate_is_best()).toBe(false);
    expect(selection.selected_existing_id()).toBe(3);
    expect(() => { selection.consider_existing(0, json); }).toThrow(RangeError);
    expect(() => { selection.consider_existing(5, '{"schema_version":6}'); }).toThrow();
    expect(selection.selected_existing_id()).toBe(3);
    selection.free();
    selection.free();
    expect(() => selection.candidate_is_best()).toThrow("disposed");
  });

  it.each(["null", "[]", "{}", "{", '{"schema_version":0}', '{"schema_version":7}', '{"schema_version":1.5}', '{"schema_version":"6"}', '{"schema_version":6}'])("rejects malformed candidate %s", (json) => {
    expect(() => createArchivedPersonalBestSelection(json)).toThrow();
  });

  it("retains current records and Personal Best indexes across saves", async () => {
    const json = currentRecordFixture();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(new IDBFactory(), "current-factory-records"),
      () => new Date("2026-10-08T00:00:00Z"), createArchivedPersonalBestSelection);
    const first = await repository.saveFrom({ export_flight_record_json: () => json });
    const second = await repository.saveFrom({ export_flight_record_json: () => json });
    expect(first.personalBest.kind).toBe("candidate");
    expect(second.personalBest).toMatchObject({ kind: "existing", id: first.id });
    expect(await repository.load(first.id)).toBe(json);
    expect(await repository.list()).toEqual([
      { id: second.id, savedAt: second.savedAt, personalBest: false },
      { id: first.id, savedAt: first.savedAt, personalBest: true }
    ]);
  });
});
