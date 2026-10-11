import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Window as BrowserWindow } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { installBrowserPageLifecycle } from "../../web/src/app/browser-page-lifecycle.js";
import { TailFlightController } from "../../web/src/game/tail-flight-controller.js";
import { parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";
import { suspendPageFlight } from "../../web/src/game/page-flight-lifecycle.js";

const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));

function preparedSession(): HybridGameSessionBridge {
  initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  const session = new HybridGameSessionBridge(0, 21, 22);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  return session;
}

describe("browser cache to Rust session boundary", () => {
  it("retains the same Flight session across two cache round trips without catch-up", async () => {
    const window = new BrowserWindow();
    const session = preparedSession();
    session.advance_countdown();
    session.launch();
    const free = vi.spyOn(session, "free");
    const readState = () => {
      const snapshot = parseTailSessionSnapshot(session.snapshot_json(), physics_hz());
      if (snapshot.frame.kind !== "flight") throw new Error("Expected a retained current flight state");
      return snapshot.frame.state;
    };
    const input = {
      readDemand: () => ({ controlLayout: "tail_incidence" as const, noseUp: 0, turnRight: 0, pilotPositionCommand: { kind: "hold" as const } }),
      reset() {}, suspend() {}, resume() {}, dispose() {}
    };
    const controller = new TailFlightController(session, input, { setFlightPose() {} }, {
      render() {}, setVisible() {}, fail(message) { throw new Error(message); }
    }, physics_hz(), () => []);
    const synchronize = vi.fn();
    installBrowserPageLifecycle(window as unknown as Window, {
      suspend: () => { suspendPageFlight(session, controller, () => {}, synchronize); },
      restore: () => { session.clear_pause_reason(1); synchronize(); },
      dispose: () => { controller.dispose(); }
    });
    const event = (type: string, persisted: boolean): void => {
      const transition = new window.Event(type);
      Object.defineProperty(transition, "persisted", { value: persisted });
      window.dispatchEvent(transition);
    };
    try {
      controller.onFrame(0);
      controller.onFrame(10);
      for (let roundTrip = 0; roundTrip < 2; roundTrip += 1) {
        const before = readState();
        event("pagehide", true);
        expect(session.phase_code()).toBe(6);
        controller.onFrame(40_000 + roundTrip * 50_000);
        event("pageshow", true);
        expect(session.phase_code()).toBe(6);
        expect(session.can_resume()).toBe(true);
        expect(readState()).toEqual(before);
        expect(free).not.toHaveBeenCalled();
        session.resume();
        controller.resume();
        const baseline = 40_010 + roundTrip * 50_000;
        controller.onFrame(baseline);
        expect(readState()).toEqual(before);
        controller.onFrame(baseline + 10);
        expect(controller.currentSnapshot.frame.state.tick).toBe(roundTrip + 2);
      }
      event("pagehide", false);
      event("pagehide", false);
      expect(free).toHaveBeenCalledTimes(1);
      expect(synchronize).toHaveBeenCalledTimes(4);
    } finally {
      controller.dispose();
      await window.happyDOM.abort();
    }
  });

  it("cancels Countdown before caching even without a visibilitychange event", async () => {
    const window = new BrowserWindow();
    const session = preparedSession();
    const invalidate = vi.fn();
    const synchronize = vi.fn();
    installBrowserPageLifecycle(window as unknown as Window, {
      suspend: () => { suspendPageFlight(session, null, invalidate, synchronize); },
      restore: synchronize,
      dispose: () => { session.free(); }
    });
    try {
      const event = new window.Event("pagehide");
      Object.defineProperty(event, "persisted", { value: true });
      window.dispatchEvent(event);
      expect(session.phase_code()).toBe(3);
      expect(invalidate).toHaveBeenCalledOnce();
      expect(synchronize).toHaveBeenCalledOnce();
      expect(() => session.advance_countdown()).toThrow();
    } finally {
      window.dispatchEvent(new window.Event("pagehide"));
      await window.happyDOM.abort();
    }
  });
});
