import type { GameSessionOperation } from "./app-state.js";

export interface GameSessionOperationPort {
  abort(): ArrayLike<number>;
  cancel_briefing(): void;
  cancel_countdown(): void;
  cycle_assistance_level(): void;
  cycle_difficulty_preset(): void;
  cycle_information_level(): void;
  cycle_weather_class(): void;
  enter_attract(): void;
  enter_replay(): void;
  leave_attract(): void;
  leave_replay(): void;
  mark_briefing_ready(): void;
  open_setup(): void;
  pause(reason: number): void;
  prepare(): void;
  resume(): void;
  retry(): void;
  retry_briefing(): void;
  return_to_title(): void;
  set_control_mode(code: number): void;
  set_information_cue(code: number, visible: boolean): void;
  start_countdown(ticks: number): void;
}

export type GameSessionOperationResult =
  | { readonly kind: "completed" }
  | { readonly kind: "countdown-started" }
  | { readonly kind: "aborted"; readonly terminalSnapshot: ArrayLike<number> };

export function executeGameSessionOperation(
  session: GameSessionOperationPort,
  operation: GameSessionOperation
): GameSessionOperationResult {
  if (typeof operation !== "string") {
    session.set_information_cue(operation.cueCode, operation.visible);
    return { kind: "completed" };
  }
  switch (operation) {
    case "open-setup":
      session.open_setup();
      break;
    case "set-control-manual":
      session.set_control_mode(0);
      break;
    case "set-control-shared":
      session.set_control_mode(1);
      break;
    case "set-control-automatic":
      session.set_control_mode(2);
      break;
    case "cycle-difficulty-preset":
      session.cycle_difficulty_preset();
      break;
    case "cycle-information-level":
      session.cycle_information_level();
      break;
    case "cycle-assistance-level":
      session.cycle_assistance_level();
      break;
    case "cycle-weather-class":
      session.cycle_weather_class();
      break;
    case "return-to-title":
      session.return_to_title();
      break;
    case "prepare":
      session.prepare();
      session.mark_briefing_ready();
      break;
    case "cancel-briefing":
      session.cancel_briefing();
      break;
    case "start-flight":
      session.start_countdown(3);
      return { kind: "countdown-started" };
    case "cancel-countdown":
      session.cancel_countdown();
      break;
    case "pause":
      session.pause(0);
      break;
    case "resume":
      session.resume();
      break;
    case "abort":
      return { kind: "aborted", terminalSnapshot: session.abort() };
    case "retry":
      session.retry();
      break;
    case "retry-briefing":
      session.retry_briefing();
      session.mark_briefing_ready();
      break;
    case "enter-replay":
      session.enter_replay();
      break;
    case "leave-replay":
      session.leave_replay();
      break;
    case "enter-attract":
      session.enter_attract();
      break;
    case "leave-attract":
      session.leave_attract();
      break;
    default:
      return assertNever(operation);
  }
  return { kind: "completed" };
}

function assertNever(value: never): never {
  throw new TypeError(`Unsupported GameSession operation: ${String(value)}`);
}
