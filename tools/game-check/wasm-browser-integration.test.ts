import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { keyboardIntent } from "../../web/src/game/keyboard-intent.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));

describe("generated WebAssembly browser binding", () => {
  it("runs a fixed-rate flight through fractional water contact", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    expect(physics_hz()).toBe(100);

    const session = new GameSessionBridge(0);
    try {
      expect(session.phase_code()).toBe(0);
      session.open_setup();
      expect(session.phase_code()).toBe(1);
      session.prepare();
      expect(session.phase_code()).toBe(2);
      session.fail_briefing(0);
      expect(session.phase_code()).toBe(8);
      session.retry_briefing();
      expect(session.phase_code()).toBe(2);
      session.cancel_briefing();
      expect(session.phase_code()).toBe(1);
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      expect(session.advance_countdown()).toBe(0);
      const initial = parseFlightSnapshot(session.launch());
      expect(initial.tick).toBe(0);
      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let index = 0; index < 3_000 && snapshot.terminal === "airborne"; index += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(Number.isInteger(snapshot.tick)).toBe(true);
      expect(snapshot.contactFraction).toBeGreaterThanOrEqual(0);
      expect(snapshot.contactFraction).toBeLessThanOrEqual(1);
      expect(snapshot.scoreCourseMeters).toBeGreaterThanOrEqual(200);
      expect(session.advance_tick(0, 0, 0, 0)).toEqual(session.snapshot());
      session.retry();
      expect(session.phase_code()).toBe(3);
      session.open_setup();
      session.return_to_title();
      expect(session.phase_code()).toBe(0);
    } finally {
      session.free();
    }
  });

  it("keeps a short pilot-position keyboard input within the playable glide range", () => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    try {
      session.open_setup();
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      session.advance_countdown();
      session.launch();

      let targetPositionMeters = 0;
      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        const pressed = tick >= 160 && tick < 170 ? new Set(["KeyJ"]) : new Set<string>();
        const intent = keyboardIntent(pressed, targetPositionMeters, physics_hz());
        targetPositionMeters = intent.pilotPositionMeters;
        snapshot = parseFlightSnapshot(session.advance_tick(
          intent.roll,
          intent.pitch,
          intent.yaw,
          intent.pilotPositionMeters
        ));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(snapshot.scoreCourseMeters).toBeGreaterThanOrEqual(180);
      expect(snapshot.scoreCourseMeters).toBeLessThanOrEqual(230);
      expect(Math.abs(snapshot.pilotPositionMeters)).toBeGreaterThan(0.03);
    } finally {
      session.free();
    }
  });

  it.each([
    { mode: 0, label: "Manual" },
    { mode: 1, label: "Shared" },
    { mode: 2, label: "Automatic" }
  ])("runs the $label control mode through pause, water contact, and retry", ({ mode }) => {
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
    const session = new GameSessionBridge(0);
    try {
      session.open_setup();
      session.set_control_mode(mode);
      session.prepare();
      session.mark_briefing_ready();
      session.start_countdown(1);
      session.advance_countdown();
      session.launch();

      session.pause(0);
      expect(session.phase_code()).toBe(6);
      session.resume();
      expect(session.phase_code()).toBe(5);

      let snapshot = parseFlightSnapshot(session.snapshot());
      for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick += 1) {
        snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
      }

      expect(snapshot.terminal).toBe("water-contact");
      expect(session.phase_code()).toBe(7);
      session.retry();
      expect(session.phase_code()).toBe(3);
    } finally {
      session.free();
    }
  });
});
