import type { PhoneVrAvailability, PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "./phone-vr-contracts.js";

type OrientationPermissionState = "granted" | "denied" | "prompt";

interface DeviceOrientationReadingEvent {
  readonly alpha: number | null;
  readonly beta: number | null;
  readonly gamma: number | null;
  readonly timeStamp: number;
}

interface PhoneVrBrowserWindow {
  readonly isSecureContext: boolean;
  readonly DeviceOrientationEvent?: {
    requestPermission?: (absolute?: boolean) => Promise<OrientationPermissionState>;
  };
  readonly screen: {
    readonly orientation?: {
      readonly angle: number;
      addEventListener(type: string, listener: () => void): void;
      removeEventListener(type: string, listener: () => void): void;
    };
  };
  addEventListener(type: string, listener: (event: DeviceOrientationReadingEvent) => void): void;
  removeEventListener(type: string, listener: (event: DeviceOrientationReadingEvent) => void): void;
}

export function createBrowserPhoneVrSensorPort(
  targetWindow: PhoneVrBrowserWindow = globalThis as unknown as PhoneVrBrowserWindow
): PhoneVrSensorPort {
  let orientationListener: ((event: DeviceOrientationReadingEvent) => void) | null = null;
  let screenOrientationListener: (() => void) | null = null;
  let onReading: ((reading: PhoneVrSensorReading) => void) | null = null;
  let onScreenOrientationChange: ((angle: number | null) => void) | null = null;

  return Object.freeze({
    checkAvailability(): Promise<PhoneVrAvailability> {
      if (!targetWindow.isSecureContext) return Promise.resolve({ supported: false, message: "Phone VR requires a secure HTTPS context" });
      if (typeof targetWindow.DeviceOrientationEvent === "undefined") {
        return Promise.resolve({ supported: false, message: "Device orientation events are unavailable in this browser" });
      }
      if (targetWindow.screen.orientation === undefined) {
        return Promise.resolve({ supported: false, message: "Screen orientation data is unavailable in this browser" });
      }
      if (!Number.isFinite(targetWindow.screen.orientation.angle)) {
        return Promise.resolve({ supported: false, message: "Screen orientation angle is unavailable" });
      }
      return Promise.resolve({ supported: true, message: "Phone VR sensor API is available; permission and sensor events are still required" });
    },
    requestPermissionFromUserGesture(): Promise<PhoneVrPermissionResult> {
      if (typeof targetWindow.DeviceOrientationEvent === "undefined") {
        return Promise.resolve({ ok: false, message: "Device orientation events are unavailable in this browser" });
      }
      const constructor = targetWindow.DeviceOrientationEvent;
      let permission: Promise<OrientationPermissionState>;
      try {
        permission = constructor.requestPermission === undefined
          ? Promise.resolve("granted")
          : constructor.requestPermission(false);
      } catch (error) {
        return Promise.resolve({ ok: false, message: `Phone VR permission request failed: ${errorMessage(error)}` });
      }
      return permission.then((result) => result === "granted"
        ? { ok: true }
        : { ok: false, message: `Phone VR sensor permission ${result}` },
      (error: unknown) => ({ ok: false, message: `Phone VR permission request failed: ${errorMessage(error)}` }));
    },
    getScreenOrientationAngle(): number | null {
      return readScreenOrientationAngle(targetWindow);
    },
    startListening(
      readingHandler: (reading: PhoneVrSensorReading) => void,
      screenOrientationHandler: (angle: number | null) => void
    ): void {
      if (orientationListener !== null) throw new Error("Phone VR sensor listener is already active");
      onReading = readingHandler;
      onScreenOrientationChange = screenOrientationHandler;
      orientationListener = (event) => {
        onReading?.(Object.freeze({
          alpha: event.alpha,
          beta: event.beta,
          gamma: event.gamma,
          timestampMs: event.timeStamp
        }));
      };
      screenOrientationListener = () => {
        onScreenOrientationChange?.(readScreenOrientationAngle(targetWindow));
      };
      targetWindow.addEventListener("deviceorientation", orientationListener);
      targetWindow.screen.orientation?.addEventListener("change", screenOrientationListener);
    },
    stopListening(): void {
      if (orientationListener !== null) targetWindow.removeEventListener("deviceorientation", orientationListener);
      if (screenOrientationListener !== null) targetWindow.screen.orientation?.removeEventListener("change", screenOrientationListener);
      orientationListener = null;
      screenOrientationListener = null;
      onReading = null;
      onScreenOrientationChange = null;
    }
  });
}

function readScreenOrientationAngle(targetWindow: PhoneVrBrowserWindow): number | null {
  const angle = targetWindow.screen.orientation?.angle;
  return angle !== undefined && Number.isFinite(angle) ? angle : null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
