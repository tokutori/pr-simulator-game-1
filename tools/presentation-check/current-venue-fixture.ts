import "./lake-model-fixture.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import { venueMapForEnvironment } from "../../web/src/game/biwa-venue-map.js";
import type { VenueMapProjection } from "../../web/src/game/biwa-venue-map.js";
import { projectRuntimeEnvironment } from "../../web/src/game/runtime-environment.js";
import type { RuntimeEnvironmentProjection } from "../../web/src/game/runtime-environment.js";

export function currentVenueEnvironmentFixture(): RuntimeEnvironmentProjection {
  const session = createAppSession({ controlModeCode: 0, seedLow: 21, seedHigh: 22 });
  try {
    session.executeSharedSessionCommand({ kind: "open_setup" });
    session.executeSharedSessionCommand({ kind: "select_weather", weather: "typical" });
    session.executeSharedSessionCommand({ kind: "prepare" });
    return projectRuntimeEnvironment(session.readEnvironment());
  } finally { session.dispose(); }
}

export function currentVenueMapFixture(): VenueMapProjection {
  return venueMapForEnvironment(currentVenueEnvironmentFixture());
}
