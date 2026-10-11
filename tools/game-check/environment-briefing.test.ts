import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { NO_ENVIRONMENT_BRIEFING, parseEnvironmentBriefingSnapshot } from "../../web/src/game/environment-briefing.js";

function snapshot(metadata: Record<string, unknown> = {}, phaseCode = 1, source = "selected"): string {
  return JSON.stringify({ schema_version: 1, context: { kind: "session", phase_code: phaseCode }, projection: {
    kind: "available", source, metadata: {
      name: "Declared condition", representative_position_ned_m: [3, 4, -11], representative_altitude_m: 11,
      representative_velocity_ned_mps: [-0.25, 0.5, -0.1], wind_domain: { kind: "grid" },
      provenance: { wind_grid: { kind: "assumed", rationale: "Software fixture" } }, ...metadata
    }
  } });
}

describe("Environment briefing boundary", () => {
  it("projects explicit wind components, reference location and provenance without recalculation", () => {
    const projection = parseEnvironmentBriefingSnapshot(snapshot(), 1);
    expect(projection).toEqual({ kind: "available", name: "Declared condition",
      representativePositionNedMeters: [3, 4, -11], representativeAltitudeMeters: 11,
      representativeWindNedMetersPerSecond: [-0.25, 0.5, -0.1], spatialVariation: "grid", windBasis: "assumed" });
    expect(Object.isFrozen(projection)).toBe(true);
  });

  it.each([
    { representative_velocity_ned_mps: [0, "0", 0] }, { representative_position_ned_m: [0, 0] },
    { representative_altitude_m: null }, { name: "" }, { wind_domain: { kind: "unknown" } },
    { provenance: { wind_grid: { kind: "unknown" } } }
  ])("rejects missing and invalid metadata %o", (metadata) => {
    expect(parseEnvironmentBriefingSnapshot(snapshot(metadata), 1)).toBe(NO_ENVIRONMENT_BRIEFING);
  });

  it("rejects another phase, missing metadata and archive sources without inheriting prior conditions", () => {
    expect(parseEnvironmentBriefingSnapshot(snapshot(), 3)).toBe(NO_ENVIRONMENT_BRIEFING);
    for (const json of ["{", "null", snapshot().replace('"selected"', '"archive"'), snapshot().replace('"available"', '"unavailable"')]) {
      expect(parseEnvironmentBriefingSnapshot(json, 1)).toBe(NO_ENVIRONMENT_BRIEFING);
    }
  });

  it.each([1, 2, 3, 4, 5, 6, 7, 8].flatMap((phaseCode) =>
    ["selected", "sealed", "record", "archive", "attract", "registry"].map((source) => ({ phaseCode, source }))
  ))("matches the Rust phase/source contract %o", ({ phaseCode, source }) => {
    const projection = parseEnvironmentBriefingSnapshot(snapshot({}, phaseCode, source), phaseCode);
    const expectedSource = phaseCode === 1 ? "selected" : "sealed";
    if (source === expectedSource) {
      expect(projection.kind).toBe("available");
    } else {
      expect(projection).toBe(NO_ENVIRONMENT_BRIEFING);
    }
  });

  it.each([-1, 0, 9, 10, 100, 0.5, 1.5, 2.5, Number.NaN, Number.POSITIVE_INFINITY].flatMap((phaseCode) =>
    ["selected", "sealed"].map((source) => ({ phaseCode, source }))
  ))("rejects unsupported and noninteger phases %o", ({ phaseCode, source }) => {
    expect(parseEnvironmentBriefingSnapshot(snapshot({}, phaseCode, source), phaseCode)).toBe(NO_ENVIRONMENT_BRIEFING);
  });

  it("decodes both selection and sealed Briefing metadata from actual WASM", () => {
    initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
    const session = new HybridGameSessionBridge(0, 21, 22);
    try {
      session.open_setup();
      session.set_weather_class(2);
      const setup = parseEnvironmentBriefingSnapshot(session.environment_snapshot_json(), 1);
      expect(setup).toMatchObject({ kind: "available", spatialVariation: "grid" });
      if (setup.kind !== "available") throw new Error("Expected the registered Typical wind");
      expect(setup.representativeWindNedMetersPerSecond[0]).toBeCloseTo(-1.767766953, 12);
      expect(setup.representativeWindNedMetersPerSecond[1]).toBeCloseTo(1.767766953, 12);
      expect(setup.representativeWindNedMetersPerSecond[2]).toBe(0);
      session.prepare();
      session.mark_briefing_ready();
      expect(parseEnvironmentBriefingSnapshot(session.environment_snapshot_json(), 3)).toEqual(setup);
    } finally {
      session.free();
    }
  });
});
