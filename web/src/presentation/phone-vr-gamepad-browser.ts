import type { PhoneVrGamepadInputPort } from "./phone-vr-contracts.js";

interface BrowserGamepadButton {
  readonly pressed: boolean;
}

interface BrowserGamepad {
  readonly connected: boolean;
  readonly mapping: string;
  readonly axes: ArrayLike<number>;
  readonly buttons: ArrayLike<BrowserGamepadButton>;
}

interface PhoneVrNavigator {
  getGamepads?: () => ArrayLike<BrowserGamepad | null>;
}

export function createBrowserPhoneVrGamepadInputPort(
  targetNavigator: PhoneVrNavigator = (globalThis as unknown as { readonly navigator?: PhoneVrNavigator }).navigator ?? {}
): PhoneVrGamepadInputPort {
  return Object.freeze({
    readState() {
      try {
        const gamepads = targetNavigator.getGamepads?.();
        if (gamepads === undefined) return null;
        for (let index = 0; index < gamepads.length; index++) {
          const gamepad = gamepads[index];
          if (gamepad === null || gamepad === undefined || !gamepad.connected || gamepad.mapping !== "standard") continue;
          return Object.freeze({
            axes: Object.freeze(Array.from(gamepad.axes, (value) => Number.isFinite(value) ? value : 0)),
            buttons: Object.freeze(Array.from(gamepad.buttons, (button) => button.pressed))
          });
        }
        return null;
      } catch {
        return null;
      }
    }
  });
}
