import type { FlightAnalysisData } from "../../game/flight-record-query.js";
import type { FlightCameraMode, PresentationMode } from "../contracts/runtime.js";

export type ReplayCameraSelection = "auto" | FlightCameraMode;

export function resolveReplayCameraMode(
  selection: ReplayCameraSelection,
  analysis: FlightAnalysisData | null,
  timeSeconds: number,
  presentationMode: PresentationMode
): FlightCameraMode {
  if (selection === "auto" && presentationMode !== "screen") return "pilot";
  if (selection !== "auto") return selection;
  if (analysis === null || analysis.samples.length === 0) return "pilot";
  if (!Number.isFinite(timeSeconds) || timeSeconds < 0) throw new RangeError("Camera time must be finite and non-negative");

  const durationSeconds = analysis.summary.durationSeconds;
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return "pilot";
  const recordTime = Math.min(timeSeconds, durationSeconds);
  const openingEndSeconds = Math.min(1.5, durationSeconds * 0.15);
  if (recordTime <= openingEndSeconds) return "platform";

  const terminalShotStartSeconds = Math.max(
    openingEndSeconds,
    durationSeconds - Math.min(2.5, durationSeconds * 0.2)
  );
  if (recordTime >= terminalShotStartSeconds) {
    return analysis.summary.terminal.reason === "water-contact" ? "shore" : "chase";
  }

  const lowAltitudeEvent = analysis.samples.find((sample) => sample.altitudeMeters <= 3);
  if (lowAltitudeEvent !== undefined && recordTime >= Math.max(openingEndSeconds, lowAltitudeEvent.timeSeconds - 1.5)) {
    return "side";
  }

  return "pilot";
}

export function resolveAttractCameraMode(
  analysis: FlightAnalysisData | null,
  timeSeconds: number,
  presentationMode: PresentationMode
): FlightCameraMode {
  if (presentationMode !== "screen" || analysis === null || analysis.samples.length === 0) return "pilot";
  if (!Number.isFinite(timeSeconds) || timeSeconds < 0) throw new RangeError("Camera time must be finite and non-negative");

  const durationSeconds = analysis.summary.durationSeconds;
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return "pilot";
  const recordTime = Math.min(timeSeconds, durationSeconds);
  const openingEndSeconds = Math.min(1.5, durationSeconds * 0.15);
  if (recordTime <= openingEndSeconds) return "platform";

  const terminalStartSeconds = Math.max(openingEndSeconds, durationSeconds - Math.min(2.5, durationSeconds * 0.25));
  if (recordTime >= terminalStartSeconds) {
    return recordTime >= (terminalStartSeconds + durationSeconds) / 2 ? "telephoto" : "shore";
  }

  const firstSample = analysis.samples[0];
  const lastSample = analysis.samples[analysis.samples.length - 1];
  if (firstSample === undefined || lastSample === undefined) return "chase";
  const totalDistance = Math.hypot(lastSample.northMeters - firstSample.northMeters, lastSample.eastMeters - firstSample.eastMeters);
  const currentSample = analysis.samples.find((sample) => sample.timeSeconds >= recordTime) ?? lastSample;
  const progressDistance = Math.hypot(currentSample.northMeters - firstSample.northMeters, currentSample.eastMeters - firstSample.eastMeters);
  return totalDistance > 0 && progressDistance >= totalDistance * 0.25 ? "side" : "chase";
}
