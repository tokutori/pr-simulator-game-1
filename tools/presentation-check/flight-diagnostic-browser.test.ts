import { describe, expect, it, vi } from "vitest";
import { createCorrelatedFlightHudPort, recoverStoppedFlight, sameFlightControllerIdentity } from "../../web/src/app/flight-diagnostic-browser.js";
import type { FlightControllerIdentity } from "../../web/src/app/flight-diagnostic-browser.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";

const raw = new Array<number>(33).fill(0);
raw[7] = 1;
raw[16] = 4;
raw[19] = -1;
raw[31] = 1;
const projection = { phaseCode: 7, snapshot: parseFlightSnapshot(raw) };

describe("Correlated flight failure and safe recovery effect boundary", () => {
  it("captures the epoch at synchronous fail invocation and rejects a stale same-object identity", () => {
    const session = {};
    const controller = {};
    let current: FlightControllerIdentity<object, object> | null = { session, controller, launchRequestId: 2 };
    const failures: { identity: FlightControllerIdentity<object, object>; cause: string }[] = [];
    const delegate = { render: vi.fn(), setVisible: vi.fn(), fail: vi.fn() };
    const port = createCorrelatedFlightHudPort(delegate, () => current, (identity, cause) => { failures.push({ identity, cause }); });
    port.fail("first");
    current = { session, controller, launchRequestId: 7 };
    port.fail("second");
    expect(failures.map((failure) => failure.identity.launchRequestId)).toEqual([2, 7]);
    const first = failures[0];
    const second = failures[1];
    if (first === undefined || second === undefined) throw new Error("Missing captured failure identities");
    expect(sameFlightControllerIdentity(current, first.identity)).toBe(false);
    expect(sameFlightControllerIdentity(current, second.identity)).toBe(true);
    current = null;
    port.fail("disposed");
    expect(delegate.fail).toHaveBeenCalledTimes(2);
    expect(failures).toHaveLength(2);
  });

  it.each([5, 6])("checks actual Rust phase %s, commits fresh terminal before a failing reset", (phase) => {
    const order: string[] = [];
    const session = { phase_code: () => phase, abort: () => { order.push("abort"); return raw; } };
    const result = recoverStoppedFlight(session, () => true, () => { order.push("projection"); return projection; },
      (fresh) => { order.push("commit"); expect(fresh).toBe(projection); }, () => { order.push("reset"); throw new Error("reset failed"); });
    expect(order).toEqual(["abort", "projection", "commit", "reset"]);
    expect(result).toEqual({ kind: "ready", projection, resetFailure: "reset failed" });
  });

  it.each([0, 1, 2, 3, 4, 7, 8, 9, 10])("does not authorize Abort from actual Rust phase %s", (phase) => {
    const abort = vi.fn(() => raw);
    const reset = vi.fn();
    const result = recoverStoppedFlight({ phase_code: () => phase, abort }, () => true, () => projection, vi.fn(), reset);
    expect(result.kind).toBe("ready");
    expect(abort).not.toHaveBeenCalled();
    expect(reset).not.toHaveBeenCalled();
  });

  it("does not fabricate Abort permission or completion when the phase getter throws", () => {
    const abort = vi.fn(() => raw);
    const commit = vi.fn();
    const result = recoverStoppedFlight({ phase_code: () => { throw new Error("phase unavailable"); }, abort }, () => true, () => projection, commit, vi.fn());
    expect(result).toEqual({ kind: "unavailable", cause: "phase unavailable" });
    expect(abort).not.toHaveBeenCalled();
    expect(commit).not.toHaveBeenCalled();
  });

  it("rejects replaced session correlation after its phase getter and before Abort", () => {
    let current = true;
    const abort = vi.fn(() => raw);
    const result = recoverStoppedFlight({ phase_code: () => { current = false; return 5; }, abort }, () => current, () => projection, vi.fn(), vi.fn());
    expect(result.kind).toBe("stale");
    expect(abort).not.toHaveBeenCalled();
  });

  it("retains unavailable status without committing a previous-run projection", () => {
    const commit = vi.fn();
    const reset = vi.fn();
    const result = recoverStoppedFlight({ phase_code: () => 5, abort: () => raw }, () => true,
      () => { throw new Error("fresh snapshot unavailable"); }, commit, reset);
    expect(result).toEqual({ kind: "unavailable", cause: "fresh snapshot unavailable" });
    expect(commit).not.toHaveBeenCalled();
    expect(reset).not.toHaveBeenCalled();
  });

  it("does not reset a new controller selected by synchronous completion effects", () => {
    let current = true;
    const reset = vi.fn();
    const result = recoverStoppedFlight({ phase_code: () => 5, abort: () => raw }, () => current,
      () => projection, () => { current = false; }, reset);
    expect(result.kind).toBe("stale");
    expect(reset).not.toHaveBeenCalled();
  });
});
