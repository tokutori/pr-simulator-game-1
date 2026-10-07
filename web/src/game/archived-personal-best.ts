import { PersonalBestSelectionBridge, TailPersonalBestSelectionBridge } from "../../pkg/birdman_game_wasm.js";
import type { PersonalBestSelectionPort } from "./flight-record-store.js";
import { boundaryInteger } from "./tail-boundary-values.js";

type ArchiveControlLayout = "legacy_three_axis" | "tail_incidence";
const maximumRecordBytes = 16 * 1024 * 1024;

export function createArchivedPersonalBestSelection(recordJson: string): PersonalBestSelectionPort {
  const layout = recordLayout(recordJson);
  const selection = createSelection(layout, recordJson);
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
        const existingLayout = recordLayout(json);
        if (existingLayout === layout && selection.is_eligible()) {
          selection.consider_existing(id, json);
        } else {
          const existing = createSelection(existingLayout, json);
          existing.free();
        }
      });
    },
    free: () => {
      if (resourceState === "disposed") return;
      resourceState = "disposed";
      selection.free();
    }
  });
}

function createSelection(layout: ArchiveControlLayout, json: string): PersonalBestSelectionPort {
  return layout === "legacy_three_axis" ? new PersonalBestSelectionBridge(json) : new TailPersonalBestSelectionBridge(json);
}

function recordLayout(json: string): ArchiveControlLayout {
  if (new TextEncoder().encode(json).byteLength > maximumRecordBytes) throw new RangeError("Flight record exceeds the persistence size limit");
  let document: unknown;
  try {
    document = JSON.parse(json) as unknown;
  } catch {
    throw new TypeError("Personal Best candidate must contain valid flight record JSON");
  }
  if (typeof document !== "object" || document === null || Array.isArray(document) || !("schema_version" in document)) {
    throw new TypeError("Personal Best candidate must contain a flight record schema version");
  }
  const version = boundaryInteger(document.schema_version, 1, 6);
  return version === 6 ? "tail_incidence" : "legacy_three_axis";
}
