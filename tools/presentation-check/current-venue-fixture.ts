import registeredEnvironment from "../../assets/biwa-typical-july-environment-v6.json";
import { venueMapForEnvironment } from "../../web/src/game/biwa-venue-map.js";
import type { VenueMapProjection } from "../../web/src/game/biwa-venue-map.js";
import type { RuntimeEnvironmentProjection } from "../../web/src/game/runtime-environment.js";

export function currentVenueEnvironmentFixture(): RuntimeEnvironmentProjection {
  const localFrame = registeredEnvironment.local_frame;
  const waves = registeredEnvironment.waves;
  return { kind: "available", value: {
    source: "sealed",
    identity: { catalogVersion: 3, scenarioId: 6, scenarioVersion: 3, aircraftModelVersion: 2,
      environmentVersion: 6, controllerProfileVersion: 3, seedLow: 21, seedHigh: 22 },
    localFrame: { kind: "available", value: { latitudeDegrees: localFrame.latitude_degrees,
      longitudeDegrees: localFrame.longitude_degrees, waterLevelDatum: localFrame.water_level_datum } },
    waves: { windNorthMetersPerSecond: waves.wind_velocity_ne_mps[0] as number,
      windEastMetersPerSecond: waves.wind_velocity_ne_mps[1] as number, fetchMeters: waves.fetch_m,
      detailAmplitudeScale: waves.detail_amplitude_scale, patternSeed: waves.pattern_seed },
    sky: { kind: "unavailable", reason: "sky_not_recorded" }
  } };
}

export function currentVenueMapFixture(): VenueMapProjection {
  return venueMapForEnvironment(currentVenueEnvironmentFixture());
}
