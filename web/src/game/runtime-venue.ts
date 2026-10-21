import type { RuntimeEnvironmentProjection } from "./runtime-environment.js";

export type RuntimeVenueProjection =
  | Readonly<{ kind: "visible"; basis: "title_exhibition" | "shared_launch" | "registered_origin" }>
  | Readonly<{ kind: "unavailable"; reason: "environment_unavailable" | "origin_not_recorded" | "unregistered_origin" }>;

export function projectRuntimeVenue(environment: RuntimeEnvironmentProjection): RuntimeVenueProjection {
  if (environment.kind === "unavailable") return { kind: "unavailable", reason: "environment_unavailable" };
  const { venue, source } = environment.value;
  switch (venue.kind) {
    case "shared_launch": return { kind: "visible", basis: source === "exhibition" ? "title_exhibition" : "shared_launch" };
    case "registered_origin": return { kind: "visible", basis: "registered_origin" };
    case "origin_not_recorded": return { kind: "unavailable", reason: "origin_not_recorded" };
    case "unregistered_origin": return { kind: "unavailable", reason: "unregistered_origin" };
  }
}
