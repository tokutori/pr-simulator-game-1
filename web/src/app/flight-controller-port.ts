import type { AppMessage, FlightControllerIdentity } from "./app-state.js";
import type { FlightHudPort, FlightSessionPort } from "../game/flight-controller.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import type { TailFlightHudPort, TailSessionPort } from "../game/tail-flight-controller.js";
import type { FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";

export class FlightControllerUiBindings {
  private readonly sessionIds = new WeakMap<FlightSessionPort | TailSessionPort, number>();
  private nextSessionId = 1;
  private nextControllerId = 1;

  bind(
    session: FlightSessionPort,
    display: Pick<FlightHudPort, "render" | "setVisible">,
    readSnapshot: () => FlightSnapshot,
    dispatch: (message: AppMessage) => void
  ): { readonly identity: FlightControllerIdentity; readonly port: FlightHudPort } {
    const identity = this.identityFor(session);
    const port: FlightHudPort = Object.freeze({
      render: (snapshot: FlightSnapshot): void => { display.render(snapshot); },
      setVisible: (visible: boolean): void => { display.setVisible(visible); },
      fail: (message: string): void => {
        dispatch({ type: "flight-controller-stopped", identity, message, snapshot: readSnapshot() });
      }
    });
    return Object.freeze({ identity, port });
  }

  bindDisplay(session: TailSessionPort, display: Pick<TailFlightHudPort, "render" | "setVisible">,
    readSnapshot: () => FlightDisplaySnapshot, dispatch: (message: AppMessage) => void
  ): { readonly identity: FlightControllerIdentity; readonly port: TailFlightHudPort } {
    const identity = this.identityFor(session);
    const port: TailFlightHudPort = Object.freeze({
      render: (snapshot: FlightDisplaySnapshot): void => { display.render(snapshot); },
      setVisible: (visible: boolean): void => { display.setVisible(visible); },
      fail: (message: string): void => { dispatch({ type: "flight-controller-stopped", identity, message, snapshot: readSnapshot() }); }
    });
    return Object.freeze({ identity, port });
  }

  private identityFor(session: FlightSessionPort | TailSessionPort): FlightControllerIdentity {
    let sessionId = this.sessionIds.get(session);
    if (sessionId === undefined) {
      sessionId = this.nextSessionId++;
      this.sessionIds.set(session, sessionId);
    }
    return Object.freeze({ sessionId, controllerId: this.nextControllerId++ });
  }
}
