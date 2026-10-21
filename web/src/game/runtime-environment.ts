import type { EnvironmentSnapshot } from "../generated/runtime-environment.js";
import { exhibition_environment_json } from "../../pkg/birdman_game_wasm.js";
import type { DisplayAvailability } from "../render/contracts/runtime.js";
import type { LakeVisualCondition } from "../render/contracts/lake-water.js";
import { createLakeSkyCondition } from "../render/contracts/lake-sky.js";
import type { LakeSkyCondition } from "../render/contracts/lake-sky.js";

type Environment = Extract<EnvironmentSnapshot["projection"], { kind: "available" }>;
export interface RuntimeEnvironmentMetadata {
  readonly source: Environment["source"];
  readonly identity: Environment["identity"];
  readonly localFrame: Environment["metadata"]["local_frame"];
  readonly venue: Environment["venue"];
  readonly waves: LakeVisualCondition;
  readonly sky: DisplayAvailability<Readonly<{ condition: LakeSkyCondition }>, "sky_not_recorded">;
}
export type RuntimeEnvironmentProjection = DisplayAvailability<RuntimeEnvironmentMetadata, "no_selection">
  | (Extract<EnvironmentSnapshot["projection"], { kind: "unavailable" }> & Readonly<{ reason: "unregistered_environment_identity" }>);

export function projectRuntimeEnvironment(snapshot: EnvironmentSnapshot): RuntimeEnvironmentProjection {
  const projection = snapshot.projection;
  if (projection.kind === "unavailable") return { ...projection, reason: "unregistered_environment_identity" };
  const { metadata, source, identity } = projection;
  const { waves, sky } = metadata;
  return { kind: "available", value: { source, identity, localFrame: metadata.local_frame, venue: projection.venue,
    sky: sky.kind === "unavailable" ? { kind: "unavailable", reason: "sky_not_recorded" }
      : { kind: "available", value: { condition: createLakeSkyCondition({ sunAzimuthDegrees: sky.value.sun_azimuth_degrees,
        sunElevationDegrees: sky.value.sun_elevation_degrees, cloudFraction: sky.value.cloud_fraction,
        cloudBaseMeters: sky.value.cloud_base_m, visibilityMeters: sky.value.visibility_m }) } },
    waves: { windNorthMetersPerSecond: waves.wind_velocity_ne_mps[0], windEastMetersPerSecond: waves.wind_velocity_ne_mps[1],
      fetchMeters: waves.fetch_m, detailAmplitudeScale: waves.detail_amplitude_scale, patternSeed: waves.pattern_seed }
  } };
}

export function readExhibitionEnvironment(): RuntimeEnvironmentMetadata {
  const environment = projectRuntimeEnvironment(JSON.parse(exhibition_environment_json()) as EnvironmentSnapshot);
  if (environment.kind === "unavailable") throw new Error(`Title exhibition: ${environment.reason}`);
  return environment.value;
}
