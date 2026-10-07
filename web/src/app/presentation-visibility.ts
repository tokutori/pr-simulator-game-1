import type { PresentationUiState } from "./app-state.js";

export function screenUiVisible(state: PresentationUiState): boolean {
  switch (state.type) {
    case "uninitialized":
    case "initializing":
    case "failed":
      return true;
    case "ready":
      return state.mode === "screen";
    case "transitioning":
      return state.phase === "requesting" && state.from === "screen";
    case "cached":
    case "hidden":
      return false;
  }
}
