import { describe, expect, it } from "vitest";
import { DEFAULT_TAIL_GAMEPAD_CONFIGURATION, DEFAULT_TAIL_KEYBOARD_CONFIGURATION,
  tailGamepadDemand, tailInputWithRateDemand, tailKeyboardDemand } from "../../web/src/game/tail-device-input.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

describe("two-tail device-independent demand", () => {
  it("maps explicit two-axis keyboard bindings and leaves old roll keys unassigned", () => {
    const { demand, normalizedPilotTarget } = tailKeyboardDemand(new Set(["ArrowUp", "ArrowRight", "KeyL"]));
    expect(demand).toEqual({ controlLayout: "tail_incidence", noseUp: 1, turnRight: 1,
      pilotPositionCommand: { kind: "set", normalized: 0.01 } });
    expect(normalizedPilotTarget).toBe(0.01);
    expect(tailKeyboardDemand(new Set(["KeyA", "KeyD"])).demand).toEqual({ controlLayout: "tail_incidence",
      noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" } });
    expect(tailKeyboardDemand(new Set(["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "KeyJ", "KeyL"])).demand)
      .toEqual({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" } });
    const released = tailKeyboardDemand(new Set(), normalizedPilotTarget);
    expect(released.normalizedPilotTarget).toBe(normalizedPilotTarget);
    expect(released.demand.pilotPositionCommand).toEqual({ kind: "hold" });
    expect(Object.isFrozen(released.demand)).toBe(true);
  });

  it("edits only normalized device target and uses the caller's physics tick rate", () => {
    const at50 = tailKeyboardDemand(new Set(["KeyJ"]), 0, { ...DEFAULT_TAIL_KEYBOARD_CONFIGURATION, physicsHz: 50 });
    const at100 = tailKeyboardDemand(new Set(["KeyJ"]));
    expect(at50.normalizedPilotTarget).toBe(-0.02);
    expect(at100.normalizedPilotTarget).toBe(-0.01);
    expect(tailKeyboardDemand(new Set(["KeyL"]), 1).normalizedPilotTarget).toBe(1);
    expect(tailKeyboardDemand(new Set(["KeyJ"]), -1).normalizedPilotTarget).toBe(-1);
    expect(() => tailKeyboardDemand(new Set(), 1.01)).toThrow(RangeError);
    expect(() => tailKeyboardDemand(new Set(), 0, { ...DEFAULT_TAIL_KEYBOARD_CONFIGURATION, physicsHz: 0 })).toThrow(RangeError);
  });

  it("uses a separate named gamepad layout with normalized absolute position", () => {
    const demand = tailGamepadDemand([null, { connected: false, axes: [1, 1, 1, 1] },
      { connected: true, axes: [0.54, -0.54, 1, -0.54] }]);
    expect(demand).toEqual({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: 0.5,
      pilotPositionCommand: { kind: "set", normalized: 0.5 } });
    expect(tailGamepadDemand([{ connected: true, axes: [0, 0, 1, 0] }])).toEqual({ controlLayout: "tail_incidence",
      noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "set", normalized: 0 } });
    expect(tailGamepadDemand([{ connected: true, axes: [5, -5, 0, 0] }])?.turnRight).toBe(1);
    expect(tailGamepadDemand([{ connected: true, axes: [NaN, Infinity] }])?.noseUp).toBe(0);
    expect(tailGamepadDemand([])).toBeNull();
    expect(() => tailGamepadDemand([], { ...DEFAULT_TAIL_GAMEPAD_CONFIGURATION, deadZone: 1 })).toThrow(RangeError);
    expect(() => tailGamepadDemand([], { ...DEFAULT_TAIL_GAMEPAD_CONFIGURATION,
      axes: { noseUp: 0, turnRight: 0, pilotPosition: 3 } })).toThrow(RangeError);
  });

  it("passes typed caller-supplied q/r demand through without deriving controller parameters", () => {
    const demand = tailKeyboardDemand(new Set(["ArrowUp"])).demand;
    const input = tailInputWithRateDemand(demand, { pitchRadiansPerSecond: 0.017, yawRadiansPerSecond: -0.009 });
    expect(input.desiredPitchRateRadiansPerSecond).toBe(0.017);
    expect(input.desiredYawRateRadiansPerSecond).toBe(-0.009);
    expect(JSON.parse(encodeTailLogicalInput(input))).toMatchObject({ desired_pitch_rate_rad_s: 0.017,
      desired_yaw_rate_rad_s: -0.009, nose_up: 1, turn_right: 0, pilot_position_command: { kind: "hold" } });
    expect(input).not.toHaveProperty("roll");
    expect(input).not.toHaveProperty("pilotPositionMeters");
    expect(() => tailInputWithRateDemand(demand, { pitchRadiansPerSecond: NaN, yawRadiansPerSecond: 0 })).toThrow(RangeError);
  });
});
