import { describe, expect, it } from "vitest";
import { acceptsFlightActivation, failFlightLaunch, flightRuntimeNotice, stopFlightRuntime } from "../../web/src/app/flight-runtime-ui.js";
import type { FlightRuntimeCorrelation, FlightRuntimeUiState } from "../../web/src/app/flight-runtime-ui.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const raw = new Array<number>(33).fill(0);
raw[7] = 1;
raw[19] = -1;
raw[31] = 1;
const snapshot = parseFlightSnapshot(raw);
const projection = { phaseCode: 5, snapshot };
const stopped: FlightRuntimeUiState = { kind: "stopped", launchRequestId: 2, cause: "previous failure" };
const expected: FlightRuntimeCorrelation = { expectedLaunchRequestId: 7, flightRuntime: stopped };

describe("Pure launch correlation and retained flight diagnostic", () => {
  it("requires the exact authorized launch, completed countdown and fresh airborne projection", () => {
    expect(acceptsFlightActivation(expected, 7, 4, 0, projection)).toBe(true);
    for (const requestId of [2, 6, 8]) expect(acceptsFlightActivation(expected, requestId, 4, 0, projection)).toBe(false);
    expect(acceptsFlightActivation(expected, 7, 4, 1, projection)).toBe(false);
    expect(acceptsFlightActivation(expected, 7, 3, 0, projection)).toBe(false);
    expect(acceptsFlightActivation(expected, 7, 4, 0, { phaseCode: 5, snapshot: null })).toBe(false);
    expect(acceptsFlightActivation({ ...expected, expectedLaunchRequestId: null }, 7, 4, 0, projection)).toBe(false);
  });

  it("invalidates pre-launch failure without clearing the previous cause", () => {
    const result = failFlightLaunch(expected, 7, 4, 2, "launch rejected", { kind: "not-launched" });
    expect(result?.correlation).toEqual({ expectedLaunchRequestId: null, flightRuntime: stopped });
    expect(result?.projection).toBeNull();
    expect(failFlightLaunch(expected, 6, 4, 0, "stale", { kind: "not-launched" })).toBeNull();
  });

  it("replaces the old cause only for a new exact post-launch epoch with its own projection", () => {
    const result = failFlightLaunch(expected, 7, 4, 0, "reset failed", { kind: "launched", projection });
    expect(result?.projection).toBe(projection);
    expect(result?.correlation).toEqual({ expectedLaunchRequestId: null, flightRuntime: { kind: "stopped", launchRequestId: 7, cause: "reset failed" } });
    expect(failFlightLaunch(expected, 7, 4, 1, "early", { kind: "launched", projection })).toBeNull();
  });

  it("does not substitute a previous run when the fresh projection is unavailable", () => {
    const result = failFlightLaunch(expected, 7, 4, 0, "snapshot getter failed", { kind: "projection-unavailable" });
    expect(result?.projection).toBeNull();
    expect(result?.correlation.flightRuntime.kind).toBe("projection-unavailable");
    expect(failFlightLaunch(expected, 7, 4, 0, "invalid", { kind: "launched", projection: { phaseCode: 5, snapshot: null } })?.correlation.flightRuntime.kind).toBe("projection-unavailable");
  });

  it("preserves the first cause and rejects old messages after same-controller epoch reuse", () => {
    const active: FlightRuntimeUiState = { kind: "active", launchRequestId: 7 };
    expect(stopFlightRuntime(active, 2, 5, "old callback")).toBe(active);
    const result = stopFlightRuntime(active, 7, 6, "first");
    expect(result).toEqual({ kind: "stopped", launchRequestId: 7, cause: "first" });
    expect(stopFlightRuntime(result, 7, 6, "duplicate")).toBe(result);
    expect(stopFlightRuntime(active, 7, 7, "terminal callback failed")).toEqual({ kind: "stopped", launchRequestId: 7, cause: "terminal callback failed" });
    expect(stopFlightRuntime(active, 2, 7, "old completed run")).toBe(active);
  });

  it("distinguishes the current stop from a retained previous operational-flight cause", () => {
    expect(flightRuntimeNotice(expected, 4)).toContain("直前の操作飛行");
    expect(flightRuntimeNotice({ ...expected, expectedLaunchRequestId: null }, 5)).toContain("最後の有効値");
    for (const phase of [0, 7, 9]) expect(flightRuntimeNotice({ ...expected, expectedLaunchRequestId: null }, phase)).toContain("直前の操作飛行");
    expect(flightRuntimeNotice({ expectedLaunchRequestId: null, flightRuntime: { kind: "active", launchRequestId: 7 } }, 5)).toBeNull();
  });
});
