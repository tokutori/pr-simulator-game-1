import type { GameUiUpdate } from "../generated/game-ui-effects.js";

export type GameUiSessionCommand = Extract<GameUiUpdate["effects"][number], { readonly kind: "session" }>["command"];
