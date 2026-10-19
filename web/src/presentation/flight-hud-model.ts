import type { GameUiView } from "../generated/game-ui-view.js";

type SharedHud = NonNullable<GameUiView["hud"]>;

export type FlightHudModel = SharedHud & { readonly theme: GameUiView["theme"] };
export type FlightHudNode = SharedHud["groups"][number]["nodes"][number];

export function projectSharedFlightHudModel(shared: SharedHud, theme: GameUiView["theme"]): FlightHudModel {
  return Object.freeze({ ...shared, theme });
}
