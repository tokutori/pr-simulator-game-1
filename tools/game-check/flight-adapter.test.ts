import { describe, expect, it } from "vitest";
import { FixedTickClock } from "../../web/src/game/fixed-tick-clock.js";
import { FLIGHT_SNAPSHOT_LENGTH, parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { keyboardIntent } from "../../web/src/game/keyboard-intent.js";
import { BrowserPilotInput, gamepadIntent } from "../../web/src/game/browser-input.js";
import { FlightController } from "../../web/src/game/flight-controller.js";
import type { FlightHudPort, FlightPosePort, FlightSessionPort, PilotInputPort } from "../../web/src/game/flight-controller.js";
import type { FlightRenderPose } from "../../web/src/render/contracts/runtime.js";
import type { FlightSnapshot } from "../../web/src/game/flight-snapshot.js";

describe("browser flight adapters", () => {
  it("advances identical physics tick counts at different render rates", () => {
    const tickCounts = [30, 60, 120].map((framesPerSecond) => {
      const clock = new FixedTickClock(10);
      let ticks = 0;
      for (let frame = 0; frame <= framesPerSecond * 5; frame += 1) {
        clock.advanceFrame(frame * 1_000 / framesPerSecond, () => { ticks += 1; });
      }
      return ticks;
    });

    expect(tickCounts).toEqual([500, 500, 500]);
  });

  it("does not accumulate hidden-page time across suspend and resume", () => {
    const clock = new FixedTickClock(10);
    let ticks = 0;
    clock.advanceFrame(0, () => { ticks += 1; });
    clock.advanceFrame(5, () => { ticks += 1; });
    clock.suspend();
    clock.advanceFrame(10_000, () => { ticks += 1; });
    clock.resume();
    clock.advanceFrame(20_000, () => { ticks += 1; });
    clock.advanceFrame(20_005, () => { ticks += 1; });

    expect(ticks).toBe(1);
  });

  it("maps keyboard state into bounded device-independent pilot intent", () => {
    expect(keyboardIntent(new Set(["KeyD", "ArrowUp", "KeyJ"]))).toEqual({
      roll: 1,
      pitch: 1,
      yaw: 0,
      pilotPositionMeters: -0.004
    });
    expect(keyboardIntent(new Set(["KeyD", "KeyA"])).roll).toBe(0);
    const heldTarget = keyboardIntent(new Set(["KeyL"])).pilotPositionMeters;
    expect(heldTarget).toBe(0.004);
    expect(keyboardIntent(new Set(), heldTarget).pilotPositionMeters).toBe(heldTarget);
    expect(keyboardIntent(new Set(["KeyL"]), 0.4).pilotPositionMeters).toBe(0.4);
  });

  it("uses a connected standard gamepad with dead-zone normalization", () => {
    const gamepad = { connected: true, axes: [0.54, -0.54, 0.0, 0.5] } as unknown as Gamepad;
    expect(gamepadIntent([gamepad])).toEqual({
      roll: 0.5,
      pitch: 0.5,
      yaw: 0,
      pilotPositionMeters: -((0.5 - 0.08) / 0.92) * 0.4
    });
    expect(gamepadIntent([null])).toBeNull();
  });

  it("requires a neutral gamepad sample after connection and reconnection", () => {
    const target = { addEventListener() {}, removeEventListener() {} } as unknown as Window;
    const input = new BrowserPilotInput(target);
    const activeGamepad = { connected: true, axes: [0.54, -0.54, 0.0, 0.5] } as unknown as Gamepad;
    const neutralGamepad = { connected: true, axes: [0, 0, 0, 0] } as unknown as Gamepad;

    expect(input.readIntent([activeGamepad])).toEqual({
      roll: 0,
      pitch: 0,
      yaw: 0,
      pilotPositionMeters: 0
    });
    input.readIntent([neutralGamepad]);
    const activeIntent = input.readIntent([activeGamepad]);
    expect(activeIntent.roll).toBe(0.5);
    expect(activeIntent.pilotPositionMeters).toBeLessThan(0);

    const disconnectedIntent = input.readIntent([null]);
    expect(disconnectedIntent.pilotPositionMeters).toBe(activeIntent.pilotPositionMeters);
    const reconnectIntent = input.readIntent([activeGamepad]);
    expect(reconnectIntent).toEqual(disconnectedIntent);
    expect(input.readIntent([neutralGamepad]).pilotPositionMeters).toBe(0);
    input.dispose();
  });

  it("connects fixed-rate frames to atomic WASM ticks and pauses without catch-up", () => {
    const session = new FakeFlightSession();
    const renderer = new FakeFlightRenderer();
    const input = new FakePilotInput();
    const hud = new FakeFlightHud();
    const controller = new FlightController(session, input, renderer, hud, 100, () => []);

    controller.onFrame(0);
    controller.onFrame(5);
    expect(session.tick).toBe(0);
    controller.onFrame(10);
    expect(session.tick).toBe(1);
    expect(renderer.pose?.datumPositionNed.north).toBe(1);
    expect(hud.snapshot?.tick).toBe(1);

    controller.suspend();
    controller.onFrame(10_000);
    controller.resume();
    controller.onFrame(20_000);
    controller.onFrame(20_010);
    expect(session.tick).toBe(2);
    controller.dispose();
    expect(session.freed).toBe(true);
    expect(input.disposed).toBe(true);
    expect(renderer.pose).toBeNull();
  });

  it("parses and validates the packed Rust snapshot contract", () => {
    const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
    values[0] = 12;
    values[7] = 1;
    values[16] = 1;
    values[19] = 0.25;
    values[20] = 12.5;
    values[21] = 10.2;
    values[22] = 10.4;
    values[26] = 0.03;
    values[31] = 1;
    values[32] = 12.125;
    const parsed = parseFlightSnapshot(values);

    expect(parsed.tick).toBe(12);
    expect(parsed.terminal).toBe("water-contact");
    expect(parsed.contactFraction).toBe(0.25);
    expect(parsed.telemetry?.altitudeMeters).toBe(12.5);
    expect(parsed.telemetry?.airspeedMetersPerSecond).toBe(10.2);
    expect(parsed.telemetry?.groundspeedMetersPerSecond).toBe(10.4);
    expect(parsed.telemetry?.angleOfAttackRadians).toBe(0.03);
    expect(parsed.flightTimeSeconds).toBe(12.125);
    expect(() => parseFlightSnapshot([...values.slice(0, 31), 2])).toThrow(RangeError);
    expect(Object.isFrozen(parsed.positionNed)).toBe(true);

    expect(() => parseFlightSnapshot(values.slice(1))).toThrow(RangeError);
    values[19] = 1.5;
    expect(() => parseFlightSnapshot(values)).toThrow(RangeError);
  });

  it("maps every typed terminal reason from the packed snapshot", () => {
    const terminalCases = [
      [0, "airborne"],
      [1, "water-contact"],
      [2, "time-limit"],
      [3, "out-of-valid-envelope"],
      [4, "manual-abort"],
      [5, "fatal-simulation-error"]
    ] as const;

    for (const [code, expected] of terminalCases) {
      const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
      values[7] = 1;
      values[16] = code;
      values[19] = code === 1 ? 0.5 : -1;
      expect(parseFlightSnapshot(values).terminal).toBe(expected);
    }
  });

  it("keeps contact tick integral and represents sub-tick time separately", () => {
    const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
    values[0] = 12;
    values[7] = 1;
    values[16] = 1;
    values[19] = 0.25;
    const parsed = parseFlightSnapshot(values);

    expect(parsed.tick).toBe(12);
    expect(parsed.contactFraction).toBe(0.25);
  });
});

class FakeFlightSession implements FlightSessionPort {
  tick = 0;
  freed = false;

  advance_tick(): ArrayLike<number> {
    this.tick += 1;
    const values = snapshotValues();
    values[0] = this.tick;
    values[1] = this.tick;
    return values;
  }

  snapshot(): ArrayLike<number> {
    return snapshotValues();
  }

  free(): void {
    this.freed = true;
  }
}

class FakePilotInput implements PilotInputPort {
  disposed = false;

  readIntent() {
    return { roll: 0, pitch: 0, yaw: 0, pilotPositionMeters: 0 };
  }

  dispose(): void {
    this.disposed = true;
  }
}

class FakeFlightRenderer implements FlightPosePort {
  pose: FlightRenderPose | null = null;

  setFlightPose(pose: FlightRenderPose | null): void {
    this.pose = pose;
  }
}

class FakeFlightHud implements FlightHudPort {
  snapshot: FlightSnapshot | null = null;

  render(snapshot: FlightSnapshot): void {
    this.snapshot = snapshot;
  }

  fail(): void {}
  setVisible(): void {}
}

function snapshotValues(): number[] {
  const values = new Array<number>(FLIGHT_SNAPSHOT_LENGTH).fill(0);
  values[7] = 1;
  values[19] = -1;
  return values;
}
