import { venueMapForEnvironment } from "./biwa-venue-map.js";
import type { RuntimeEnvironmentProjection } from "./runtime-environment.js";

export type RuntimeVenueProjection =
  | Readonly<{ kind: "visible"; basis: "title_exhibition" | "shared_launch" | "recorded_origin" }>
  | Readonly<{ kind: "unavailable"; reason: "environment_unavailable" | "origin_not_recorded" | "unregistered_origin" | "context_mismatch" }>;

export function projectRuntimeVenue(environment: RuntimeEnvironmentProjection, phaseCode: number): RuntimeVenueProjection {
  if (phaseCode === 0 && environment.kind === "unavailable" && environment.reason === "no_selection") {
    return Object.freeze({ kind: "visible", basis: "title_exhibition" });
  }
  if (environment.kind === "unavailable") return Object.freeze({ kind: "unavailable", reason: "environment_unavailable" });
  const source = environment.value.source;
  if (source === "archive") {
    if (phaseCode !== 9) return Object.freeze({ kind: "unavailable", reason: "context_mismatch" });
    const venue = venueMapForEnvironment(environment);
    return venue.kind === "available" ? Object.freeze({ kind: "visible", basis: "recorded_origin" })
      : Object.freeze({ kind: "unavailable", reason: venue.reason === "origin_not_recorded" ? "origin_not_recorded" : "unregistered_origin" });
  }
  const currentContext = source === "selected" ? phaseCode === 1
    : source === "sealed" ? Number.isInteger(phaseCode) && phaseCode >= 2 && phaseCode <= 8
    : source === "record" ? phaseCode === 9 : phaseCode === 10;
  return currentContext ? Object.freeze({ kind: "visible", basis: "shared_launch" })
    : Object.freeze({ kind: "unavailable", reason: "context_mismatch" });
}
