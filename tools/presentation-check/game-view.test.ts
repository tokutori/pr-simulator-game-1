import { describe, expect, it } from "vitest";
import { currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { controlInputAction } from "../../web/src/render/contracts/ui.js";

import { NO_HEAD_HUD } from "../../web/src/render/contracts/head-hud.js";
import type { GameUiAction } from "../../web/src/generated/game-ui-action.js";
describe("Shared Rust game View projection", () => {
  it.each(["flight_setup", "result"] as const)("projects shared metadata and widget payloads in %s", (phase) => {
    withSharedGameUiSession(phase, { actions: phase === "result" ? [{ kind: "set_result_tab", tab: "analysis" }] : [] }, (session, platform) => {
      const shared = session.readSharedUi(platform);
      const before = JSON.stringify(shared);
      const view = createGameViewModel(shared);
      expect(view).toMatchObject({ scene: shared.scene, title: shared.title, description: shared.description, activeOverlay: shared.active_overlay });
      expect(view.sharedPresentation?.layout).toBe(shared.layout);
      expect(view.sharedPresentation?.theme).toBe(shared.theme);
      expect(view.headHud).toBe(NO_HEAD_HUD);
      expect(view.panels.map((panel) => panel.id)).toEqual(shared.panels.map((panel) => panel.id));
      shared.panels.forEach((panel, index) => {
        const projected = view.panels[index];
        if (projected === undefined) throw new Error("Missing projected panel");
        expect(projected.anchor).toBe(panel.anchor);
        expect(projected.sections).toBe(panel.sections);
        expect(projected.size).toEqual({ width: panel.size[0], height: panel.size[1] });
        expect(projected.localPose.position).toEqual({ x: panel.local_position[0], y: panel.local_position[1], z: panel.local_position[2] });
        expect(projected.controls.map((control) => [control.id, control.label, control.enabled, control.section]))
          .toEqual(panel.controls.map((control) => [control.id, control.label, control.enabled, control.section]));
        panel.controls.forEach((control, controlIndex) => {
          const output = projected.controls[controlIndex];
          expect(output?.kind).toBe(control.content.kind);
          expect(output?.rect).toEqual(control.rect);
          if (output?.kind === "status" && control.content.kind === "status") expect(output.value).toBe(control.content.value);
          if (output?.kind === "range" && control.content.kind === "range") {
            expect(output.value).toBe(control.content.value);
            expect(output.displayValue).toBe(control.content.display_value);
          }
          if (output?.kind === "chart" && control.content.kind === "chart") {
            const plot = control.content.plot;
            expect(output.series).toBe(plot.series);
            expect(output.vectors).toBe(plot.vectors);
            expect(output.markers).toBe(plot.markers);
            expect(output.time_markers).toBe(plot.time_markers);
            expect(output.reference_lines).toBe(plot.reference_lines);
            expect(output.cursor_points).toBe(plot.cursor_points);
            expect(output.presentation).toBe(plot.presentation);
          }
        });
      });
      expect(JSON.stringify(shared)).toBe(before);
    });
  });

  it("keeps Flight information outside the menu and projects the VR pause action exactly", () => {
    const screen = createGameViewModel(currentSharedGameUiFixture("flight_running"));
    const vr = createGameViewModel(currentSharedGameUiFixture("flight_running", { platform: { presentation_mode: "phone_vr" } }));
    expect(screen.panels.flatMap((panel) => panel.controls.map((control) => control.id))).toEqual(["game-flight-pause", "game-flight-abort"]);
    expect(vr.panels.flatMap((panel) => panel.controls.map((control) => control.id))).toEqual(["game-flight-pause"]);
    expect(vr.panels.every((panel) => panel.anchor === "menu")).toBe(true);
  });

  it.each(["menu", "settings", "help"] as const)("projects shared paused %s controls and the domain resume permission", (overlay) => {
    const actions: readonly GameUiAction[] = overlay === "menu" ? [] : [{ kind: "set_pause_overlay", overlay }];
    const view = createGameViewModel(currentSharedGameUiFixture("flight_paused", { actions }));
    expect(view.activeOverlay).toBe(overlay === "menu" ? "Pause" : overlay === "settings" ? "PauseSettings" : "PauseHelp");
    const blocked = currentSharedGameUiFixture("flight_paused", { pauseReason: 1 });
    const resume = blocked.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-flight-resume");
    if (overlay === "menu") expect(resume?.enabled).toBe(false);
  });

  it("routes widget observations through the actual shared UI owner", () => {
    withSharedGameUiSession("flight_setup", {}, (session, platform) => {
      const input = { type: "activate" as const, controlId: "game-setup-start" };
      const action = controlInputAction(input);
      if (action === null) throw new Error("Missing control input request");
      expect(session.updateSharedUi(action, platform).effects)
        .toEqual([{ kind: "session", command: { kind: "prepare" } }]);
      for (const rejected of [
        { type: "activate" as const, controlId: "unknown" },
        { type: "set-toggle" as const, controlId: input.controlId, value: true }
      ]) expect(() => session.updateSharedUi({ kind: "control_input", input: rejected }, platform)).toThrow();
      expect(() => session.updateSharedUi(action, { ...platform, operation_pending: true })).toThrow();
    });
    withSharedGameUiSession("result", { actions: [{ kind: "set_result_tab", tab: "analysis" }] }, (session, platform) => {
      const range = session.readSharedUi(platform).panels.flatMap(panel => panel.controls).find(control => control.content.kind === "range");
      if (range?.content.kind !== "range") throw new Error("Missing analysis range");
      const request = (value: number): GameUiAction => ({ kind: "control_input", input: { type: "set-range", controlId: range.id, value } });
      expect(session.updateSharedUi(request(range.content.minimum), platform).effects)
        .toEqual([{ kind: "record", action: { kind: "seek_analysis", time_seconds: range.content.minimum } }]);
      for (const value of [NaN, Infinity, -Infinity, range.content.minimum - 1, range.content.maximum + 1])
        expect(() => session.updateSharedUi(request(value), platform)).toThrow();
    });
  });

  it("projects stored record choices and native capability restrictions from the shared view", () => {
    const records = [{ id: 12, saved_at: "2026-09-28T00:00:00.000Z", personal_best: true }] as const;
    const title = currentSharedGameUiFixture("title", { platform: { stored_records: records } });
    const control = createGameViewModel(title).panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-title-open-record-12");
    expect(control).toMatchObject({ kind: "button", enabled: true });
    expect(control?.label).toContain("2026/09/28 00:00 UTC");
    expect(control?.label).toContain("自己ベスト");
    const native = currentSharedGameUiFixture("result", { platform: { stored_records_available: false, flight_log_available: false, supported_replay_cameras: ["pilot", "chase"] } });
    expect(native.panels.flatMap((panel) => panel.controls).filter((control) => control.id.includes("download")).every((control) => !control.enabled)).toBe(true);
  });

});
