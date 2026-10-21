import { HybridGameSessionBridge, physics_hz, ScreenRenderAutoBridge } from "../../web/pkg/birdman_game_wasm.js";
import { initializeWasmFixture } from "./wasm-fixture.js";
import type { ScreenRenderAutoSnapshot } from "../../web/src/generated/screen-render-auto.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import type { FlightRecordHudProfileDocument } from "../../web/src/generated/record-context.js";
import type { SessionPhase } from "../../web/src/app/session-snapshot.js";
import type { TailGameSessionProjection } from "../../web/src/app/session-snapshot.js";
import { readRuntimeSessionProjection } from "../../web/src/app/session-runtime-projection.js";
import type { GameUiAction } from "../../web/src/generated/game-ui-action.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";
import type { GameUiView } from "../../web/src/generated/game-ui-view.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import type { FlightHudModel, FlightHudNode } from "../../web/src/presentation/flight-hud-model.js";
import { neutralTailInput } from "./current-session-fixture.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-input.js";
import type { LakeWaterQuality } from "../../web/src/render/contracts/lake-water.js";

initializeWasmFixture();

export interface SharedGameUiFixtureOptions {
  readonly platform?: Partial<GameUiPlatformInput>;
  readonly actions?: readonly GameUiAction[];
  readonly informationCode?: 0 | 1 | 2 | 3 | 4;
  readonly assistanceCode?: 0 | 1 | 2 | 3;
  readonly weatherCode?: 0 | 1 | 2 | 3 | 4;
  readonly presetCode?: 0 | 1 | 2 | 3;
  readonly hudProfile?: FlightRecordHudProfileDocument;
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
    water_quality: "high",
    water_quality_operation: Object.freeze({ kind: "idle" }),
    screen_render_scale: sharedScreenRenderSnapshot(),
    notification: "",
    flight_controller_failure: null,
    head_hud_failure: null,
    stored_records: Object.freeze([] as const),
    ...overrides
  });
}

export function screenRenderAutoSnapshotReader(policy: ScreenRenderAutoBridge): () => ScreenRenderAutoSnapshot {
  let previous: { readonly json: string; readonly value: ScreenRenderAutoSnapshot } | null = null;
  return () => {
    const json = policy.snapshot_json();
    if (previous === null || previous.json !== json) previous = { json, value: JSON.parse(json) as ScreenRenderAutoSnapshot };
    return previous.value;
  };
}

export function sharedScreenRenderSnapshot(scale: "100" | "75" | "50" = "100", quality: LakeWaterQuality = "high"): GameUiPlatformInput["screen_render_scale"] {
  const bridge = new ScreenRenderAutoBridge();
  try {
    bridge.select_manual(Number(scale), 0);
    bridge.select_water_quality(quality, 0);
    if (bridge.requested_scale_percent() !== 0) {
      const requestedQuality = bridge.requested_water_quality();
      if (requestedQuality === undefined) throw new Error("Missing requested water quality");
      bridge.applied(bridge.requested_epoch(), Number(scale), bridge.requested_shadow_map_size(), requestedQuality, 0);
    }
    return screenRenderAutoSnapshotReader(bridge)();
  } finally { bridge.free(); }
}

export function createSharedGameUiSession(phase: SessionPhase, options: SharedGameUiFixtureOptions = {}):
  Readonly<{ session: TailAppSessionFacade; platform: GameUiPlatformInput }> {
  const bridge = new HybridGameSessionBridge(0, 21, 22);
  let owner: TailAppSessionFacade | null = null;
  try {
    owner = new TailAppSessionFacade(bridge, physics_hz());
    if (phase === "attract") bridge.enter_attract();
    else if (phase !== "title") {
      bridge.open_setup();
      if (options.presetCode !== undefined) bridge.set_difficulty_preset(options.presetCode);
      if (options.informationCode !== undefined) bridge.set_information_level(options.informationCode);
      if (options.assistanceCode !== undefined) bridge.set_assistance_level(options.assistanceCode);
      if (options.weatherCode !== undefined) bridge.set_weather_class(options.weatherCode);
      if (options.hudProfile !== undefined) {
        const profile = options.hudProfile;
        [profile.telemetry, profile.attitude, profile.wind, profile.flight_path, profile.angle_of_attack, profile.warnings]
          .forEach((visible, code) => { bridge.set_information_cue(code, visible); });
      }
      if (phase !== "flight_setup") {
        bridge.prepare();
        if (phase === "briefing_failed") bridge.fail_briefing(0);
        else if (phase !== "briefing_preparing") {
          bridge.mark_briefing_ready();
          if (phase !== "briefing_ready") {
            bridge.start_countdown(phase === "countdown" ? 3 : 1);
            if (phase !== "countdown") {
              bridge.observe_countdown(bridge.countdown_token(), 1, true);
              const ticks = options.ticks ?? 2;
              if (!Number.isSafeInteger(ticks) || ticks < 0) throw new RangeError("Fixture tick count must be a nonnegative integer");
              for (let index = 0; index < ticks && owner.readSnapshot().phase === "flight_running"; index += 1) bridge.advance_tick_json(encodeTailLogicalInput(neutralTailInput));
              if (phase === "flight_paused") bridge.pause(options.pauseReason ?? 0);
              if (phase === "result" || phase === "replay") bridge.abort();
              if (phase === "replay") bridge.enter_replay();
            }
          }
        }
      }
    }
    if (owner.readLifecycle().phase !== phase) throw new RangeError("Fixture did not reach the requested Rust phase");
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
    return { session: owner, platform };
  } catch (cause) {
    if (owner === null) bridge.free();
    else owner.dispose();
    throw cause;
  }
}

export function withSharedGameUiSession<Value>(phase: SessionPhase, options: SharedGameUiFixtureOptions,
  observe: (session: TailAppSessionFacade, platform: GameUiPlatformInput) => Value): Value {
  const { session, platform } = createSharedGameUiSession(phase, options);
  try { return observe(session, platform); }
  finally { session.dispose(); }
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

export function currentSharedGameUiFixture(phase: SessionPhase = "title", options: SharedGameUiFixtureOptions = {}): GameUiView {
  return withSharedGameUiSession(phase, options, (session, platform) => session.readSharedUi(platform));
}

export function currentGameSessionProjectionFixture<Phase extends SessionPhase>(phase: Phase, options: SharedGameUiFixtureOptions = {}): TailGameSessionProjection & Readonly<{ phase: Phase }> {
  return withSharedGameUiSession(phase, options, session => readRuntimeSessionProjection(session)) as TailGameSessionProjection & Readonly<{ phase: Phase }>;
}

export function currentSharedHudFixture(options: SharedGameUiFixtureOptions = {}): FlightHudModel {
  const view = currentSharedGameUiFixture("flight_running", options);
  if (view.hud === null) throw new Error("Rust flight fixture did not provide a shared HUD");
  return projectSharedFlightHudModel(view.hud, view.theme);
}

export function currentSharedHeadHudFixture(options: SharedGameUiFixtureOptions = {}): FlightHudModel {
  return currentSharedHudFixture({ ...options, platform: { ...options.platform, presentation_mode: "phone_vr" } });
}

export function sharedHudNodes(model: FlightHudModel): readonly FlightHudNode[] {
  return model.groups.flatMap((group) => group.nodes);
}

export function sharedHudNode(model: FlightHudModel, id: string): FlightHudNode {
  const node = sharedHudNodes(model).find((candidate) => candidate.id === id);
  if (node === undefined) throw new Error(`Missing shared HUD node: ${id}`);
  return node;
}

export function mapSharedHudNodes(model: FlightHudModel, transform: (node: FlightHudNode) => FlightHudNode): FlightHudModel {
  return { ...model, groups: model.groups.map((group) => ({ ...group, nodes: group.nodes.map(transform) })) };
}
