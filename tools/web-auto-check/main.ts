import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { transform } from "esbuild";
import { chromium } from "playwright-core";
import type { Browser } from "playwright-core";
import { summarize } from "./summary.js";

const [address, destination, ...options] = process.argv.slice(2);
assert.ok(address !== undefined && destination !== undefined, "Usage: npm run measure:web-auto -- VITE_URL OUTPUT_DIRECTORY [--allow-dirty]");
assert.ok(options.every(option => option === "--allow-dirty"));
const url = new URL(address);
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname));
const root = resolve(import.meta.dirname, "../..");
const output = resolve(destination);
await mkdir(output, { recursive: true });
await writeFile(resolve(output, "result.json"), '{"status":"failed","failure":"GPU observation incomplete"}\n', "utf8");
const gitOptions = { cwd: root, encoding: "utf8" as const, windowsHide: true, maxBuffer: 16 * 1024 * 1024 };
const git = (args: string[]) => execFileSync("git", args, gitOptions);
const diffArguments = ["diff", "--no-ext-diff", "--no-textconv", "--binary", "HEAD"];
const untrackedArguments = ["ls-files", "--others", "--exclude-standard"];
const hash = (bytes: string | Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const sourceHead = git(["rev-parse", "HEAD"]).trim();
const patch = git(diffArguments);
assert.equal(git(untrackedArguments), "", "Add new source files to the index before observation");
assert.ok(options.includes("--allow-dirty") || patch.length === 0);
await writeFile(resolve(output, "source.patch"), patch, "utf8");
const wasmPath = resolve(root, "web/pkg/birdman_game_wasm_bg.wasm");
const wasmSha256 = hash(await readFile(wasmPath));
const errors: string[] = [];
const requestFailures: { at: number; url: string; resourceType: string; errorText: string; afterCloseRequested: boolean }[] = [];
const browserLifecycle: { clock: string; close_requested: number | null; closed: number | null } = {
  clock: "Node performance.now() milliseconds", close_requested: null, closed: null
};
const wasm: { url: string; status: number; sha256: string }[] = [];
const responses: Promise<void>[] = [];
let moduleUrl: string | undefined;
let browser: Browser | undefined;
const report: Record<string, unknown> = { status: "failed", sourceHead, sourceDiffSha256: hash(patch), wasmSha256,
  execution: "Actual production Rust/WASM owner, WebGL GPU completions, DOM controls and browser RAF; Typical/Manual; water ceiling High; Auto rendering",
  limits: "Development/headless GPU work and RAF sample; performance acceptance follows docs/verification.md.", errors, requestFailures, browserLifecycle, wasm };
try {
  browser = await chromium.launch({ headless: true, channel: "chromium" });
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });
  page.setDefaultTimeout(30_000);
  page.on("pageerror", cause => { errors.push(cause.message); });
  page.on("console", message => { if (message.type() === "error") errors.push(`${message.location().url}: ${message.text()}`); });
  page.on("requestfailed", request => {
    const errorText = request.failure()?.errorText ?? "request failed";
    requestFailures.push({ at: performance.now(), url: request.url(), resourceType: request.resourceType(), errorText,
      afterCloseRequested: browserLifecycle.close_requested !== null });
    errors.push(`${request.url()}: ${errorText}`);
  });
  page.on("response", response => {
    const path = new URL(response.url()).pathname;
    if (path.endsWith("/pkg/birdman_game_wasm.js")) moduleUrl = response.url();
    if (response.status() >= 400) errors.push(`HTTP ${String(response.status())}: ${response.url()}`);
    if (path.endsWith(".wasm")) responses.push(response.body().then(bytes => { wasm.push({ url: response.url(), status: response.status(), sha256: hash(bytes) }); }));
  });
  const script = await transform(await readFile(resolve(import.meta.dirname, "browser-observer.ts"), "utf8"), { loader: "ts", format: "iife", target: "es2023" });
  await page.addInitScript({ content: script.code });
  await page.goto(url.href, { waitUntil: "domcontentloaded" });
  const control = (id: string) => page.locator(`[data-control-id="${id}"]`);
  const click = async (id: string) => { await control(id).scrollIntoViewIfNeeded(); await control(id).click(); };
  await control("game-title-start").waitFor();
  assert.ok((await page.locator(".build-version").innerText()).includes(sourceHead.slice(0, 8)), "Restart Vite for the current HEAD");
  assert.ok(moduleUrl !== undefined, "The production WASM module was not observed");
  await page.evaluate(async address => {
    const module = await import(address) as typeof import("../../web/pkg/birdman_game_wasm.js");
    window.__runtimeMeasure.attach(module.ApplicationRuntimeBridge.prototype);
  }, moduleUrl);
  await click("game-title-start");
  await click("game-setup-select-weather-2");
  await click("game-setup-select-assistance-3");
  await click("game-setup-start");
  await click("game-briefing-start");
  await control("game-flight-pause").waitFor();
  await click("game-flight-pause");
  await click("game-pause-open-settings");
  await click("presentation-water-quality-high");
  if (await control("presentation-render-scale-auto").getAttribute("aria-pressed") !== "true") {
    await click("presentation-render-scale-auto");
  }
  await page.waitForFunction(() => {
    const choice = document.querySelector<HTMLButtonElement>('[data-control-id="presentation-render-scale-auto"]');
    return choice?.getAttribute("aria-pressed") === "true" && !choice.disabled;
  });
  await page.screenshot({ path: resolve(output, "prepared-auto.png") });
  await click("game-pause-settings-back");
  await click("game-flight-resume");
  const observation = await page.evaluate(() => window.__runtimeMeasure.measure());
  const summary = summarize(observation);
  report.observation = observation;
  report.summary = summary;
  await page.screenshot({ path: resolve(output, "after-window.png") });
  assert.equal(observation.truncated, false, "Observation capacity exceeded");
  assert.ok(summary.frameCount >= 2 && summary.frames.every(frame => frame.scene === "Flight" && frame.overlay === null), "The source window contains a paused or absent Flight");
  report.status = summary.gpu.length === 0 ? "unavailable" : "observed";
} catch (cause) { errors.push(cause instanceof Error ? `${cause.message}\n${cause.stack ?? ""}` : String(cause)); }
finally {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const received = await Promise.race([Promise.allSettled(responses), new Promise<null>(complete => { timer = setTimeout(() => { complete(null); }, 5000); })]);
  clearTimeout(timer);
  if (received === null || received.some(response => response.status === "rejected") || wasm.length === 0
      || wasm.some(response => response.status !== 200 || response.sha256 !== wasmSha256)) errors.push("Served WASM/hash mismatch");
  try {
    if (browser !== undefined) {
      browserLifecycle.close_requested = performance.now();
      await browser.close();
      browserLifecycle.closed = performance.now();
    }
  } catch (cause) { errors.push(`Browser cleanup: ${String(cause)}`); }
  try {
    assert.equal(git(["rev-parse", "HEAD"]).trim(), sourceHead);
    assert.equal(git(diffArguments), patch);
    assert.equal(git(untrackedArguments), "");
    assert.equal(hash(await readFile(wasmPath)), wasmSha256);
  } catch (cause) { errors.push(`Source guard: ${String(cause)}`); }
  if (errors.length > 0) report.status = "failed";
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
}
if (report.status === "failed") process.exitCode = 1;
console.log(JSON.stringify({ status: report.status, sourceHead, output, summary: report.summary, errors }, null, 2));
