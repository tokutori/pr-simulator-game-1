import { readFileSync } from "node:fs";
import { Window } from "happy-dom";
import { PerspectiveCamera, StereoCamera } from "three";
import { describe, expect, it } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import type { TailSnapshotDocument } from "../../web/src/generated/tail-snapshot.js";
import type { RecordedPlaybackSample } from "../../web/src/generated/record-sample.js";
import type { GameUiView } from "../../web/src/generated/game-ui-view.js";
import { readContract } from "../game-ui-build/validate.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-input.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
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

function display(session: HybridGameSessionBridge): TailSnapshotDocument {
  return readContract<TailSnapshotDocument>("tail_snapshot", session.snapshot_json());
}

function advance(session: HybridGameSessionBridge): void {
  session.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0.5, turnRight: -0.5,
    desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0,
    pilotPositionCommand: { kind: "set", normalized: 0.1 } }));
}

function sharedHud(session: HybridGameSessionBridge, head = false) {
  const view = readContract<GameUiView>("view", session.game_ui_view_json(JSON.stringify(sharedGameUiPlatform(head ? { presentation_mode: "phone_vr" } : {}))));
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
      if (snapshot.frame.kind !== "flight") {
        throw new Error("Expected Rust two-tail fields");
      }
      const model = sharedHud(session);
      const rate = snapshot.frame.state.angular_rate_body_rad_s;
      const degrees = (value: number): string => (value * 180 / Math.PI).toFixed(1);
      const attitude = sharedHudNode(model, "hud-attitude").content;
      expect(attitude.kind === "attitude" && attitude.geometry).toMatchObject({ kind: "attitude", horizon: {
        rotation_degrees: -snapshot.frame.telemetry.attitude_euler_rad[0] * 180 / Math.PI
      } });
      const readouts = model.groups.flatMap(group => group.nodes).map(node => `${node.label} ${node.content.value}`).join("\n");
      expect(sharedHudNode(model, "hud-angular-rates").content.value).toBe(`${degrees(rate[0])}/${degrees(rate[1])}/${degrees(rate[2])} °/s`);
      expect(sharedHudNode(model, "hud-tail-incidence").content.value)
        .toBe(`${degrees(snapshot.frame.state.physical_incidence.horizontal_tail_rad)}/${degrees(snapshot.frame.state.physical_incidence.vertical_tail_rad)}°`);
      expect(sharedHudNode(model, "hud-pilot-position").content.value).toContain(snapshot.frame.state.pilot_position_target_m.toFixed(2));
      expect(sharedHudNode(model, "hud-distance").content.value).toBe(`${snapshot.frame.progress_m.course_parallel_m.toFixed(1)} m`);
      expect(readouts).not.toContain("確定距離");
      expect(sharedHudNode(model, "hud-pilot-position").content.value).toContain(snapshot.frame.state.pilot_position_m.toFixed(2));
      expect(model.groups.flatMap((group) => group.nodes).some((node) => node.id === "hud-controls")).toBe(false);
      expect(readouts).not.toMatch(/intent|Hold|Set/);
      session.pause(0);
      expect(sharedHudNode(sharedHud(session), "hud-primary-readouts").content).toEqual(sharedHudNode(model, "hud-primary-readouts").content);
    } finally {
      session.free();
    }
  });

  it("keeps Information and Custom visibility independent from the physical controls", () => {
      const controls = withSharedGameUiSession("flight_running", { ticks: 0 }, (owner) => owner.readSnapshot());
      expect(currentSharedHudFixture({ informationCode: 2, ticks: 0 }).groups.flatMap(group => group.nodes).some(node => node.id === "hud-tail-incidence")).toBe(false);
      const attitudeProfile = { telemetry: false, attitude: true,
        wind: false, flight_path: false, angle_of_attack: false, warnings: false };
      const attitudeOnly = currentSharedHudFixture({ informationCode: 4, hudProfile: attitudeProfile, ticks: 0 });
      expect(sharedHudNode(attitudeOnly, "hud-angular-rates").content.value).toContain("°/s");
      const telemetryProfile = { telemetry: true, attitude: false,
        wind: false, flight_path: false, angle_of_attack: false, warnings: false };
      const telemetryOnly = currentSharedHudFixture({ informationCode: 4, hudProfile: telemetryProfile, ticks: 0 });
      expect(sharedHudNode(telemetryOnly, "hud-tail-incidence").content.value).toContain("°");
      expect(telemetryOnly.groups.flatMap(group => group.nodes).some(node => node.id === "hud-angular-rates")).toBe(false);
      for (const options of [{ informationCode: 2 as const }, { informationCode: 4 as const, hudProfile: attitudeProfile },
        { informationCode: 4 as const, hudProfile: telemetryProfile }]) {
        withSharedGameUiSession("flight_running", { ...options, ticks: 0 }, (owner) => {
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
      expect(root.querySelector('[data-node="hud-tail-incidence"] output')?.textContent).toBe(sharedHudNode(model, "hud-tail-incidence").content.value);
      expect(root.querySelector('[data-node="hud-controls"]')).toBeNull();
      expect(root.querySelector('[data-node="hud-pilot-position"] output')?.textContent).toBe(sharedHudNode(model, "hud-pilot-position").content.value);
      adapter.setVisible(false);
      adapter.render(currentSharedHudFixture({ informationCode: 2 }));
      expect(root.querySelector('[data-node="hud-tail-incidence"]')).toBeNull();
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
      const readouts = view.layer.elements.find((element) => element.id === "hud-tail-incidence");
      expect(readouts?.content).toEqual(sharedHudNode(model, "hud-tail-incidence").content);
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
      const terminalView = readContract<GameUiView>("view", source.game_ui_view_json(JSON.stringify(sharedGameUiPlatform())));
      expect(terminalView.hud).toBeNull();
      expect(terminalView.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-result-terminal")?.content)
        .toMatchObject({ kind: "status", value: "操作により飛行を終了した" });
      expect(result.phase).toBe("result");
      replay.open_archived_flight_record(source.export_flight_record_json());
      const snapshot = readContract<RecordedPlaybackSample>("record_sample", replay.flight_record_sample_at_seconds(0.005));
      replay.game_ui_action_json(JSON.stringify({ kind: "set_replay_view", view: "telemetry" }), JSON.stringify(sharedGameUiPlatform()));
      expect(readContract<GameUiView>("view", replay.game_ui_view_json(JSON.stringify(sharedGameUiPlatform()))).selections.replay_view).toBe("telemetry");
      replay.seek_playback(0.005);
      const model = sharedHud(replay);
      expect(sharedHudNode(model, "hud-distance").content.value).toBe("—");
      expect(sharedHudNode(model, "hud-pilot-position").content.value).toContain("(—) m");
      expect(sharedHudNode(model, "hud-elapsed").content.value).toBe(`${snapshot.flight_time_s.toFixed(1)} s`);
      expect(snapshot.flight_time_s).toBe(0.005);
    } finally {
      source.free();
      replay.free();
    }
  });
});
