import type { GameUiUpdate } from "../generated/game-ui-effects.js";

export type SessionCommand = Extract<GameUiUpdate["effects"][number], { readonly kind: "session" }>["command"];
