import { MenuSessionBridge } from "../../pkg/birdman_game_wasm.js";
import type { MenuSessionInput } from "../generated/menu-session-input.js";
import type { MenuSessionState } from "../generated/menu-session-state.js";

export class MenuSession {
  private owner: MenuSessionBridge | null = null;
  private json = '{"kind":"closed","generation":0}';
  private state: MenuSessionState = Object.freeze({ kind: "closed", generation: 0 });
  private disposed = false;

  update(input: MenuSessionInput): MenuSessionState {
    if (this.disposed) throw new Error("Menu owner is disposed");
    if (this.owner === null && input.type === "menu-scroll-synchronized" && input.target.kind === "closed") return this.state;
    this.owner ??= new MenuSessionBridge();
    const json = this.owner.update_json(JSON.stringify(input));
    if (json !== this.json) {
      this.json = json;
      this.state = Object.freeze(JSON.parse(json) as MenuSessionState);
    }
    return this.state;
  }

  dispose(): void {
    this.disposed = true;
    const owner = this.owner;
    this.owner = null;
    owner?.free();
  }
}
