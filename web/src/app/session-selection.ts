import type { ConfigurationMetadataUiState, DifficultyUiState, HudProfileUiState } from "./app-state.js";
import type { DisplayAvailability } from "../game/flight-display-snapshot.js";
import { boundaryInteger } from "../game/tail-boundary-values.js";

export interface SessionSelectionPort {
  difficulty_preset_code(): number;
  information_level_code(): number;
  assistance_level_code(): number;
  weather_class_code(): number;
  information_profile_codes(): ArrayLike<number>;
  configuration_metadata(): ArrayLike<number>;
}
export type PreparedUiConfiguration = DisplayAvailability<ConfigurationMetadataUiState, "unprepared_phase" | "playback_context">;

export function decodeSessionDifficulty(port: SessionSelectionPort): DifficultyUiState {
  return Object.freeze({ presetCode: boundaryInteger(port.difficulty_preset_code(), 0, 4),
    informationCode: boundaryInteger(port.information_level_code(), 0, 4),
    assistanceCode: boundaryInteger(port.assistance_level_code(), 0, 3), weatherCode: boundaryInteger(port.weather_class_code(), 0, 4),
    hudProfile: decodeHudProfile(port.information_profile_codes()) });
}

export function decodePreparedUiConfiguration(phaseCode: number, read: () => ArrayLike<number>): PreparedUiConfiguration {
  if (phaseCode === 9 || phaseCode === 10) return Object.freeze({ kind: "unavailable", reason: "playback_context" });
  if (phaseCode < 2 || phaseCode > 8) return Object.freeze({ kind: "unavailable", reason: "unprepared_phase" });
  const values = read();
  if (values.length !== 18) throw new RangeError("Resolved configuration metadata requires 18 values");
  const valueAt = (index: number, maximum = 0xffff_ffff): number => boundaryInteger(values[index], 0, maximum);
  return Object.freeze({ kind: "available", value: Object.freeze({ presetCode: valueAt(0, 4), informationCode: valueAt(1, 4),
    assistanceCode: valueAt(2, 3), weatherCode: valueAt(3, 4), catalogVersion: valueAt(4), scenarioId: valueAt(5),
    scenarioVersion: valueAt(6), aircraftModelVersion: valueAt(7), environmentVersion: valueAt(8), controllerProfileVersion: valueAt(9),
    seedLow: valueAt(10), seedHigh: valueAt(11), hudProfile: decodeHudProfile([12, 13, 14, 15, 16, 17].map((index) => valueAt(index, 1))) }) });
}

export function decodeHudProfile(values: ArrayLike<number>): HudProfileUiState {
  if (values.length !== 6) throw new RangeError("HUD profile requires six visibility codes");
  const cueAt = (index: number): boolean => boundaryInteger(values[index], 0, 1) === 1;
  return Object.freeze({ telemetry: cueAt(0), attitude: cueAt(1), wind: cueAt(2), flightPath: cueAt(3), angleOfAttack: cueAt(4), warnings: cueAt(5) });
}
