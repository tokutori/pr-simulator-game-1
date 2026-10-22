import { advanceFlightFixture } from "../game-check/current-session-fixture.js";
import "./lake-model-fixture.js";
import { createAppSession } from "../../web/src/app/session-factory.js";
import type { TailAppSessionFacade } from "../../web/src/app/session-facade.js";
import { launchCurrentSession } from "../game-check/current-session-fixture.js";

export function recordedCameraFixture(): TailAppSessionFacade {
  const session = createAppSession({ controlModeCode: 0, seedLow: 3, seedHigh: 42 });
  try {
    launchCurrentSession(session);
    for (let tick = 0; tick < 3; tick++) advanceFlightFixture(session);
    session.executeSharedSessionCommand({ kind: "abort" });
    session.executeSharedSessionCommand({ kind: "enter_replay" });
    return session;
  } catch (cause) {
    session.dispose();
    throw cause;
  }
}
