import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { transform } from "esbuild";
import { chromium } from "playwright-core";
import { parseScreenRenderAutoSnapshot } from "../../web/src/render/contracts/screen-render-auto.js";
import { summarize } from "./summary.js";

const [address, destination] = process.argv.slice(2);
assert.ok(address !== undefined && destination !== undefined, "Usage: npm run measure:web-auto -- VITE_URL OUTPUT_DIRECTORY");
const url = new URL(address);
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname), "Use the local Vite development server");
const output = resolve(destination);
await mkdir(output, { recursive: true });
const root = resolve(import.meta.dirname, "../..");
const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
execFileSync("git", ["diff", "--exit-code", "HEAD", "--", "web", "crates", "assets"], { cwd: root, stdio: "inherit" });
const browser = await chromium.launch({ headless: true, channel: "chromium" });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });
page.setDefaultTimeout(15_000);
const errors: string[] = [];
const warnings: string[] = [];
const wasm: { url: string; sha256: string; status: number }[] = [];
const responses: Promise<void>[] = [];
const moduleUrls: string[] = [];
const report: Record<string, unknown> = { sourceHead: head, execution: "Vite development server; same Rust core/renderer, not production bundle performance",
  playwright: "1.60.0", browser: browser.version(), headless: true, viewport: { width: 1280, height: 720, dpr: 1 },
  conditions: "Typical / High / neutral input / UI Auto / 2-second Running source window",
  limits: "RAF is headless/display paced (typically 60 Hz). GPU 12.5ms and actual 80 FPS presentation are distinct. Prototype/JSON/RAF observation adds CPU cost; no overhead subtraction.",
  errors, warnings, wasm };
const control = (id: string) => page.locator(`[data-control-id="${id}"]`);
const click = async (id: string) => { await control(id).scrollIntoViewIfNeeded(); await control(id).click(); };
const scene = async (name: string) => page.waitForFunction(expected => document.querySelector(".screen-ui-shell")?.getAttribute("data-scene") === expected, name);
page.on("pageerror", error => { errors.push(error.message); });
page.on("console", message => {
  if (message.type() === "warning") warnings.push(message.text());
  if (message.type() === "error" && !message.location().url.endsWith("/favicon.ico")) errors.push(message.text());
});
page.on("requestfailed", request => { errors.push(`${request.url()}: ${request.failure()?.errorText ?? "request failed"}`); });
page.on("response", response => {
  if (/\/pkg\/birdman_game_wasm\.js(?:\?|$)/.test(response.url()) && moduleUrls.length < 4) moduleUrls.push(response.url());
  if (response.url().endsWith(".wasm")) responses.push(response.body().then(body => {
    wasm.push({ url: response.url(), status: response.status(), sha256: createHash("sha256").update(body).digest("hex") });
  }));
  if (response.status() >= 400 && !response.url().endsWith("/favicon.ico")) errors.push(`HTTP ${String(response.status())} ${response.url()}`);
});
try {
  const injected = await transform(await readFile(resolve(import.meta.dirname, "browser-observer.ts"), "utf8"), { loader: "ts", format: "iife", target: "es2023" });
  await page.addInitScript({ content: injected.code });
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await scene("Title");
  report.build = await page.locator(".build-version").innerText();
  assert.ok(String(report.build).includes(head.slice(0, 8)), "Vite build/source HEAD mismatch");
  const moduleUrl = moduleUrls.at(-1);
  assert.ok(moduleUrl !== undefined, "The existing Vite WASM module was not observed");
  await page.evaluate(async address => {
    const loaded: unknown = await import(address);
    if (typeof loaded !== "object" || loaded === null || !("ScreenRenderAutoBridge" in loaded) || typeof loaded.ScreenRenderAutoBridge !== "function") throw new Error("Existing shared Auto bridge is unavailable");
    const prototype: unknown = Object.getOwnPropertyDescriptor(loaded.ScreenRenderAutoBridge, "prototype")?.value;
    if (typeof prototype !== "object" || prototype === null) throw new Error("Existing shared Auto prototype is unavailable");
    window.__liveAuto.attach(prototype);
  }, moduleUrl);
  report.environment = await page.evaluate(() => {
    const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas");
    const gl = canvas?.getContext("webgl2");
    if (gl === null || gl === undefined) throw new Error("The actual WebGL2 canvas is unavailable");
    const debug = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer: unknown = gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER);
    if (typeof renderer !== "string" || /SwiftShader|llvmpipe|software/i.test(renderer)) throw new Error("Hardware WebGL rendering is required");
    return { userAgent: navigator.userAgent, renderer, visibility: document.visibilityState, focused: document.hasFocus() };
  });
  await click("game-title-start");
  await scene("FlightSetup");
  await click("game-setup-select-weather-2");
  await click("game-setup-select-assistance-3");
  await click("game-setup-start");
  await scene("Briefing");
  report.briefing = await page.locator(".screen-ui-shell").innerText();
  await click("game-briefing-start");
  await scene("Flight");
  if (await page.evaluate(() => window.__liveAuto.phase) === 5) await click("game-flight-pause");
  await control("game-pause-open-settings").waitFor();
  await click("game-pause-open-settings");
  await click("presentation-water-quality-high");
  await page.waitForFunction(() => document.querySelector('[data-control-id="presentation-water-quality-high"]')?.getAttribute("aria-pressed") === "true");
  report.waterQuality = await control("presentation-water-quality-high").getAttribute("aria-pressed");
  await click("presentation-render-scale-100");
  await click("presentation-render-scale-auto");
  const preparationStart = Date.now();
  let preparation;
  do {
    await page.waitForTimeout(250);
    const json = await page.evaluate(() => window.__liveAuto.policyJson);
    assert.ok(json !== null, "The live shared policy was not observed");
    preparation = parseScreenRenderAutoSnapshot(json);
    if (["ready", "coverage_limited", "failed", "unsupported"].includes(preparation.status.kind)) break;
  } while (Date.now() - preparationStart < 8000);
  report.autoPreparation = { elapsedMs: Date.now() - preparationStart, snapshot: preparation,
    settled: preparation.status.kind === "ready" || preparation.status.kind === "coverage_limited",
    scope: "Paused preparation is excluded from the live source window; normal Auto history may retain preparatory frames." };
  assert.ok(preparation.status.kind === "ready" || preparation.status.kind === "coverage_limited", "Auto preparation did not settle within eight seconds");
  await page.screenshot({ path: resolve(output, "01-paused-auto-setup.png") });
  await click("game-pause-settings-back");
  await control("game-flight-resume").waitFor({ state: "visible" });
  await click("game-flight-resume");
  await page.waitForFunction(() => window.__liveAuto.phase === 5);
  const observation = await page.evaluate(() => window.__liveAuto.measure());
  const summary = summarize(observation);
  report.observation = observation;
  report.summary = summary;
  assert.equal(summary.initial.phaseCode, 5);
  assert.equal(summary.initial.identity.scenario.scenarioId, 6, "The actual Rust scenario is not Typical");
  await page.screenshot({ path: resolve(output, "02-live-window-end.png") });
  report.after = await page.evaluate(() => ({ phase: window.__liveAuto.phase, tick: window.__liveAuto.tick,
    snapshotJson: window.__liveAuto.snapshotJson, contextLosses: window.__liveAuto.contextLosses }));
  assert.ok(summary.complete, summary.problems.join("; "));
  assert.equal(await page.evaluate(() => window.__liveAuto.contextLosses), 0);
  assert.deepEqual(errors, []);
  report.status = "observed";
} catch (cause) {
  report.status = "failed";
  report.failure = cause instanceof Error ? { message: cause.message, stack: cause.stack } : String(cause);
  report.failureState = await page.evaluate(() => ({ phase: window.__liveAuto.phase, tick: window.__liveAuto.tick,
    snapshotJson: window.__liveAuto.snapshotJson, policyJson: window.__liveAuto.policyJson, contextLosses: window.__liveAuto.contextLosses })).catch(() => null);
  await page.screenshot({ path: resolve(output, "failure.png") }).catch(() => undefined);
  process.exitCode = 1;
} finally {
  const received = await Promise.allSettled(responses);
  if (wasm.length === 0 || received.some(result => result.status === "rejected")) {
    errors.push("The served WASM response/hash was not completely captured");
    report.status = "failed";
    process.exitCode = 1;
  }
  try { await browser.close(); }
  catch (cause) {
    errors.push(`Browser cleanup failed: ${String(cause)}`);
    report.status = "failed";
    process.exitCode = 1;
  }
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
}
console.log(JSON.stringify({ status: report.status, sourceHead: head, output, summary: report.summary, failure: report.failure }, null, 2));
