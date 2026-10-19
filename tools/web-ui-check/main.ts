import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { transform } from "esbuild";
import { chromium } from "playwright-core";

const [address, destination] = process.argv.slice(2);
assert.ok(address !== undefined && destination !== undefined, "Usage: npm run test:web-ui -- VITE_URL OUTPUT_DIRECTORY");
const url = new URL(address);
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname), "Use the local Vite development server");
const output = resolve(destination);
await mkdir(output, { recursive: true });
const root = resolve(import.meta.dirname, "../..");
const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
execFileSync("git", ["diff", "--exit-code", "HEAD", "--", "web", "crates", "assets"], { cwd: root, stdio: "inherit" });
const browser = await chromium.launch({ headless: true, channel: "chromium" });
const context = await browser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, acceptDownloads: true });
const page = await context.newPage();
page.setDefaultTimeout(15_000);
const errors: string[] = [];
const warnings: string[] = [];
const checks: string[] = [];
const wasm: { url: string; sha256: string; status: number }[] = [];
const responses: Promise<void>[] = [];
const report: Record<string, unknown> = { sourceHead: head, execution: "Local Vite; production Rust/WASM, DOM and WebGL2; isolated browser context",
  browser: browser.version(), viewport: { width: 1280, height: 720, dpr: 1 }, errors, warnings, checks, wasm,
  limits: "Screen UI smoke only. No HMD, Phone VR, physical keyboard or 80 FPS acceptance; no performance budget is inferred." };
const control = (id: string) => page.locator(`[data-control-id="${id}"]`);
const click = async (id: string) => { await control(id).scrollIntoViewIfNeeded(); await control(id).click(); };
const scene = async (name: string) => page.waitForFunction(expected => document.querySelector(".screen-ui-shell")?.getAttribute("data-scene") === expected, name);
const screenshot = async (name: string) => page.screenshot({ path: resolve(output, `${name}.png`) });
const download = async (format: "csv" | "json", prefix: string) => {
  const received = page.waitForEvent("download");
  await click(`game-flight-log-${format}`);
  const file = await received;
  assert.equal(await file.failure(), null);
  assert.ok(file.suggestedFilename().endsWith(`.${format}`));
  const path = resolve(output, `${prefix}.${format}`);
  await file.saveAs(path);
  return readFile(path, "utf8");
};
page.on("pageerror", error => { errors.push(error.message); });
page.on("console", message => {
  if (message.type() === "warning") warnings.push(message.text());
  if (message.type() === "error" && !message.location().url.endsWith("/favicon.ico")) errors.push(message.text());
});
page.on("requestfailed", request => { errors.push(`${request.url()}: ${request.failure()?.errorText ?? "request failed"}`); });
page.on("response", response => {
  if (response.url().endsWith(".wasm")) responses.push(response.body().then(body => {
    wasm.push({ url: response.url(), status: response.status(), sha256: createHash("sha256").update(body).digest("hex") });
  }));
  if (response.status() >= 400 && !response.url().endsWith("/favicon.ico")) errors.push(`HTTP ${String(response.status())} ${response.url()}`);
});
try {
  const observer = await transform(await readFile(resolve(root, "tools/web-auto-check/browser-observer.ts"), "utf8"), { loader: "ts", format: "iife", target: "es2023" });
  await page.addInitScript({ content: observer.code });
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  await scene("Title");
  report.build = await page.locator(".build-version").innerText();
  assert.ok(String(report.build).includes(head.slice(0, 8)), "Vite build/source HEAD mismatch");
  report.environment = await page.evaluate(() => {
    const gl = document.querySelector<HTMLCanvasElement>(".presentation-canvas")?.getContext("webgl2");
    if (gl === undefined || gl === null) throw new Error("Production WebGL2 canvas is unavailable");
    const debug = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer: unknown = gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER);
    if (typeof renderer !== "string") throw new Error("WebGL renderer identity is unavailable");
    return { renderer, userAgent: navigator.userAgent, focused: document.hasFocus(), visibility: document.visibilityState };
  });
  await click("game-title-start");
  await scene("FlightSetup");
  await click("game-setup-select-information-0");
  await click("game-setup-select-assistance-2");
  await click("game-setup-select-weather-0");
  for (const id of ["game-setup-select-information-0", "game-setup-select-assistance-2", "game-setup-select-weather-0"]) {
    assert.equal(await control(id).getAttribute("aria-pressed"), "true");
  }
  report.setup = await page.locator(".screen-ui-shell").innerText();
  assert.match(String(report.setup), /代表水平風速/);
  assert.match(String(report.setup), /鉛直流/);
  await screenshot("01-setup");
  await click("game-setup-start");
  await scene("Briefing");
  assert.equal(await control("game-briefing-technical").getAttribute("aria-expanded"), "false");
  await click("game-briefing-technical");
  assert.match(await page.locator("#game-briefing-technical-content").innerText(), /Personal Best比較方式: v2/);
  await click("game-briefing-technical");
  await screenshot("02-briefing");
  checks.push("Setup direct choices and actual weather; Briefing initially closed and expandable provenance");
  await click("game-briefing-start");
  await scene("Flight");
  await page.waitForFunction(() => window.__liveAuto.phase === 5 && (window.__liveAuto.tick ?? 0) >= 10);
  const hud = page.locator('.flight-hud[aria-hidden="false"]');
  await hud.waitFor({ state: "visible" });
  const kinds = await hud.locator("[data-kind]").evaluateAll(nodes => [...new Set(nodes.map(node => node.getAttribute("data-kind")))]);
  assert.deepEqual([...kinds].sort(), ["attitude", "direction", "heading_tape", "linear_gauge", "text"]);
  assert.ok(await hud.locator("output").count() > 0);
  report.hudKinds = kinds;
  await click("game-flight-pause");
  await page.waitForFunction(() => window.__liveAuto.phase === 6);
  await screenshot("03-paused-hud");
  await click("game-flight-resume");
  await page.waitForFunction(() => window.__liveAuto.phase === 5);
  await click("game-flight-abort");
  await scene("Result");
  checks.push("Countdown to actual flight ticks; five typed HUD kinds; Pause and Resume; Abort to Result");
  const originalJson = await download("json", "result-record");
  const originalCsv = await download("csv", "result-log");
  assert.match(originalJson, /"schema_version"\s*:\s*6/);
  assert.match(originalCsv.split("\n")[1] ?? "", /^3,6,tail_incidence,/);
  report.downloads = { jsonBytes: Buffer.byteLength(originalJson), csvBytes: Buffer.byteLength(originalCsv),
    jsonSha256: createHash("sha256").update(originalJson).digest("hex"), csvSha256: createHash("sha256").update(originalCsv).digest("hex") };
  await click("game-result-open-analysis");
  for (const chart of ["map", "altitude", "speed"]) {
    await click(`game-analysis-${chart}`);
    const plot = page.locator(".screen-ui-chart-viewport svg");
    await plot.waitFor({ state: "visible" });
    const bounds = await plot.boundingBox();
    assert.ok(bounds !== null && bounds.width > 10 && bounds.height > 10, `Invalid ${chart} viewport`);
    assert.ok(await plot.locator("path").count() > 0, `Missing ${chart} geometry`);
    assert.ok(await page.locator(".screen-ui-chart-legend span").count() > 0, `Missing ${chart} legend`);
  }
  await screenshot("04-result-analysis");
  await click("game-result-replay");
  await scene("Replay");
  assert.equal(await control("game-replay-technical").getAttribute("aria-expanded"), "false");
  await click("game-replay-technical");
  assert.match(await page.locator("#game-replay-technical-content").innerText(), /Personal Best比較方式: v2/);
  await click("game-replay-technical");
  await click("game-replay-view-mode");
  await control("game-replay-cursor").waitFor({ state: "visible" });
  await click("game-replay-view-mode");
  await control("game-analysis-speed").waitFor({ state: "visible" });
  assert.equal(await download("json", "replay-record"), originalJson);
  assert.equal(await download("csv", "replay-log"), originalCsv);
  await screenshot("05-replay-analysis");
  checks.push("All three Analysis charts and legends; Replay modes and provenance; actual JSON/CSV downloads remain identical");
  await click("game-replay-return");
  await scene("Result");
  await click("game-result-retry");
  await scene("Briefing");
  assert.equal(await control("game-briefing-technical").getAttribute("aria-expanded"), "false");
  checks.push("Replay returns to Result; Retry retains conditions and closes technical details");
  assert.equal(await page.evaluate(() => window.__liveAuto.contextLosses), 0);
  assert.deepEqual(errors, []);
  report.status = "passed";
} catch (cause) {
  report.status = "failed";
  report.failure = cause instanceof Error ? { message: cause.message, stack: cause.stack } : String(cause);
  report.failureState = await page.evaluate(() => ({ phase: window.__liveAuto.phase, tick: window.__liveAuto.tick,
    snapshotJson: window.__liveAuto.snapshotJson, contextLosses: window.__liveAuto.contextLosses })).catch(() => null);
  await screenshot("failure").catch(() => undefined);
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
console.log(JSON.stringify({ status: report.status, sourceHead: head, output, checks, failure: report.failure }, null, 2));
