import { readFileSync } from "node:fs";
import { Window } from "happy-dom";
import { PerspectiveCamera, StereoCamera } from "three";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { projectRecordedFlightSnapshot, projectTailFlightSnapshot } from "../../web/src/game/flight-display-snapshot.js";
import type { FlightDisplaySnapshot } from "../../web/src/game/flight-display-snapshot.js";
import { parseNamedRecordSample, parseNamedReplayContext } from "../../web/src/game/named-record-query.js";
import { encodeTailLogicalInput, parseTailSessionSnapshot } from "../../web/src/game/tail-session-codec.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { parseGameUiView } from "../../web/src/app/game-ui-codec.js";
import { currentSharedHudFixture, sharedHudNode, sharedGameUiPlatform, withSharedGameUiSession } from "./shared-game-ui-fixture.js";
import { createHeadHudView } from "../../web/src/presentation/head-hud-view.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

function launch(session: HybridGameSessionBridge): void {
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.observe_countdown(session.countdown_token(), 1, true);
}

function display(session: HybridGameSessionBridge): FlightDisplaySnapshot {
  const projected = projectTailFlightSnapshot(parseTailSessionSnapshot(session.snapshot_json()));
  if (projected.kind !== "available") throw new Error("Expected a flight display");
  return projected.value;
}

function advance(session: HybridGameSessionBridge): void {
  session.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: -0.5,
    desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0,
    pilotPositionCommand: { kind: "set", normalized: 0.1 } }));
}

function sharedHud(session: HybridGameSessionBridge, head = false) {
  const view = parseGameUiView(session.game_ui_view_json(JSON.stringify(sharedGameUiPlatform(head ? { presentation_mode: "phone_vr" } : {}))));
  if (view.hud === null) throw new Error("Missing shared Rust HUD");
  return projectSharedFlightHudModel(view.hud, view.theme);
}

describe("tail-aware shared Flight HUD", () => {
  it("displays Rust attitude, body rates, physical tails, held target and signed live progress", () => {
    const session = new HybridGameSessionBridge(0, 21, 22);
    try {
      launch(session);
      advance(session);
      const snapshot = display(session);
      if (snapshot.kind !== "tail_flight") {
        throw new Error("Expected Rust two-tail fields");
      }
      const model = sharedHud(session);
      const rate = snapshot.angularRateBodyRadiansPerSecond.value;
      const degrees = (value: number): string => (value * 180 / Math.PI).toFixed(1);
      const attitude = sharedHudNode(model, "hud-attitude").content;
      expect(attitude.kind === "attitude" && attitude.geometry).toMatchObject({ kind: "attitude", horizon: {
        rotation_degrees: -snapshot.telemetry.value.rollRadians * 180 / Math.PI
      } });
      const readouts = sharedHudNode(model, "hud-primary-readouts").content.value;
      for (const line of [
        `p ${degrees(rate.roll)}  q ${degrees(rate.pitch)}  r ${degrees(rate.yaw)} °/s`,
        `水平尾翼 ${degrees(snapshot.controls.physicalIncidence.horizontalTailRadians)}°  垂直尾翼 ${degrees(snapshot.controls.physicalIncidence.verticalTailRadians)}°`,
        `PILOT TARGET ${snapshot.pilotPositionTargetMeters.value.toFixed(2)} m`
      ]) expect(readouts).toContain(line);
      expect(readouts).toContain(`距離 ${snapshot.progressMeters.value.courseParallelMeters.toFixed(1)} m`);
      expect(readouts).not.toContain("確定距離");
      expect(sharedHudNode(model, "hud-pilot-position").content.value).toContain(snapshot.pilotPositionMeters.toFixed(2));
      const controls = sharedHudNode(model, "hud-controls").content.value;
      expect(controls).toContain("nose-up/down intent");
      expect(controls).toContain("←/→ left/right intent");
      expect(controls).toContain("pilot Hold");
      expect(controls).not.toContain("A/D roll");
      expect(Object.isFrozen(model.groups)).toBe(true);
      session.pause(0);
      expect(sharedHudNode(sharedHud(session), "hud-primary-readouts").label).toBe("一時停止");
    } finally {
      session.free();
    }
  });

  it("keeps Information and Custom visibility independent from the physical controls", () => {
      const controls = withSharedGameUiSession(5, { ticks: 0 }, (owner) => owner.readSnapshot());
      expect(sharedHudNode(currentSharedHudFixture({ informationCode: 2, ticks: 0 }), "hud-primary-readouts").content.value).not.toContain("水平尾翼");
      const attitudeProfile = { telemetry: false, attitude: true,
        wind: false, flightPath: false, angleOfAttack: false, warnings: false };
      const attitudeOnly = currentSharedHudFixture({ informationCode: 4, hudProfile: attitudeProfile, ticks: 0 });
      expect(sharedHudNode(attitudeOnly, "hud-primary-readouts").content.value).toMatch(/^p /);
      const telemetryProfile = { telemetry: true, attitude: false,
        wind: false, flightPath: false, angleOfAttack: false, warnings: false };
      const telemetryOnly = currentSharedHudFixture({ informationCode: 4, hudProfile: telemetryProfile, ticks: 0 });
      expect(sharedHudNode(telemetryOnly, "hud-primary-readouts").content.value).toContain("水平尾翼 ");
      expect(sharedHudNode(telemetryOnly, "hud-primary-readouts").content.value).not.toMatch(/^p /);
      for (const options of [{ informationCode: 2 as const }, { informationCode: 4 as const, hudProfile: attitudeProfile },
        { informationCode: 4 as const, hudProfile: telemetryProfile }]) {
        withSharedGameUiSession(5, { ...options, ticks: 0 }, (owner) => {
          expect(owner.readSnapshot()).toEqual(controls);
        });
      }
  });

  it.each([1280 / 720, 720 / 1280])("uses the same tail model for DOM and camera-anchored Head HUD at aspect %s", async (aspect) => {
    const session = new HybridGameSessionBridge(0, 21, 22);
    const window = new Window();
    try {
      launch(session);
      advance(session);
      const model = sharedHud(session);
      const root = window.document.createElement("section") as unknown as HTMLElement;
      const adapter = new FlightHudAdapter(root);
      adapter.render(model);
      const nodes = [...root.children];
      expect(root.querySelector('[data-node="hud-primary-readouts"] output')?.textContent).toContain("水平尾翼");
      expect(root.querySelector('[data-node="hud-controls"] output')?.textContent).toBe(sharedHudNode(model, "hud-controls").content.value);
      expect(root.querySelector('[data-node="hud-pilot-position"] output')?.textContent).toBe(sharedHudNode(model, "hud-pilot-position").content.value);
      adapter.setVisible(false);
      adapter.render(currentSharedHudFixture({ informationCode: 2 }));
      expect(root.querySelector('[data-node="hud-primary-readouts"] output')?.textContent).not.toContain("水平尾翼");
      adapter.setVisible(true);
      adapter.render(model);
      expect([...root.children]).toEqual(nodes);

      const camera = new PerspectiveCamera(60, aspect, 0.05, 100);
      camera.updateMatrixWorld(true);
      const stereo = new StereoCamera();
      stereo.aspect = 0.5;
      const headModel = sharedHud(session, true);
      const view = createHeadHudView(headModel, captureConfiguredViewerFrame(camera, stereo), "ja");
      expect(view.kind).toBe("visible");
      if (view.kind !== "visible") throw new Error("Expected tail Head HUD");
      expect(view.layer.anchor).toBe("head");
      const readouts = view.layer.elements.find((element) => element.id === "hud-primary-readouts");
      expect(readouts?.content.value).toContain("水平尾翼");
      expect(view.layer.elements.find((element) => element.id === "hud-pilot-position")?.content).toEqual(sharedHudNode(model, "hud-pilot-position").content);
      expect(view.layer.clearRegion.height).toBeGreaterThan(0);
    } finally {
      session.free();
      await window.happyDOM.abort();
    }
  });

  it("keeps archive cursor time separate from terminal score and unavailable held target", () => {
    const source = new HybridGameSessionBridge(0, 21, 22);
    const replay = new HybridGameSessionBridge(0, 31, 32);
    try {
      launch(source);
      advance(source);
      source.abort();
      const result = display(source);
      const terminalView = parseGameUiView(source.game_ui_view_json(JSON.stringify(sharedGameUiPlatform())));
      expect(terminalView.hud).toBeNull();
      expect(terminalView.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-result-terminal")?.content)
        .toMatchObject({ kind: "status", value: "操作により飛行を終了した" });
      expect(result.kind).toBe("tail_result");
      replay.open_archived_flight_record(source.export_flight_record_json());
      const context = parseNamedReplayContext(replay.playback_context_json());
      const sample = parseNamedRecordSample(replay.flight_record_sample_at_seconds(0.005), physics_hz(), context);
      const snapshot = projectRecordedFlightSnapshot(sample, context);
      replay.game_ui_action_json(JSON.stringify({ kind: "set_replay_view", view: "telemetry" }), JSON.stringify(sharedGameUiPlatform()));
      expect(parseGameUiView(replay.game_ui_view_json(JSON.stringify(sharedGameUiPlatform()))).selections.replay_view).toBe("telemetry");
      replay.seek_playback(0.005);
      const model = sharedHud(replay);
      const readouts = sharedHudNode(model, "hud-primary-readouts");
      expect(readouts.label).toBe("記録再生");
      expect(readouts.content.value).toContain("保存標本");
      expect(readouts.content.value).not.toContain("確定距離");
      expect(readouts.content.value).toContain("PILOT TARGET unavailable");
      expect(snapshot.stamp.timeSeconds).toBe(0.005);
    } finally {
      source.free();
      replay.free();
    }
  });
});
