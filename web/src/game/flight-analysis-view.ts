import type { DisplayAvailability } from "../render/contracts/runtime.js";
import type { PlaybackContext } from "../generated/record-context.js";
import type { RecordedPlaybackSample } from "../generated/record-sample.js";
import type { SummaryDocument } from "../generated/record-summary.js";
import type { WindDocument } from "../generated/record-wind.js";

export interface NamedAnalysisDataset {
  readonly kind: "named_record";
  readonly context: PlaybackContext;
  readonly samples: readonly RecordedPlaybackSample[];
  readonly summary: SummaryDocument;
  readonly windGrid: DisplayAvailability<WindDocument, "not_requested">;
}

export type FlightAnalysisInput = NamedAnalysisDataset;
export type NamedAnalysisCursor = RecordedPlaybackSample & Readonly<{ kind: "named_record"; context: PlaybackContext }>;
export type FlightAnalysisCursorInput = NamedAnalysisCursor;

export function analysisCursorMatches(data: FlightAnalysisInput | null, sample: FlightAnalysisCursorInput): boolean {
  return data !== null && data.context === sample.context;
}
