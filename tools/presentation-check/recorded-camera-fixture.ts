import "./lake-model-fixture.js";
import { HybridGameSessionBridge, physics_hz } from "../../web/pkg/birdman_game_wasm.js";
import { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";

export function recordedCameraFixture(): TailAppSessionFacade {
  const bridge = new HybridGameSessionBridge(0, 3, 42);
  bridge.open_setup();
  bridge.prepare();
  bridge.mark_briefing_ready();
  bridge.start_countdown(1);
  bridge.observe_countdown(bridge.countdown_token(), 1, true);
  const input = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
    desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });
  for (let tick = 0; tick < 3; tick++) bridge.advance_tick_json(input);
  const session = new TailAppSessionFacade(bridge, physics_hz());
  session.executeSharedSessionCommand({ kind: "abort" });
  session.executeSharedSessionCommand({ kind: "enter_replay" });
  return session;
}
