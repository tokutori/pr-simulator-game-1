import { TailPersonalBestSelectionBridge } from "../../pkg/birdman_game_wasm.js";
import type { PersonalBestSelectionPort } from "./flight-record-store.js";
import { boundaryInteger } from "./tail-boundary-values.js";

const maximumRecordBytes = 16 * 1024 * 1024;

export function createArchivedPersonalBestSelection(recordJson: string): PersonalBestSelectionPort {
  requireCurrentRecord(recordJson);
  const selection = new TailPersonalBestSelectionBridge(recordJson);
  let resourceState: "alive" | "disposed" = "alive";
  const observe = <Value>(read: () => Value): Value => {
    if (resourceState !== "alive") throw new Error("Personal Best selection has been disposed");
    return read();
  };
  return Object.freeze({
    candidate_is_best: () => observe(() => selection.candidate_is_best()),
    is_eligible: () => observe(() => selection.is_eligible()),
    key_hex: () => observe(() => selection.key_hex()),
    selected_existing_id: () => observe(() => selection.selected_existing_id()),
    consider_existing: (id: number, json: string) => {
      observe(() => {
        boundaryInteger(id, 1);
        requireCurrentRecord(json);
        selection.consider_existing(id, json);
      });
    },
    free: () => {
      if (resourceState === "disposed") return;
      resourceState = "disposed";
      selection.free();
    }
  });
}

function requireCurrentRecord(json: string): void {
  if (new TextEncoder().encode(json).byteLength > maximumRecordBytes) throw new RangeError("Flight record exceeds the persistence size limit");
  const document: unknown = JSON.parse(json);
  if (typeof document !== "object" || document === null || Array.isArray(document) || !("schema_version" in document)) {
    throw new TypeError("Personal Best candidate must contain a flight record schema version");
  }
  if (document.schema_version !== 6) throw new RangeError("Unsupported flight record schema version");
}
