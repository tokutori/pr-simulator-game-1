import type { PhoneVrGamepadConnection, PhoneVrGamepadInputPort } from "./phone-vr-contracts.js";

interface BrowserGamepadButton {
  readonly pressed: boolean;
}

interface BrowserGamepad {
  readonly index: number;
  readonly connected: boolean;
  readonly mapping: string;
  readonly axes: ArrayLike<number>;
  readonly buttons: ArrayLike<BrowserGamepadButton>;
}

interface PhoneVrNavigator {
  getGamepads?: () => ArrayLike<BrowserGamepad | null | undefined>;
}

interface GamepadSession {
  readonly generations: Map<number, number>;
  selected: PhoneVrGamepadConnection | null;
  readonly connected: EventListener;
  readonly disconnected: EventListener;
}

export function createBrowserPhoneVrGamepadInputPort(
  targetNavigator: PhoneVrNavigator = (globalThis as unknown as { readonly navigator?: PhoneVrNavigator }).navigator ?? {},
  events: Pick<EventTarget, "addEventListener" | "removeEventListener"> | null = typeof window === "undefined" ? null : window
): PhoneVrGamepadInputPort {
  let active: GamepadSession | null = null;
  let generation = 0;
  const release = (session: GamepadSession): void => {
    let failure: { readonly type: "none" } | { readonly type: "failed"; readonly cause: unknown } = { type: "none" };
    for (const [type, listener] of [["gamepadconnected", session.connected], ["gamepaddisconnected", session.disconnected]] as const) {
      try {
        events?.removeEventListener(type, listener);
      } catch (cause) {
        if (failure.type === "none") failure = { type: "failed", cause };
      }
    }
    if (failure.type === "failed") throw failure.cause;
  };
  return Object.freeze({
    start() {
      if (active !== null || events === null) return;
      const session: GamepadSession = {
        generations: new Map(),
        selected: null,
        connected: (event) => {
          if (active !== session) return;
          const index = eventGamepadIndex(event);
          if (index !== null) session.generations.set(index, ++generation);
        },
        disconnected: (event) => {
          if (active !== session) return;
          const index = eventGamepadIndex(event);
          if (index === null) return;
          session.generations.delete(index);
          if (session.selected?.index === index) session.selected = null;
        }
      };
      active = session;
      try {
        events.addEventListener("gamepadconnected", session.connected);
        if (active === session) events.addEventListener("gamepaddisconnected", session.disconnected);
        if (active !== session) release(session);
      } catch (error) {
        if (active === session) active = null;
        try { release(session); } catch { throw error; }
        throw error;
      }
    },
    stop() {
      const session = active;
      active = null;
      if (session !== null) release(session);
    },
    readState() {
      const session = active;
      if (session === null) return null;
      try {
        const gamepads = Array.from(targetNavigator.getGamepads?.() ?? []).filter(
          (gamepad): gamepad is BrowserGamepad => gamepad !== null && gamepad !== undefined &&
            gamepad.connected && gamepad.mapping === "standard" && Number.isSafeInteger(gamepad.index) && gamepad.index >= 0
        );
        if (active !== session) return null;
        for (const index of session.generations.keys()) {
          if (!gamepads.some((gamepad) => gamepad.index === index)) session.generations.delete(index);
        }
        const selected = session.selected;
        const gamepad = gamepads.find((candidate) => selected !== null && candidate.index === selected.index &&
          session.generations.get(candidate.index) === selected.generation) ?? gamepads[0];
        if (gamepad === undefined) {
          session.selected = null;
          return null;
        }
        const currentGeneration = session.generations.get(gamepad.index) ?? ++generation;
        session.generations.set(gamepad.index, currentGeneration);
        const connection = Object.freeze({ index: gamepad.index, generation: currentGeneration });
        session.selected = connection;
        return Object.freeze({
          connection,
          axes: Object.freeze(Array.from(gamepad.axes, (value) => Number.isFinite(value) ? value : 0)),
          buttons: Object.freeze(Array.from(gamepad.buttons, (button) => button.pressed))
        });
      } catch {
        return null;
      }
    }
  });
}

function eventGamepadIndex(event: Event): number | null {
  const gamepad = (event as Event & { readonly gamepad?: BrowserGamepad }).gamepad;
  return gamepad !== undefined && Number.isSafeInteger(gamepad.index) && gamepad.index >= 0 ? gamepad.index : null;
}
