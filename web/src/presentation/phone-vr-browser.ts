import type { PhoneVrAvailability, PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "./phone-vr-contracts.js";

type OrientationPermissionState = "granted" | "denied" | "prompt";

interface DeviceOrientationReadingEvent {
  readonly alpha: number | null;
  readonly beta: number | null;
  readonly gamma: number | null;
  readonly timeStamp: number;
  readonly absolute: boolean;
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
  let absoluteOrientationListener: ((event: DeviceOrientationReadingEvent) => void) | null = null;
  let screenOrientationListener: (() => void) | null = null;
  let listeningGeneration = 0;
  let availability: Promise<PhoneVrAvailability> | null = null;
  let cancelAvailabilityProbe: (() => void) | null = null;

  const probeAvailability = (): Promise<PhoneVrAvailability> => new Promise((resolve) => {
    const probe = { settled: false };
    const finish = (result: PhoneVrAvailability) => {
      if (probe.settled) return;
      probe.settled = true;
      clearTimeout(timer);
      cancelAvailabilityProbe = null;
      let failure: unknown;
      for (const type of ["deviceorientation", "deviceorientationabsolute"]) {
        try { targetWindow.removeEventListener(type, receive); }
        catch (error) { failure ??= error; }
      }
      resolve(failure === undefined ? result : { supported: false, message: "端末姿勢の確認を終了できなかった。ページを再読み込みしてください。" });
    };
    const receive = (event: DeviceOrientationReadingEvent) => {
      if (event.absolute && [event.alpha, event.beta, event.gamma, event.timeStamp].every((value) =>
        typeof value === "number" && Number.isFinite(value)) && readScreenOrientationAngle(targetWindow) !== null) {
        finish({ supported: true, message: "端末姿勢センサーを利用できる。" });
      }
    };
    const timer = setTimeout(() => { finish({ supported: false, message: "端末姿勢を取得できないため、スマートフォンVRは利用できない。" }); }, 1500);
    cancelAvailabilityProbe = () => { finish({ supported: false, message: "端末姿勢の確認を中断した。" }); };
    try {
      targetWindow.addEventListener("deviceorientation", receive);
      if (!probe.settled) targetWindow.addEventListener("deviceorientationabsolute", receive);
    } catch {
      finish({ supported: false, message: "端末姿勢センサーを利用できない。" });
    }
  });

  return Object.freeze({
    checkAvailability(): Promise<PhoneVrAvailability> {
      if (!targetWindow.isSecureContext) return Promise.resolve({ supported: false, message: "スマートフォンVRにはHTTPS接続が必要である。" });
      if (typeof targetWindow.DeviceOrientationEvent === "undefined") {
        return Promise.resolve({ supported: false, message: "このブラウザでは端末姿勢を取得できない。" });
      }
      if (targetWindow.screen.orientation === undefined) {
        return Promise.resolve({ supported: false, message: "このブラウザでは画面の向きを取得できない。" });
      }
      if (!Number.isFinite(targetWindow.screen.orientation.angle)) {
        return Promise.resolve({ supported: false, message: "画面の向きを取得できない。" });
      }
      if (targetWindow.DeviceOrientationEvent.requestPermission !== undefined) {
        return Promise.resolve({ supported: true, message: "切替時に端末姿勢センサーの利用を許可してください。" });
      }
      availability ??= probeAvailability();
      return availability;
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
          : constructor.requestPermission(true);
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
      cancelAvailabilityProbe?.();
      const generation = ++listeningGeneration;
      let source: "deviceorientation" | "deviceorientationabsolute" | null = null;
      const receive = (eventSource: "deviceorientation" | "deviceorientationabsolute", event: DeviceOrientationReadingEvent) => {
        if (generation !== listeningGeneration || (source !== null && source !== eventSource)) return;
        if (event.absolute) source = eventSource;
        readingHandler(Object.freeze({
          alpha: event.alpha,
          beta: event.beta,
          gamma: event.gamma,
          timestampMs: event.timeStamp,
          gravityEvidence: Object.freeze(event.absolute ? { kind: "earth-z-up" } : { kind: "unavailable" })
        }));
      };
      orientationListener = (event) => { receive("deviceorientation", event); };
      absoluteOrientationListener = (event) => { receive("deviceorientationabsolute", event); };
      screenOrientationListener = () => {
        if (generation === listeningGeneration) screenOrientationHandler(readScreenOrientationAngle(targetWindow));
      };
      targetWindow.addEventListener("deviceorientation", orientationListener);
      targetWindow.addEventListener("deviceorientationabsolute", absoluteOrientationListener);
      targetWindow.screen.orientation?.addEventListener("change", screenOrientationListener);
    },
    stopListening(): void {
      cancelAvailabilityProbe?.();
      listeningGeneration++;
      const orientation = orientationListener;
      const absoluteOrientation = absoluteOrientationListener;
      const screenOrientation = screenOrientationListener;
      orientationListener = null;
      absoluteOrientationListener = null;
      screenOrientationListener = null;
      const removals: Array<() => void> = [];
      if (orientation !== null) removals.push(() => { targetWindow.removeEventListener("deviceorientation", orientation); });
      if (absoluteOrientation !== null) removals.push(() => { targetWindow.removeEventListener("deviceorientationabsolute", absoluteOrientation); });
      if (screenOrientation !== null) removals.push(() => { targetWindow.screen.orientation?.removeEventListener("change", screenOrientation); });
      let failure: Error | null = null;
      for (const remove of removals) {
        try { remove(); }
        catch (error) { failure ??= new Error(`Phone VR sensor listener removal failed: ${errorMessage(error)}`); }
      }
      if (failure !== null) throw failure;
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
