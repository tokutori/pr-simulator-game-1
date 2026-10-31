import "./styles.css";
import initializeWasm from "../pkg/birdman_game_wasm.js";
import { APPLICATION_FAILURE_HTML, APPLICATION_LOADING_HTML } from "./generated/application-startup.js";
import { BrowserApplication } from "./browser-application.js";

const icon = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
if (icon !== null) icon.href = new URL("../../assets/favicon.svg", import.meta.url).href;

const mount = document.getElementById("app");
if (!(mount instanceof HTMLElement)) throw new Error("Required app element is missing");

function startup(html: string): void {
  if (mount === null) return;
  mount.innerHTML = html;
  mount.querySelector<HTMLButtonElement>("#application-reload")?.addEventListener("click", () => { window.location.reload(); });
}

startup(APPLICATION_LOADING_HTML);
try {
  await initializeWasm();
  await document.fonts.ready;
  const hmd = navigator.xr === undefined ? false : await navigator.xr.isSessionSupported("immersive-vr").catch((cause: unknown) => { console.error(cause); return false; });
  new BrowserApplication(mount, {
    analysis_available: true,
    flight_log_available: typeof URL.createObjectURL === "function",
    hmd_available: hmd,
    touch_available: false,
    phone_sensors: window.isSecureContext && "DeviceOrientationEvent" in window && "orientation" in screen ? {
      permission_required: typeof (DeviceOrientationEvent as typeof DeviceOrientationEvent & { requestPermission?: unknown }).requestPermission === "function",
      screen_angle_degrees: screen.orientation.angle
    } : null,
    replay_available: true,
    stored_records_available: "indexedDB" in window
  });
} catch (cause) {
  console.error(cause);
  startup(APPLICATION_FAILURE_HTML);
}
