import type { AppMessage, FlightControllerIdentity } from "./app-state.js";
import type { FlightHudPort, FlightSessionPort } from "../game/flight-controller.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";

export class FlightControllerUiBindings {
  private readonly sessionIds = new WeakMap<FlightSessionPort, number>();
  private nextSessionId = 1;
  private nextControllerId = 1;

  bind(
    session: FlightSessionPort,
    display: Pick<FlightHudPort, "render" | "setVisible">,
    readSnapshot: () => FlightSnapshot,
    dispatch: (message: AppMessage) => void
  ): { readonly identity: FlightControllerIdentity; readonly port: FlightHudPort } {
    let sessionId = this.sessionIds.get(session);
    if (sessionId === undefined) {
      sessionId = this.nextSessionId++;
      this.sessionIds.set(session, sessionId);
    }
    const identity = Object.freeze({ sessionId, controllerId: this.nextControllerId++ });
    const port: FlightHudPort = Object.freeze({
      render: (snapshot: FlightSnapshot): void => { display.render(snapshot); },
      setVisible: (visible: boolean): void => { display.setVisible(visible); },
      fail: (message: string): void => {
        dispatch({ type: "flight-controller-stopped", identity, message, snapshot: readSnapshot() });
      }
    });
    return Object.freeze({ identity, port });
  }
}
