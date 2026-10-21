import type { AppMessage, FlightControllerIdentity } from "./app-state.js";
import type { TailGameSessionProjection } from "./session-snapshot.js";
import type { TailControllerSnapshot, TailResultSnapshot } from "../game/tail-flight-controller.js";
import type { TailFlightHudPort, TailSessionPort } from "../game/tail-flight-controller.js";
import type { FlightSnapshotInput } from "./session-snapshot.js";

export class FlightControllerUiBindings {
  private readonly sessionIds = new WeakMap<TailSessionPort, number>();
  private nextSessionId = 1;
  private nextControllerId = 1;

  bindAppDisplay(session: TailSessionPort, display: Pick<TailFlightHudPort, "render" | "setVisible">,
    readProjection: () => TailGameSessionProjection, readSnapshot: () => FlightSnapshotInput, dispatch: (message: AppMessage) => void
  ): { readonly identity: FlightControllerIdentity; readonly port: TailFlightHudPort; readonly onTerminal: (snapshot: TailResultSnapshot) => void } {
    const identity = this.identityFor(session);
    const publishTerminal = (): void => {
      const projection = readProjection();
      if (projection.phase !== "result") throw new RangeError("Terminal notification requires the Rust Result projection");
      dispatch({ type: "tail-controller-terminal", identity, projection });
    };
    const port: TailFlightHudPort = Object.freeze({
      render: (snapshot: TailControllerSnapshot): void => { display.render(snapshot); },
      setVisible: (visible: boolean): void => { display.setVisible(visible); },
      fail: (message: string): void => {
        const projection = readProjection();
        if (projection.phase === "result") {
          dispatch({ type: "tail-controller-terminal", identity, projection });
          dispatch({ type: "flight-controller-result-feedback", identity, message });
          return;
        }
        dispatch({ type: "flight-controller-stopped", identity, message, projection, snapshot: readSnapshot() });
      }
    });
    return Object.freeze({ identity, port, onTerminal: publishTerminal });
  }

  private identityFor(session: TailSessionPort): FlightControllerIdentity {
    let sessionId = this.sessionIds.get(session);
    if (sessionId === undefined) {
      sessionId = this.nextSessionId++;
      this.sessionIds.set(session, sessionId);
    }
    return Object.freeze({ sessionId, controllerId: this.nextControllerId++ });
  }
}
