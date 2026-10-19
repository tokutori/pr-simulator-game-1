import { venueMapForEnvironment } from "./biwa-venue-map.js";
import type { RuntimeEnvironmentProjection } from "./runtime-environment.js";
import type { SessionLifecycleDocument } from "../generated/session-lifecycle.js";

export type RuntimeVenueProjection =
  | Readonly<{ kind: "visible"; basis: "title_exhibition" | "shared_launch" | "recorded_origin" }>
  | Readonly<{ kind: "unavailable"; reason: "environment_unavailable" | "origin_not_recorded" | "unregistered_origin" | "context_mismatch" }>;

export function projectRuntimeVenue(environment: RuntimeEnvironmentProjection, phase: SessionLifecycleDocument["phase"] | "boot"): RuntimeVenueProjection {
  if (phase === "title" && environment.kind === "unavailable" && environment.reason === "no_selection") {
    return Object.freeze({ kind: "visible", basis: "title_exhibition" });
  }
  if (environment.kind === "unavailable") return Object.freeze({ kind: "unavailable", reason: "environment_unavailable" });
  const source = environment.value.source;
  if (source === "archive") {
    if (phase !== "replay") return Object.freeze({ kind: "unavailable", reason: "context_mismatch" });
    const venue = venueMapForEnvironment(environment);
    return venue.kind === "available" ? Object.freeze({ kind: "visible", basis: "recorded_origin" })
      : Object.freeze({ kind: "unavailable", reason: venue.reason === "origin_not_recorded" ? "origin_not_recorded" : "unregistered_origin" });
  }
  const currentContext = source === "selected" ? phase === "flight_setup"
    : source === "sealed" ? ["briefing_preparing", "briefing_ready", "briefing_failed", "countdown", "flight_running", "flight_paused", "result"].includes(phase)
    : source === "record" ? phase === "replay" : source === "attract" && phase === "attract";
  return currentContext ? Object.freeze({ kind: "visible", basis: "shared_launch" })
    : Object.freeze({ kind: "unavailable", reason: "context_mismatch" });
}
