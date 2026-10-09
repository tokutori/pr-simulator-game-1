export type EnvironmentBriefingProjection =
  | {
      readonly kind: "available";
      readonly name: string;
      readonly representativePositionNedMeters: readonly [number, number, number];
      readonly representativeAltitudeMeters: number;
      readonly representativeWindNedMetersPerSecond: readonly [number, number, number];
      readonly spatialVariation: "uniform" | "grid";
      readonly windBasis: "observed" | "derived" | "assumed" | "game_tuned";
    }
  | { readonly kind: "unavailable" };

export const NO_ENVIRONMENT_BRIEFING: EnvironmentBriefingProjection = Object.freeze({ kind: "unavailable" });

export function parseEnvironmentBriefingSnapshot(json: string, phaseCode: number): EnvironmentBriefingProjection {
  if (!Number.isInteger(phaseCode) || phaseCode < 1 || phaseCode > 8) return NO_ENVIRONMENT_BRIEFING;
  try {
    const snapshot: unknown = JSON.parse(json);
    if (!isRecord(snapshot) || snapshot.schema_version !== 1 || !isRecord(snapshot.context)
        || snapshot.context.kind !== "session" || snapshot.context.phase_code !== phaseCode
        || !isRecord(snapshot.projection)) return NO_ENVIRONMENT_BRIEFING;
    const projection = snapshot.projection;
    if (projection.kind !== "available" || projection.source !== (phaseCode === 1 ? "selected" : "sealed")
        || !isRecord(projection.metadata)) return NO_ENVIRONMENT_BRIEFING;
    const metadata = projection.metadata;
    const position = finiteVector(metadata.representative_position_ned_m);
    const wind = finiteVector(metadata.representative_velocity_ned_mps);
    const altitude = metadata.representative_altitude_m;
    if (typeof metadata.name !== "string" || metadata.name.trim().length === 0 || position === null || wind === null
        || typeof altitude !== "number" || !Number.isFinite(altitude) || !isRecord(metadata.wind_domain)
        || (metadata.wind_domain.kind !== "uniform" && metadata.wind_domain.kind !== "grid")
        || !isRecord(metadata.provenance) || !isRecord(metadata.provenance.wind_grid)) return NO_ENVIRONMENT_BRIEFING;
    const basis = metadata.provenance.wind_grid.kind;
    if (basis !== "observed" && basis !== "derived" && basis !== "assumed" && basis !== "game_tuned") return NO_ENVIRONMENT_BRIEFING;
    return Object.freeze({
      kind: "available", name: metadata.name,
      representativePositionNedMeters: position, representativeAltitudeMeters: altitude,
      representativeWindNedMetersPerSecond: wind,
      spatialVariation: metadata.wind_domain.kind, windBasis: basis
    });
  } catch {
    return NO_ENVIRONMENT_BRIEFING;
  }
}

function finiteVector(value: unknown): readonly [number, number, number] | null {
  if (!Array.isArray(value) || value.length !== 3 || value.some((entry) => typeof entry !== "number" || !Number.isFinite(entry))) return null;
  return Object.freeze([value[0] as number, value[1] as number, value[2] as number]);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
