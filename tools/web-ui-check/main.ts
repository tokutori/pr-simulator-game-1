import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { transform } from "esbuild";
import { chromium } from "playwright-core";
import type { Browser } from "playwright-core";

declare global {
  interface Window { __uiPauseEvents: readonly { at: number; reason: number }[] }
}

const [address, destination] = process.argv.slice(2);
assert.ok(address !== undefined && destination !== undefined, "Usage: npm run test:web-ui -- VITE_URL OUTPUT_DIRECTORY");
const url = new URL(address);
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname), "Use the local Vite development server");
const output = resolve(destination);
await mkdir(output, { recursive: true });
const root = resolve(import.meta.dirname, "../..");
const head = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
execFileSync("git", ["diff", "--exit-code", "HEAD", "--", "web", "crates", "assets"], { cwd: root, stdio: "inherit" });
let browser: Browser | undefined;
let closing = false;
const errors: string[] = [];
const warnings: string[] = [];
const checks: string[] = [];
const findings: string[] = [];
const wasm: { url: string; sha256: string; status: number }[] = [];
const responses: Promise<void>[] = [];
let moduleUrl: string | undefined;
const report: Record<string, unknown> = { sourceHead: head, execution: "Local Vite; production Rust/WASM, DOM and WebGL2; isolated browser context",
  viewport: { width: 1280, height: 720, dpr: 1 }, errors, warnings, checks, findings, wasm,
  limits: "Screen UI smoke only. No HMD, Phone VR, physical keyboard or 80 FPS acceptance; no performance budget is inferred." };

async function verifyScreen(activeBrowser: Browser): Promise<void> {
  const context = await activeBrowser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, acceptDownloads: true });
  const page = await context.newPage();
  page.setDefaultTimeout(15_000);
  const control = (id: string) => page.locator(`[data-control-id="${id}"]`);
  const click = async (id: string) => { await control(id).scrollIntoViewIfNeeded(); await control(id).click(); };
  const scene = async (name: string) => page.waitForFunction(expected => document.querySelector(".screen-ui-shell")?.getAttribute("data-scene") === expected, name);
  const screenshot = async (name: string) => page.screenshot({ path: resolve(output, `${name}.png`) });
  const download = async (format: "csv" | "json", prefix: string) => {
    const [file] = await Promise.all([page.waitForEvent("download"), click(`game-flight-log-${format}`)]);
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
  page.on("requestfailed", request => {
    const cause = request.failure()?.errorText ?? "request failed";
    if (!closing || cause !== "net::ERR_ABORTED") errors.push(`${request.url()}: ${cause}`);
  });
  page.on("response", response => {
    if (/\/pkg\/birdman_game_wasm\.js(?:\?|$)/.test(response.url())) moduleUrl = response.url();
    if (response.url().endsWith(".wasm")) responses.push(response.body().then(body => {
      wasm.push({ url: response.url(), status: response.status(), sha256: createHash("sha256").update(body).digest("hex") });
    }, (cause: unknown) => { errors.push(`WASM body capture failed: ${String(cause)}`); }));
    if (response.status() >= 400 && !response.url().endsWith("/favicon.ico")) errors.push(`HTTP ${String(response.status())} ${response.url()}`);
  });
  try {
    const observer = await transform(await readFile(resolve(root, "tools/web-auto-check/browser-observer.ts"), "utf8"), { loader: "ts", format: "iife", target: "es2023" });
    await page.addInitScript({ content: observer.code });
    await page.goto(url.href, { waitUntil: "domcontentloaded" });
    await scene("Title");
    report.build = await page.locator(".build-version").innerText();
    assert.ok(String(report.build).includes(head.slice(0, 8)), "Vite build/source HEAD mismatch");
    assert.ok(moduleUrl !== undefined, "The production WASM module was not observed");
    await page.evaluate(async address => {
      const loaded: unknown = await import(address);
      if (typeof loaded !== "object" || loaded === null || !("HybridGameSessionBridge" in loaded) || typeof loaded.HybridGameSessionBridge !== "function") throw new Error("Production session bridge is unavailable");
      const prototype: unknown = Object.getOwnPropertyDescriptor(loaded.HybridGameSessionBridge, "prototype")?.value;
      if (typeof prototype !== "object" || prototype === null) throw new Error("Production session prototype is unavailable");
      const descriptor = Object.getOwnPropertyDescriptor(prototype, "pause");
      if (descriptor === undefined || typeof descriptor.value !== "function") throw new Error("Production pause operation is unavailable");
      const original = descriptor.value as (...args: unknown[]) => unknown;
      const events: { at: number; reason: number }[] = [];
      window.__uiPauseEvents = events;
      Object.defineProperty(prototype, "pause", { ...descriptor, value(this: unknown, ...args: unknown[]): unknown {
        const returned: unknown = Reflect.apply(original, this, args);
        if (typeof args[0] === "number" && events.length < 16) events.push({ at: performance.now(), reason: args[0] });
        return returned;
      } });
    }, moduleUrl);
    report.environment = await page.evaluate(() => {
      const gl = document.querySelector<HTMLCanvasElement>(".presentation-canvas")?.getContext("webgl2");
      if (gl === undefined || gl === null) throw new Error("Production WebGL2 canvas is unavailable");
      const debug = gl.getExtension("WEBGL_debug_renderer_info");
      const renderer: unknown = gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER);
      if (typeof renderer !== "string") throw new Error("WebGL renderer identity is unavailable");
      return { renderer, userAgent: navigator.userAgent, focused: document.hasFocus(), visibility: document.visibilityState };
    });
    await screenshot("00-title");
    await click("game-credits-open");
    await page.getByRole("heading", { name: "クレジット・利用条件", exact: true }).waitFor({ state: "visible" });
    for (const source of ["osm", "jaxa", "copernicus", "jma", "font", "earcut", "ryu", "time"]) {
      assert.ok(await page.locator(`#game-credits-${source}`).count() > 0, `Missing credit: ${source}`);
    }
    await screenshot("00-credits");
    await click("game-credits-close");
    await page.setViewportSize({ width: 360, height: 640 });
    await click("game-title-start");
    await scene("FlightSetup");
    await click("game-setup-select-information-0");
    await click("game-setup-select-assistance-2");
    await click("game-setup-select-weather-0");
    for (const id of ["game-setup-select-information-0", "game-setup-select-assistance-2", "game-setup-select-weather-0"]) {
      assert.equal(await control(id).getAttribute("aria-pressed"), "true");
    }
    report.setup = await page.locator(".screen-ui-shell").innerText();
    assert.match(String(report.setup), /m\/s/);
    assert.doesNotMatch(String(report.setup), /N\/E\/D|未定義|incidence|Synthetic/);
    assert.equal(await page.locator('.screen-ui-choice[aria-pressed="false"] small').count(), 0);
    const selectedDescriptions = await page.locator(".screen-ui-choice small").allTextContents();
    assert.ok(selectedDescriptions.every(value => Array.from(value).length <= 30));
    const overflowing = await page.evaluate(() => {
      const invalid: string[] = [];
      for (const node of document.querySelectorAll<HTMLElement>(".screen-ui-shell button")) {
        const bounds = node.getBoundingClientRect();
        if (bounds.left < -1 || bounds.right > innerWidth + 1 || node.scrollWidth > node.clientWidth + 1) invalid.push(node.dataset.controlId ?? node.textContent);
      }
      for (const node of document.querySelectorAll<HTMLElement>(".screen-ui-choice small")) {
        if (node.getBoundingClientRect().height > Number.parseFloat(getComputedStyle(node).lineHeight) + 1) invalid.push(`説明が複数行: ${node.textContent}`);
      }
      return invalid;
    });
    assert.deepEqual(overflowing, []);
    report.portraitViewport = { width: 360, height: 640, selectedDescriptions };
    await screenshot("01-setup-portrait");
    await page.setViewportSize({ width: 1280, height: 720 });
    await screenshot("01-setup");
    await click("game-setup-start");
    await scene("Briefing");
    const briefingConditions = await page.locator("#game-briefing-conditions").innerText();
    const briefingWeather = await page.locator("#game-weather-conditions").innerText();
    assert.doesNotMatch(await page.locator(".screen-ui-shell").innerText(), /N\/E\/D|未定義|incidence|Synthetic|Seed:|Catalog:/);
    assert.equal(await control("game-briefing-technical").getAttribute("aria-expanded"), "false");
    await click("game-briefing-technical");
    assert.match(await page.locator("#game-briefing-technical-content").innerText(), /Personal Best比較方式: v2/);
    await click("game-briefing-technical");
    await screenshot("02-briefing");
    await page.setViewportSize({ width: 360, height: 640 });
    await control("game-briefing-start").scrollIntoViewIfNeeded();
    assert.ok(await control("game-briefing-start").isVisible());
    await screenshot("02-briefing-portrait");
    await page.setViewportSize({ width: 1280, height: 720 });
    checks.push("共通クレジットの開閉・縦長画面の選択肢・30字以内の選択説明・通常表示からの内部情報分離");
    checks.push("Setup direct choices and actual weather; Briefing initially closed and expandable provenance");
    await click("game-briefing-start");
    await scene("Flight");
    await page.waitForFunction(() => window.__liveAuto.phase === 6 || window.__liveAuto.phase === 5 && (window.__liveAuto.tick ?? 0) >= 10);
    if (await page.evaluate(() => window.__liveAuto.phase) === 6) {
      report.startupPause = await page.evaluate(() => ({ tick: window.__liveAuto.tick,
        snapshotJson: window.__liveAuto.snapshotJson, pauseEvents: window.__uiPauseEvents,
        text: document.querySelector(".screen-ui-shell")?.textContent }));
      findings.push("Flight suspended before the initial Running observation; continued once through the public Resume action");
      await click("game-flight-resume");
    }
    await page.waitForFunction(() => window.__liveAuto.phase === 5 && (window.__liveAuto.tick ?? 0) >= 10);
    const hud = page.locator('.flight-hud[aria-hidden="false"]');
    await hud.waitFor({ state: "visible" });
    const kinds = await hud.locator("[data-kind]").evaluateAll(nodes => [...new Set(nodes.map(node => node.getAttribute("data-kind")))]);
    assert.deepEqual([...kinds].sort(), ["attitude", "direction", "heading_tape", "linear_gauge", "text"]);
    assert.ok(await hud.locator("output").count() > 0);
    assert.equal(await hud.locator("a, details").count(), 0);
    assert.doesNotMatch(await hud.innerText(), /緯度|経度|実機構成は未確認|incidence|intent|Hold|PILOT TARGET|COPERNICUS/);
    report.hudKinds = kinds;
    await click("game-flight-pause");
    await page.waitForFunction(() => window.__liveAuto.phase === 6);
    await screenshot("03-paused-hud");
    const pausedTick = await page.evaluate(() => window.__liveAuto.tick);
    await click("game-credits-open");
    await page.getByRole("heading", { name: "クレジット・利用条件", exact: true }).waitFor({ state: "visible" });
    assert.equal(await page.evaluate(() => window.__liveAuto.phase), 6);
    assert.equal(await page.evaluate(() => window.__liveAuto.tick), pausedTick);
    await screenshot("03-paused-credits");
    await click("game-credits-close");
    await click("game-flight-resume");
    await page.waitForFunction(() => window.__liveAuto.phase === 5);
    await click("game-flight-abort");
    await scene("Result");
    checks.push("Countdown to actual flight ticks; five typed HUD kinds; Pause and Resume; Abort to Result");
    const originalJson = await download("json", "result-record");
    const originalCsv = await download("csv", "result-log");
    const archive: unknown = JSON.parse(originalJson);
    assert.ok(typeof archive === "object" && archive !== null);
    assert.ok("schema_version" in archive && archive.schema_version === 6);
    assert.ok("samples" in archive && Array.isArray(archive.samples) && archive.samples.length > 1);
    assert.ok("finalization" in archive && typeof archive.finalization === "object" && archive.finalization !== null);
    assert.ok("reason" in archive.finalization && archive.finalization.reason === "manual_abort");
    assert.ok("terminal_tick" in archive.finalization && typeof archive.finalization.terminal_tick === "number" && archive.finalization.terminal_tick >= 10);
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
    assert.equal(await page.locator("#game-briefing-conditions").innerText(), briefingConditions);
    assert.equal(await page.locator("#game-weather-conditions").innerText(), briefingWeather);
    assert.equal(await control("game-briefing-technical").getAttribute("aria-expanded"), "false");
    checks.push("Replay returns to Result; Retry retains conditions and closes technical details");
    await click("game-briefing-cancel");
    await click("game-setup-back");
    await scene("Title");
    const saved = page.locator('[data-control-id^="game-title-open-record-"]').first();
    await saved.waitFor({ state: "visible" });
    report.savedRecordLabel = await saved.innerText();
    assert.match(String(report.savedRecordLabel), /\d{4}\/\d{2}\/\d{2} \d{2}:\d{2} UTC/);
    await screenshot("06-saved-title");
    checks.push("一時停止中のCreditsはtickを保持し、保存日時は共通UTC形式で表示する");
    assert.equal(await page.evaluate(() => window.__liveAuto.contextLosses), 0);
    assert.deepEqual(errors, []);
    report.status = findings.length === 0 ? "passed" : "failed";
    if (findings.length > 0) process.exitCode = 1;
  } catch (cause) {
    report.status = "failed";
    report.failure = cause instanceof Error ? { message: cause.message, stack: cause.stack } : String(cause);
    report.failureState = await page.evaluate(() => ({ phase: window.__liveAuto.phase, tick: window.__liveAuto.tick,
      snapshotJson: window.__liveAuto.snapshotJson, pauseEvents: window.__uiPauseEvents,
      contextLosses: window.__liveAuto.contextLosses })).catch(() => null);
    await screenshot("failure").catch(() => undefined);
    process.exitCode = 1;
  }
}

try {
  browser = await chromium.launch({ headless: true, channel: "chromium" });
  report.browser = browser.version();
  await verifyScreen(browser);
} catch (cause) {
  report.status = "failed";
  report.failure = cause instanceof Error ? { message: cause.message, stack: cause.stack } : String(cause);
  process.exitCode = 1;
} finally {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const received = await Promise.race([Promise.allSettled(responses), new Promise<null>(complete => { timer = setTimeout(() => { complete(null); }, 5_000); })]);
  clearTimeout(timer);
  if (received === null || wasm.length === 0 || received.some(result => result.status === "rejected")) {
    errors.push("The served WASM response/hash was not completely captured");
    report.status = "failed";
    process.exitCode = 1;
  }
  closing = true;
  try { await browser?.close(); }
  catch (cause) {
    errors.push(`Browser cleanup failed: ${String(cause)}`);
    report.status = "failed";
    process.exitCode = 1;
  }
  if (errors.length > 0) {
    report.status = "failed";
    process.exitCode = 1;
  }
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
}
console.log(JSON.stringify({ status: report.status, sourceHead: head, output, checks, findings, failure: report.failure }, null, 2));
