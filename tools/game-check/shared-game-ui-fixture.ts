import { readFileSync } from "node:fs";
import { HybridGameSessionBridge, initSync, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { HudProfileUiState } from "../../web/src/app/app-state.js";
import type { GameUiAction } from "../../web/src/generated/game-ui-action.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";
import type { GameUiView } from "../../web/src/generated/game-ui-view.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import type { FlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { neutralTailInput } from "./current-session-fixture.js";

initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });

export interface SharedGameUiFixtureOptions {
  readonly platform?: Partial<GameUiPlatformInput>;
  readonly actions?: readonly GameUiAction[];
  readonly informationCode?: 0 | 1 | 2 | 3 | 4;
  readonly assistanceCode?: 0 | 1 | 2 | 3;
  readonly weatherCode?: 0 | 1 | 2 | 3 | 4;
  readonly presetCode?: 0 | 1 | 2 | 3;
  readonly hudProfile?: HudProfileUiState;
  readonly ticks?: number;
  readonly pauseReason?: 0 | 1 | 2;
}

export function sharedGameUiPlatform(overrides: Partial<GameUiPlatformInput> = {}): GameUiPlatformInput {
  return Object.freeze({
    viewport: Object.freeze({ width: 1280, height: 720, root_font_size: 16 }),
    chart_extent: null,
    presentation_mode: "screen",
    operation_pending: false,
    webxr_available: true,
    phone_vr_available: true,
    analysis_available: true,
    replay_available: true,
    supported_replay_cameras: Object.freeze(["auto", "pilot", "chase", "orbit", "platform", "shore", "overhead", "side", "front", "telephoto"] as const),
    stored_records_available: true,
    flight_log_available: true,
    flight_log_pending: false,
    water_quality: "high",
    water_quality_operation: Object.freeze({ kind: "idle" }),
    notification: "",
    flight_controller_failure: null,
    head_hud_failure: null,
    stored_records: Object.freeze([]),
    ...overrides
  });
}

export function withSharedGameUiSession<Value>(phaseCode: number, options: SharedGameUiFixtureOptions,
  observe: (session: TailAppSessionFacade, platform: GameUiPlatformInput) => Value): Value {
  const bridge = new HybridGameSessionBridge(0, 21, 22);
  let owner: TailAppSessionFacade | null = null;
  try {
    if (![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].includes(phaseCode)) throw new RangeError("Unsupported shared UI fixture phase");
    if (phaseCode === 10) bridge.enter_attract();
    else if (phaseCode !== 0) {
      bridge.open_setup();
      if (options.presetCode !== undefined) bridge.set_difficulty_preset(options.presetCode);
      if (options.informationCode !== undefined) bridge.set_information_level(options.informationCode);
      if (options.assistanceCode !== undefined) bridge.set_assistance_level(options.assistanceCode);
      if (options.weatherCode !== undefined) bridge.set_weather_class(options.weatherCode);
      if (options.hudProfile !== undefined) {
        const profile = options.hudProfile;
        [profile.telemetry, profile.attitude, profile.wind, profile.flightPath, profile.angleOfAttack, profile.warnings]
          .forEach((visible, code) => { bridge.set_information_cue(code, visible); });
      }
      if (phaseCode >= 2) {
        bridge.prepare();
        if (phaseCode === 8) bridge.fail_briefing(0);
        else if (phaseCode !== 2) {
          bridge.mark_briefing_ready();
          if (phaseCode >= 4) {
            bridge.start_countdown(phaseCode === 4 ? 3 : 1);
            if (phaseCode >= 5) {
              bridge.advance_countdown();
              bridge.launch();
              const ticks = options.ticks ?? 2;
              if (!Number.isSafeInteger(ticks) || ticks < 0) throw new RangeError("Fixture tick count must be a nonnegative integer");
              for (let index = 0; index < ticks && bridge.phase_code() === 5; index += 1) bridge.advance_tick_json(neutralTailInput);
              if (phaseCode === 6) bridge.pause(options.pauseReason ?? 0);
              if (phaseCode === 7 || phaseCode === 9) bridge.abort();
              if (phaseCode === 9) bridge.enter_replay();
            }
          }
        }
      }
    }
    if (bridge.phase_code() !== phaseCode) throw new RangeError("Fixture did not reach the requested Rust phase");
    owner = new TailAppSessionFacade(bridge, physics_hz());
    const platform = sharedGameUiPlatform(options.platform);
    for (const action of options.actions ?? []) {
      if (action.kind === "set_replay_view") {
        selectSharedReplayView(owner, platform, action.view);
        continue;
      }
      const update = owner.updateSharedUi(action, platform);
      for (const effect of update.effects) {
        if (effect.kind === "session") owner.executeSharedSessionCommand(effect.command);
        if (effect.kind === "record") owner.executeSharedRecordAction(effect.action);
      }
    }
    return observe(owner, platform);
  } finally {
    if (owner === null) bridge.free();
    else owner.dispose();
  }
}

export function selectSharedReplayView(session: TailAppSessionFacade, platform: GameUiPlatformInput,
  target: Extract<GameUiAction, { readonly kind: "set_replay_view" }>["view"]): void {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const view = session.readSharedUi(platform);
    if (view.selections.replay_view === target) return;
    const control = view.panels.flatMap((panel) => panel.controls).find((candidate) => candidate.enabled
      && candidate.content.kind === "button" && candidate.content.action.kind === "set_replay_view");
    if (control?.content.kind !== "button") throw new Error("Replay view selector is unavailable");
    session.updateSharedUi(control.content.action, platform);
  }
  if (session.readSharedUi(platform).selections.replay_view !== target) throw new Error("Replay view selector did not reach its target");
}

export function currentSharedGameUiFixture(phaseCode = 0, options: SharedGameUiFixtureOptions = {}): GameUiView {
  return withSharedGameUiSession(phaseCode, options, (session, platform) => session.readSharedUi(platform));
}

export function currentSharedHudFixture(options: SharedGameUiFixtureOptions = {}): FlightHudModel {
  const view = currentSharedGameUiFixture(5, options);
  if (view.hud === null) throw new Error("Rust flight fixture did not provide a shared HUD");
  return projectSharedFlightHudModel(view.hud, view.theme);
}
