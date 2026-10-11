export interface GameSessionOperationPort {
  abort(): string;
  cancel_briefing(): void;
  cancel_countdown(): void;
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
  set_difficulty_preset(code: number): void;
  set_information_level(code: number): void;
  set_assistance_level(code: number): void;
  set_weather_class(code: number): void;
  set_information_cue(code: number, visible: boolean): void;
  start_countdown(ticks: number): void;
}
