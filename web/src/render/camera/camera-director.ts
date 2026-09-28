import type { FlightAnalysisData } from "../../game/flight-record-query.js";
import type { FlightCameraMode, PresentationMode } from "../contracts/runtime.js";

export type ReplayCameraSelection = "auto" | FlightCameraMode;

export function resolveReplayCameraMode(
  selection: ReplayCameraSelection,
  analysis: FlightAnalysisData | null,
  timeSeconds: number,
  presentationMode: PresentationMode
): FlightCameraMode {
  if (selection !== "auto") return selection;
  if (presentationMode !== "screen" || analysis === null || analysis.samples.length === 0) return "pilot";
  if (!Number.isFinite(timeSeconds) || timeSeconds < 0) throw new RangeError("Camera time must be finite and non-negative");

  const durationSeconds = analysis.summary.durationSeconds;
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return "pilot";
  const recordTime = Math.min(timeSeconds, durationSeconds);
  const openingEndSeconds = Math.min(1.5, durationSeconds * 0.15);
  if (recordTime <= openingEndSeconds) return "chase";

  const lowAltitudeEvent = analysis.samples.find((sample) => sample.altitudeMeters <= 3);
  if (lowAltitudeEvent !== undefined && recordTime >= Math.max(openingEndSeconds, lowAltitudeEvent.timeSeconds - 1.5)) {
    return "chase";
  }

  const terminalShotStartSeconds = Math.max(
    openingEndSeconds,
    durationSeconds - Math.min(2.5, durationSeconds * 0.2)
  );
  return recordTime >= terminalShotStartSeconds ? "chase" : "pilot";
}
