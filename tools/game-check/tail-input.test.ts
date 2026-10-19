import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-input.js";
import type { TailLogicalInput } from "../../web/src/game/tail-input.js";
import type { TailInputDocument } from "../../web/src/generated/tail-input.js";
import type { TailSnapshotDocument } from "../../web/src/generated/tail-snapshot.js";
import { contractValidator, readContract } from "../game-ui-build/validate.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
const neutral: TailLogicalInput = { controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
  desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } };
const validateInput = contractValidator<TailInputDocument>("tail_input");

function launch(): HybridGameSessionBridge {
  const session = new HybridGameSessionBridge(0, 21, 22);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.observe_countdown(session.countdown_token(), 1, true);
  return session;
}

describe("owned input data and Rust admission", () => {
  it("transfers Hold and Set through the actual generated contract and Rust controller", () => {
    const session = launch();
    try {
      const json = encodeTailLogicalInput({ ...neutral, noseUp: 1, turnRight: 1,
        desiredPitchRateRadiansPerSecond: 0.2, desiredYawRateRadiansPerSecond: 0.2,
        pilotPositionCommand: { kind: "set", normalized: 1 } });
      expect(readContract<TailInputDocument>("tail_input", json)).toEqual({ schema_version: 2, control_layout: "tail_incidence",
        nose_up: 1, turn_right: 1, desired_pitch_rate_rad_s: 0.2, desired_yaw_rate_rad_s: 0.2,
        pilot_position_command: { kind: "set", normalized: 1 } });
      const moved = readContract<TailSnapshotDocument>("tail_snapshot", session.advance_tick_json(json));
      if (moved.frame.kind !== "flight") throw new Error("Expected the committed flight frame");
      expect(moved.frame.state.physical_incidence).toEqual({ horizontal_tail_rad: -0.01, vertical_tail_rad: -0.01 });
      expect(moved.frame.state.pilot_position_target_m).toBe(0.4);
      const held = readContract<TailSnapshotDocument>("tail_snapshot", session.advance_tick_json(encodeTailLogicalInput(neutral)));
      if (held.frame.kind !== "flight") throw new Error("Expected the held flight frame");
      expect(held.frame.state.pilot_position_target_m).toBe(0.4);
    } finally { session.free(); }
  });

  it("lets the Rust boundary reject invalid domain values without advancing state or record", () => {
    const session = launch();
    try {
      const before = session.snapshot_json();
      for (const invalid of [
        { ...neutral, noseUp: 1.01 }, { ...neutral, desiredPitchRateRadiansPerSecond: 0.201 },
        { ...neutral, pilotPositionCommand: { kind: "set", normalized: -1.01 } },
        { ...neutral, pilotPositionCommand: { kind: "hold", normalized: 0 } },
        { ...neutral, pilotPositionCommand: { kind: "set", normalized: 0, extra: true } }
      ]) {
        const json = encodeTailLogicalInput(invalid as TailLogicalInput);
        expect(() => session.advance_tick_json(json)).toThrow();
        expect(session.snapshot_json()).toBe(before);
      }
      for (const invalid of [{ ...neutral, turnRight: NaN }, { ...neutral, desiredYawRateRadiansPerSecond: Infinity },
        { ...neutral, pilotPositionCommand: { kind: "hold", normalized: undefined } }]) {
        expect(() => encodeTailLogicalInput(invalid as TailLogicalInput)).toThrow(RangeError);
        expect(session.snapshot_json()).toBe(before);
      }
      session.abort();
      const record = JSON.parse(session.export_flight_record_json()) as { readonly samples: readonly unknown[] };
      expect(record.samples).toHaveLength(1);
    } finally { session.free(); }
  });

  it("keeps schema-only structural tests outside the shipped runtime", () => {
    const document = readContract<TailInputDocument>("tail_input", encodeTailLogicalInput(neutral));
    for (const field of Object.keys(document)) {
      expect(validateInput(Object.fromEntries(Object.entries(document).filter(([key]) => key !== field))), field).toBe(false);
      expect(validateInput({ ...document, [field]: null }), field).toBe(false);
    }
    expect(validateInput({ ...document, roll: 0 })).toBe(false);
    expect(validateInput({ ...document, pilot_position_command: { kind: "set" } })).toBe(false);
    expect(validateInput({ ...document, desired_pitch_rate_rad_s: 0.201 })).toBe(true);
  });

  it("captures getters once and excludes inherited and non-enumerable toJSON", () => {
    const redirected = Object.defineProperty({ kind: "hold" as const }, "toJSON", { value: () => ({ kind: "set", normalized: 1 }) });
    const inherited = Object.assign(Object.create({ toJSON: () => ({ kind: "set", normalized: 1 }) }) as { kind: "hold" }, { kind: "hold" as const });
    for (const command of [redirected, inherited]) {
      expect(JSON.parse(encodeTailLogicalInput({ ...neutral, pilotPositionCommand: command })))
        .toHaveProperty("pilot_position_command", { kind: "hold" });
    }
    let reads = 0;
    const command = { kind: "set" as const, get normalized() { reads += 1; return reads === 1 ? 0.5 : 10; } };
    expect(JSON.parse(encodeTailLogicalInput({ ...neutral, pilotPositionCommand: command })))
      .toHaveProperty("pilot_position_command", { kind: "set", normalized: 0.5 });
    expect(reads).toBe(1);
    expect(() => encodeTailLogicalInput({ ...neutral, pilotPositionCommand: { kind: "hold", toJSON: () => ({ kind: "set", normalized: 1 }) } } as TailLogicalInput)).toThrow(RangeError);
  });

  it("requires own enumerable outer fields and command discriminants", () => {
    for (const field of Object.keys(neutral)) {
      const missing = Object.fromEntries(Object.entries(neutral).filter(([key]) => key !== field));
      expect(() => encodeTailLogicalInput(missing as unknown as TailLogicalInput), field).toThrow(RangeError);
      expect(() => encodeTailLogicalInput(Object.assign(Object.create({ [field]: neutral[field as keyof TailLogicalInput] }) as object, missing) as TailLogicalInput), field).toThrow(RangeError);
    }
    for (const command of [Object.create({ kind: "hold" }) as { kind: "hold" },
      Object.defineProperty({}, "kind", { value: "hold" }) as { kind: "hold" },
      Object.assign(Object.create({ normalized: 0.5 }) as object, { kind: "set" as const }) as { kind: "set"; normalized: number }]) {
      expect(() => encodeTailLogicalInput({ ...neutral, pilotPositionCommand: command })).toThrow(RangeError);
    }
    expect(() => encodeTailLogicalInput({ ...neutral, roll: 0 } as TailLogicalInput)).toThrow(RangeError);
  });

  it("does not invoke polluted global toJSON or inherit command fields", () => {
    const originals = new Map(["toJSON", "kind", "normalized"].map((key) => [key, Object.getOwnPropertyDescriptor(Object.prototype, key)]));
    let calls = 0;
    let encoded = "";
    try {
      Object.defineProperty(Object.prototype, "toJSON", { value: () => { calls += 1; return "redirected"; }, configurable: true });
      Object.defineProperty(Object.prototype, "kind", { value: "hold", configurable: true });
      Object.defineProperty(Object.prototype, "normalized", { value: 0.5, configurable: true });
      encoded = encodeTailLogicalInput(neutral);
      for (const command of [{}, { kind: "set" }]) expect(() => encodeTailLogicalInput({ ...neutral, pilotPositionCommand: command as TailLogicalInput["pilotPositionCommand"] })).toThrow(RangeError);
    } finally {
      for (const [key, descriptor] of originals) {
        if (descriptor === undefined) Reflect.deleteProperty(Object.prototype, key);
        else Object.defineProperty(Object.prototype, key, descriptor);
      }
    }
    expect(calls).toBe(0);
    expect(JSON.parse(encoded)).toHaveProperty("pilot_position_command", { kind: "hold" });
  });
});
