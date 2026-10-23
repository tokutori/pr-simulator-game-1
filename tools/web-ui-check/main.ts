import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright-core";
import type { Browser, Page } from "playwright-core";

declare global {
  interface Window { __graphicsLosses: number }
}

const [address, destination, ...options] = process.argv.slice(2);
assert.ok(address !== undefined && destination !== undefined, "Usage: npm run test:web-ui -- VITE_URL OUTPUT_DIRECTORY [--allow-dirty] [--without-gpu-timer]");
assert.ok(options.every(option => ["--allow-dirty", "--without-gpu-timer"].includes(option)), "Unknown Web UI check option");
const url = new URL(address);
assert.ok(["127.0.0.1", "localhost", "[::1]"].includes(url.hostname), "Use the local Vite development server");
const root = resolve(import.meta.dirname, "../..");
const output = resolve(destination);
const gitOptions = { cwd: root, encoding: "utf8" as const, maxBuffer: 16 * 1024 * 1024, windowsHide: true };
const git = (args: string[]) => execFileSync("git", args, gitOptions);
const diffArguments = ["diff", "--no-ext-diff", "--no-textconv", "--binary", "HEAD"];
const untrackedArguments = ["ls-files", "--others", "--exclude-standard"];
const hash = (bytes: string | Uint8Array) => createHash("sha256").update(bytes).digest("hex");
await mkdir(output, { recursive: true });
await writeFile(resolve(output, "result.json"), '{"status":"failed","failure":"Web verification incomplete"}\n', "utf8");
const sourceHead = git(["rev-parse", "HEAD"]).trim();
const sourcePatch = git(diffArguments);
assert.equal(git(untrackedArguments), "", "Add new source files to the index before verification");
assert.ok(options.includes("--allow-dirty") || sourcePatch.length === 0, "Commit source changes or specify --allow-dirty");
await writeFile(resolve(output, "source.patch"), sourcePatch, "utf8");
const wasmPath = resolve(root, "web/pkg/birdman_game_wasm_bg.wasm");
const wasmSha256 = hash(await readFile(wasmPath));
const errors: string[] = [];
const warnings: string[] = [];
const checks: string[] = [];
const wasm: { url: string; status: number; sha256: string }[] = [];
const pendingResponses: Promise<void>[] = [];
const captures: unknown[] = [];
const report: Record<string, unknown> = { status: "failed", sourceHead, sourceDiffSha256: hash(sourcePatch), wasmSha256,
  execution: "Production page, aggregate Rust/WASM owner, actual DOM/Canvas/Three/WebGL2; HD/FHD at DPR 1",
  limits: "Synthetic WebXR engine submission is covered separately. Physical HMD/phone input and presentation-rate acceptance require their own runs.",
  withoutGpuTimer: options.includes("--without-gpu-timer"), errors, warnings, checks, wasm, captures };
let browser: Browser | undefined;

async function snapshot(page: Page, name: string, graphics = true): Promise<void> {
  if (graphics) await page.locator(".presentation-canvas").waitFor({ state: "visible" });
  await page.evaluate(() => new Promise<void>(complete => { requestAnimationFrame(() => { requestAnimationFrame(() => { complete(); }); }); }));
  if (graphics) {
    const observed = await page.evaluate(() => new Promise<{ width: number; height: number; colors: number; nonblack: number; error: number; lost: boolean; losses: number }>((complete, reject) => {
      requestAnimationFrame(() => {
        try {
          const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas");
          const gl = canvas?.getContext("webgl2");
          if (canvas == null || gl == null || canvas.hidden) throw new Error("Production WebGL2 canvas is unavailable");
          const colors = new Set<string>();
          let nonblack = 0;
          const pixel = new Uint8Array(4);
          for (let row = 1; row < 8; row++) for (let column = 1; column < 12; column++) {
            gl.readPixels(Math.floor(canvas.width * column / 12), Math.floor(canvas.height * row / 8), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);
            colors.add(pixel.join(","));
            if (pixel[0] !== 0 || pixel[1] !== 0 || pixel[2] !== 0) nonblack++;
          }
          complete({ width: canvas.width, height: canvas.height, colors: colors.size, nonblack, error: gl.getError(), lost: gl.isContextLost(), losses: window.__graphicsLosses });
        } catch (cause) { reject(cause instanceof Error ? cause : new Error(String(cause))); }
      });
    }));
    assert.ok(observed.nonblack > 8 && observed.colors > 4, `${name}: missing scene pixels`);
    assert.equal(observed.error, 0, `${name}: WebGL error`);
    assert.equal(observed.lost, false, `${name}: context lost`);
    assert.equal(observed.losses, 0, `${name}: context loss event`);
    captures.push({ name, viewport: page.viewportSize(), graphics: observed });
  } else captures.push({ name, viewport: page.viewportSize() });
  await page.screenshot({ path: resolve(output, `${name}.png`), fullPage: false });
}

async function verify(activeBrowser: Browser): Promise<void> {
  const context = await activeBrowser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, acceptDownloads: true });
  try {
    await context.addInitScript(withoutTimer => {
      window.__graphicsLosses = 0;
      document.addEventListener("webglcontextlost", () => { window.__graphicsLosses++; }, true);
      if (withoutTimer) {
        const original = Object.getOwnPropertyDescriptor(WebGL2RenderingContext.prototype, "getExtension")?.value as WebGL2RenderingContext["getExtension"] | undefined;
        if (original === undefined) throw new Error("WebGL2 extension lookup is unavailable");
        Object.defineProperty(WebGL2RenderingContext.prototype, "getExtension", { configurable: true, value(this: WebGL2RenderingContext, name: string): unknown {
          return name === "EXT_disjoint_timer_query_webgl2" ? null : Reflect.apply(original, this, [name]);
        } });
      }
    }, options.includes("--without-gpu-timer"));
    const page = await context.newPage();
    page.setDefaultTimeout(30_000);
    page.on("pageerror", cause => { errors.push(cause.message); });
    page.on("console", message => {
      if (message.type() === "error") errors.push(`${message.location().url}: ${message.text()}`);
      if (message.type() === "warning") warnings.push(`${message.location().url}: ${message.text()}`);
    });
    page.on("requestfailed", request => { errors.push(`${request.url()}: ${request.failure()?.errorText ?? "request failed"}`); });
    page.on("response", response => {
      if (response.status() >= 400) errors.push(`HTTP ${String(response.status())}: ${response.url()}`);
      if (new URL(response.url()).pathname.endsWith(".wasm")) pendingResponses.push(response.body().then(bytes => {
        wasm.push({ url: response.url(), status: response.status(), sha256: hash(bytes) });
      }));
    });
    const control = (id: string) => page.locator(`[data-control-id="${id}"]`);
    const click = async (id: string) => { await control(id).scrollIntoViewIfNeeded(); await control(id).click(); };
    const scene = async (name: string) => page.waitForFunction(value => document.querySelector(".screen-ui-shell")?.getAttribute("data-scene") === value, name);
    const pair = async (name: string, graphics = true) => {
      for (const [width, height, size] of [[1280, 720, "hd"], [1920, 1080, "fhd"]] as const) {
        await page.setViewportSize({ width, height });
        await snapshot(page, `${name}-${size}`, graphics);
      }
    };
    await page.goto(url.href, { waitUntil: "domcontentloaded" });
    await scene("Title");
    report.build = await page.locator(".build-version").innerText();
    assert.ok(String(report.build).includes(sourceHead.slice(0, 8)), "Restart Vite for the current source HEAD before verification");
    report.graphics = await page.evaluate(() => {
      const gl = document.querySelector<HTMLCanvasElement>(".presentation-canvas")?.getContext("webgl2");
      if (gl == null) throw new Error("WebGL2 renderer was not created");
      const debug = gl.getExtension("WEBGL_debug_renderer_info");
      return { renderer: String(gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER)), version: String(gl.getParameter(gl.VERSION)), userAgent: navigator.userAgent };
    });
    await pair("title");
    checks.push("Title: production scene pixels and controls at HD/FHD");
    await click("game-title-start");
    await scene("FlightSetup");
    await click("game-setup-select-weather-0");
    await click("game-setup-select-assistance-3");
    await pair("setup-calm");
    await click("game-setup-start");
    await scene("Briefing");
    await control("game-briefing-start").waitFor({ state: "visible" });
    await pair("briefing");
    await click("game-briefing-start");
    await scene("Flight");
    await control("game-flight-pause").waitFor({ state: "visible" });
    await page.waitForTimeout(400);
    assert.equal(await control("game-flight-resume").isVisible(), false, "Flight unexpectedly paused");
    await click("game-flight-pause");
    await control("game-flight-resume").waitFor({ state: "visible" });
    await pair("pause");
    checks.push("Flight: shared Countdown, running frame, Pause and HD/FHD HUD");
    await click("game-pause-open-settings");
    for (const quality of ["low", "medium", "high"]) {
      await click(`presentation-water-quality-${quality}`);
      await page.waitForFunction(id => {
        const button = document.querySelector<HTMLButtonElement>(`[data-control-id="${id}"]`);
        return button?.getAttribute("aria-pressed") === "true" && !button.disabled;
      }, `presentation-water-quality-${quality}`);
      await snapshot(page, `water-${quality}`);
    }
    await click("game-pause-settings-back");
    await click("game-flight-resume");
    await page.waitForTimeout(200);
    assert.equal(await control("game-flight-resume").isVisible(), false, "Resume did not continue Flight");
    await click("game-flight-abort");
    await scene("Result");
    await pair("result");
    await click("game-result-technical");
    const download = async (format: "json" | "csv", name: string) => {
      const [file] = await Promise.all([page.waitForEvent("download"), click(`game-flight-log-${format}`)]);
      const path = resolve(output, `${name}.${format}`);
      await file.saveAs(path);
      const bytes = await readFile(path);
      assert.ok(bytes.length > 0, `${format}: empty download`);
      return bytes;
    };
    const original = await download("json", "record");
    const csv = await download("csv", "record");
    assert.ok(JSON.parse(original.toString("utf8")) !== null);
    await click("game-result-open-analysis");
    for (const chart of ["map", "altitude", "speed"]) {
      await click(`game-analysis-${chart}`);
      await pair(`analysis-${chart}`, false);
    }
    await click("game-result-replay");
    await scene("Replay");
    await pair("replay");
    await click("game-replay-technical");
    assert.deepEqual(await download("json", "replay-record"), original, "Replay changed the original JSON");
    assert.deepEqual(await download("csv", "replay-record"), csv, "Replay changed the exported CSV");
    checks.push("Result/Analysis/Replay: actual charts, HD/FHD scenes and byte-identical JSON/CSV downloads");
    await click("game-replay-return");
    await click("game-result-retry");
    await scene("Briefing");
    await click("game-briefing-cancel");
    await click("game-setup-back");
    await scene("Title");
    await snapshot(page, "returned-title");
    checks.push("Retry and Title: retained controls and environment resource replacement");
  } finally { await context.close(); }
}

try {
  browser = await chromium.launch({ headless: true, channel: "chromium" });
  report.browser = browser.version();
  await verify(browser);
  report.status = "passed";
} catch (cause) {
  errors.push(cause instanceof Error ? `${cause.message}\n${cause.stack ?? ""}` : String(cause));
} finally {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const responses = await Promise.race([Promise.allSettled(pendingResponses), new Promise<null>(complete => { timer = setTimeout(() => { complete(null); }, 5000); })]);
  clearTimeout(timer);
  if (responses === null || responses.some(response => response.status === "rejected") || wasm.length === 0
      || wasm.some(response => response.status !== 200 || response.sha256 !== wasmSha256)) errors.push("Served WASM response/hash does not match the captured local artifact");
  try { await browser?.close(); } catch (cause) { errors.push(`Browser cleanup: ${String(cause)}`); }
  try {
    assert.equal(git(["rev-parse", "HEAD"]).trim(), sourceHead, "HEAD changed during verification");
    assert.equal(git(diffArguments), sourcePatch, "Source changed during verification");
    assert.equal(git(untrackedArguments), "", "New source appeared during verification");
    assert.equal(hash(await readFile(wasmPath)), wasmSha256, "WASM changed during verification");
  } catch (cause) { errors.push(String(cause)); }
  if (errors.length > 0) report.status = "failed";
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
}
if (report.status !== "passed") process.exitCode = 1;
console.log(JSON.stringify({ status: report.status, sourceHead, output, checks, errors }, null, 2));
