import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function preparedCountdown() {
  const bridge = new HybridGameSessionBridge(0, 21, 22);
  const session = new TailAppSessionFacade(bridge, physics_hz());
  session.executeSharedSessionCommand({ kind: "open_setup" });
  session.executeSharedSessionCommand({ kind: "prepare" });
  session.executeSharedSessionCommand({ kind: "start_countdown" });
  return { bridge, session };
}

describe("shared elapsed Countdown through generated WASM and the application owner", () => {
  it("retains three seconds after 800 ms, two after one second, and launches once after three seconds", () => {
    const { bridge, session } = preparedCountdown();
    const observe = vi.spyOn(bridge, "observe_countdown");
    try {
      const metadata = bridge.configuration_metadata();
      const token = session.captureCountdownToken();
      expect(session.observeCountdown(token, 0.8, true)).toEqual({ kind: "waiting", remainingSeconds: 3 });
      expect(session.readLifecycle().countdown_remaining_seconds).toBe(3);
      expect(session.observeCountdown(token, 0.8, true)).toEqual({ kind: "ignored" });
      expect(observe).toHaveBeenCalledTimes(1);
      expect(session.observeCountdown(session.captureCountdownToken(), 0, true)).toEqual({ kind: "waiting", remainingSeconds: 3 });
      expect(session.observeCountdown(session.captureCountdownToken(), 0.2, true)).toEqual({ kind: "waiting", remainingSeconds: 2 });
      const finalToken = session.captureCountdownToken();
      expect(session.observeCountdown(finalToken, 2, true)).toEqual({ kind: "launched" });
      expect(session.readLifecycle().phase).toBe("flight_running");
      const initial = bridge.snapshot_json();
      expect(session.observeCountdown(finalToken, 2, true)).toEqual({ kind: "ignored" });
      expect(session.captureCountdownToken()).toBeNull();
      expect(bridge.snapshot_json()).toBe(initial);
      expect(bridge.configuration_metadata()).toEqual(metadata);
      expect(observe.mock.results.filter((result) => result.type === "return" && result.value === 3)).toHaveLength(1);
    } finally { session.dispose(); }
  });

  it("rejects negative, nonfinite and overflowing elapsed without mutating the Rust phase or prepared state", () => {
    const { bridge, session } = preparedCountdown();
    try {
      const before = bridge.snapshot_json();
      const prepared = bridge.prepared_launch_snapshot_json();
      for (const elapsed of [-0.1, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.MAX_VALUE]) {
        expect(() => session.observeCountdown(session.captureCountdownToken(), elapsed, true)).toThrow();
        expect(bridge.snapshot_json()).toBe(before);
        expect(bridge.prepared_launch_snapshot_json()).toBe(prepared);
      }
      for (const elapsed of [0.15, 0.65, 0, 0.2, 1.75]) session.observeCountdown(session.captureCountdownToken(), elapsed, true);
      expect(session.readLifecycle().countdown_remaining_seconds).toBe(1);
      expect(session.observeCountdown(session.captureCountdownToken(), 0.25, true)).toEqual({ kind: "launched" });
    } finally { session.dispose(); }
  });

  it("invalidates cancelled attempts and stale owners without advancing a restarted Countdown", () => {
    const current = preparedCountdown();
    const other = preparedCountdown();
    try {
      const obsolete = current.session.captureCountdownToken();
      const rawToken = current.bridge.countdown_token();
      current.session.observeCountdown(obsolete, 0.8, true);
      current.session.executeSharedSessionCommand({ kind: "cancel_countdown" });
      current.session.executeSharedSessionCommand({ kind: "start_countdown" });
      const before = current.bridge.snapshot_json();
      expect(current.bridge.countdown_token()).not.toBe(rawToken);
      expect(current.bridge.observe_countdown(rawToken, 3, false)).toBe(0);
      expect(current.session.observeCountdown(obsolete, 3, true)).toEqual({ kind: "ignored" });
      expect(current.session.observeCountdown(other.session.captureCountdownToken(), 3, true)).toEqual({ kind: "ignored" });
      expect(current.bridge.snapshot_json()).toBe(before);
      expect(current.session.readLifecycle().countdown_remaining_seconds).toBe(3);
    } finally { current.session.dispose(); other.session.dispose(); }
  });

  it("cancels unavailable presentation frames independently of a normal one-second timer observation", () => {
    const { bridge, session } = preparedCountdown();
    try {
      const metadata = bridge.configuration_metadata();
      expect(session.observeCountdown(session.captureCountdownToken(), 1, true)).toEqual({ kind: "waiting", remainingSeconds: 2 });
      const obsolete = bridge.countdown_token();
      expect(session.observeCountdown(session.captureCountdownToken(), 2, false)).toEqual({ kind: "cancelled" });
      expect(session.readLifecycle().phase).toBe("briefing_ready");
      expect(bridge.configuration_metadata()).toEqual(metadata);
      expect(bridge.observe_countdown(obsolete, 3, true)).toBe(0);
      session.executeSharedSessionCommand({ kind: "start_countdown" });
      expect(session.observeCountdown(session.captureCountdownToken(), 0.8, true)).toEqual({ kind: "waiting", remainingSeconds: 3 });
    } finally { session.dispose(); }
  });
});
