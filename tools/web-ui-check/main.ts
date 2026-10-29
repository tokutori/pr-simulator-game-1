import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright-core";
import type { Browser, Page } from "playwright-core";
import type { ApplicationRuntimeBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { ApplicationInput, ApplicationObservation, ApplicationOutput, ApplicationRequest } from "../../web/src/generated/application-contracts.js";

declare global {
  interface Window {
    __graphicsLosses: number;
    __adapterProbe: { active: boolean; axes: number[]; latest: unknown; rows: { observed_ms: number; input: ApplicationInput; consumed: boolean; water_time: number | null; scene: string | null; overlay: string | null }[] };
    __recoveryRead: { requestSucceeded: boolean; aborts: number; error: string | null };
    __recoveryGpu: { framebufferFailures: number; partialTextureCreated: boolean; partialTextureDeletes: number; partialFramebufferCreated: boolean; partialFramebufferDeletes: number; canvas: HTMLCanvasElement | OffscreenCanvas | null };
    __recoveryCompile: {
      hold: boolean; polls: number; heldReadyPolls: number; contextLost: boolean;
      textureDeletes: number; framebufferDeletes: number; requestId: number | null;
      inputs: Extract<ApplicationInput, { kind: "resources" }>[];
      requests: Extract<ApplicationRequest, { kind: "cancel_resources" | "commit_resources" }>[];
      restore(): void;
    };
  }
}

const [address, destination, ...options] = process.argv.slice(2);
assert.ok(address !== undefined && destination !== undefined, "Usage: npm run test:web-ui -- VITE_URL OUTPUT_DIRECTORY [--allow-dirty] [--without-gpu-timer] [--recovery]");
assert.ok(options.every(option => ["--allow-dirty", "--without-gpu-timer", "--recovery", "--adapter-observe"].includes(option)), "Unknown Web UI check option");
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
const chartInteractions: { chart: string; before: string; after: string }[] = [];
const recovery = options.includes("--recovery");
let phase = "startup";
const diagnostics: { phase: string; source: string; url: string; text: string }[] = [];
const expectedDiagnostics: { phase: string; source: string; text: string; url?: string; observed: typeof diagnostics }[] = [];
const wasmFailure = { url: "", status: 503, sha256: hash("Injected WASM request failure") };
const report: Record<string, unknown> = { status: "failed", sourceHead, sourceDiffSha256: hash(sourcePatch), wasmSha256,
  execution: "Production page, aggregate Rust/WASM owner, actual DOM/Canvas/Three/WebGL2; HD/FHD at DPR 1",
  limits: "Synthetic WebXR engine submission is covered separately. Physical HMD/phone input and presentation-rate acceptance require their own runs.",
  withoutGpuTimer: options.includes("--without-gpu-timer"), recovery, errors, warnings, checks, wasm, captures, chartInteractions,
  diagnostics, expectedDiagnostics };
let browser: Browser | undefined;

async function snapshot(page: Page, name: string, graphics = true): Promise<void> {
  await page.evaluate(() => new Promise<void>(complete => { requestAnimationFrame(() => { requestAnimationFrame(() => { complete(); }); }); }));
  const chart = await page.evaluate(() => {
    const shell = document.querySelector<HTMLElement>(".screen-ui-shell");
    const viewport = document.querySelector<HTMLElement>(".screen-ui-chart-viewport");
    if (shell == null || viewport == null) return null;
    const shellRect = shell.getBoundingClientRect();
    const chartRect = viewport.getBoundingClientRect();
    return {
      rootFontSize: getComputedStyle(document.documentElement).fontSize,
      shell: { left: shellRect.left, top: shellRect.top, width: shellRect.width, height: shellRect.height },
      viewport: { left: chartRect.left, top: chartRect.top, width: chartRect.width, height: chartRect.height },
      coordinateExtent: viewport.querySelector("svg")?.getAttribute("viewBox"),
      fields: Array.from(document.querySelectorAll<HTMLElement>(".screen-ui-field"), element => ({
        id: element.dataset.fieldId, label: element.querySelector("dt")?.textContent,
        value: element.querySelector(".screen-ui-field-value")?.textContent,
        unit: element.querySelector(".screen-ui-field-unit")?.textContent
      }))
    };
  });
  if (graphics) {
    const observed = await page.evaluate(() => new Promise<{ width: number; height: number; colors: number; nonblack: number; error: number; lost: boolean; losses: number }>((complete, reject) => {
      let frame = 0;
      const timer = setTimeout(() => { cancelAnimationFrame(frame); reject(new Error("Production scene preparation timed out")); }, 30_000);
      const sample: [FrameRequestCallback] = [() => {
        try {
          if (window.__graphicsLosses !== 0) throw new Error("Production WebGL2 context was lost");
          const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas");
          if (canvas == null || canvas.hidden) { frame = requestAnimationFrame(sample[0]); return; }
          const gl = canvas.getContext("webgl2");
          if (gl == null) throw new Error("Production WebGL2 context is unavailable");
          const colors = new Set<string>();
          let nonblack = 0;
          const pixel = new Uint8Array(4);
          for (let row = 1; row < 8; row++) for (let column = 1; column < 12; column++) {
            gl.readPixels(Math.floor(canvas.width * column / 12), Math.floor(canvas.height * row / 8), 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);
            colors.add(pixel.join(","));
            if (pixel[0] !== 0 || pixel[1] !== 0 || pixel[2] !== 0) nonblack++;
          }
          clearTimeout(timer);
          complete({ width: canvas.width, height: canvas.height, colors: colors.size, nonblack, error: gl.getError(), lost: gl.isContextLost(), losses: window.__graphicsLosses });
        } catch (cause) { clearTimeout(timer); reject(cause instanceof Error ? cause : new Error(String(cause))); }
      }];
      frame = requestAnimationFrame(sample[0]);
    }));
    captures.push({ name, viewport: page.viewportSize(), graphics: observed, chart });
    assert.ok(observed.nonblack > 8 && observed.colors > 4, `${name}: missing scene pixels`);
    assert.equal(observed.error, 0, `${name}: WebGL error`);
    assert.equal(observed.lost, false, `${name}: context lost`);
    assert.equal(observed.losses, 0, `${name}: context loss event`);
  } else captures.push({ name, viewport: page.viewportSize(), chart });
  if (graphics) await page.locator(".presentation-canvas").waitFor({ state: "visible" });
  await page.screenshot({ path: resolve(output, `${name}.png`), fullPage: false });
}

async function verify(activeBrowser: Browser): Promise<void> {
  const context = await activeBrowser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, acceptDownloads: true });
  try {
    await context.addInitScript(({ withoutTimer, recovery }) => {
      window.__graphicsLosses = 0;
      window.addEventListener("error", event => {
        console.error("window.error", JSON.stringify({ at: performance.now(),
          message: event instanceof ErrorEvent ? event.message : String(event),
          filename: event instanceof ErrorEvent ? event.filename : null,
          line: event instanceof ErrorEvent ? event.lineno : null,
          column: event instanceof ErrorEvent ? event.colno : null
        }));
      }, true);
      document.addEventListener("webglcontextlost", () => { window.__graphicsLosses++; }, true);
      if (withoutTimer) {
        const original = Object.getOwnPropertyDescriptor(WebGL2RenderingContext.prototype, "getExtension")?.value as WebGL2RenderingContext["getExtension"] | undefined;
        if (original === undefined) throw new Error("WebGL2 extension lookup is unavailable");
        Object.defineProperty(WebGL2RenderingContext.prototype, "getExtension", { configurable: true, value(this: WebGL2RenderingContext, name: string): unknown {
          return name === "EXT_disjoint_timer_query_webgl2" ? null : Reflect.apply(original, this, [name]);
        } });
      }
      if (recovery) {
        const original = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, "getContext")?.value as HTMLCanvasElement["getContext"] | undefined;
        if (original === undefined) throw new Error("Canvas context lookup is unavailable");
        Object.defineProperty(HTMLCanvasElement.prototype, "getContext", { configurable: true, writable: true,
          value(this: HTMLCanvasElement, kind: string, ...parameters: unknown[]): unknown {
            if (kind === "webgl2") {
              Object.defineProperty(HTMLCanvasElement.prototype, "getContext", { configurable: true, writable: true, value: original });
              throw new Error("Injected WebGL2 acquisition failure");
            }
            return Reflect.apply(original, this, [kind, ...parameters]);
          }
        });
        const extension = Object.getOwnPropertyDescriptor(WebGL2RenderingContext.prototype, "getExtension")?.value as WebGL2RenderingContext["getExtension"] | undefined;
        if (extension === undefined) throw new Error("WebGL2 extension lookup is unavailable");
        Object.defineProperty(WebGL2RenderingContext.prototype, "getExtension", { configurable: true, value(this: WebGL2RenderingContext, name: string): unknown {
          if (name === "EXT_disjoint_timer_query_webgl2") {
            Object.defineProperty(WebGL2RenderingContext.prototype, "getExtension", { configurable: true, writable: true, value: extension });
            throw new Error("Injected WebGL2 observation failure");
          }
          return Reflect.apply(extension, this, [name]);
        } });
      }
    }, { withoutTimer: options.includes("--without-gpu-timer"), recovery });
    const page = await context.newPage();
    if (options.includes("--adapter-observe")) await page.clock.install();
    page.setDefaultTimeout(30_000);
    page.on("pageerror", cause => { diagnostics.push({ phase, source: "page", url: page.url(), text: cause.message }); });
    page.on("console", message => {
      if (message.type() === "error") diagnostics.push({ phase, source: "console", url: message.location().url, text: message.text() });
      if (message.type() === "warning") warnings.push(`${message.location().url}: ${message.text()}`);
    });
    page.on("requestfailed", request => { diagnostics.push({ phase, source: "request", url: request.url(), text: request.failure()?.errorText ?? "request failed" }); });
    page.on("response", response => {
      if (response.status() >= 400) diagnostics.push({ phase, source: "http", url: response.url(), text: `HTTP ${String(response.status())}` });
      if (new URL(response.url()).pathname.endsWith(".wasm")) pendingResponses.push(response.body().then(bytes => {
        wasm.push({ url: response.url(), status: response.status(), sha256: hash(bytes) });
      }));
    });
    const control = (id: string) => page.locator(`[data-control-id="${id}"]`);
    const click = async (id: string) => { await control(id).scrollIntoViewIfNeeded(); await control(id).click(); };
    const scene = async (name: string) => page.waitForFunction(value => document.querySelector(".screen-ui-shell")?.getAttribute("data-scene") === value, name);
    const chartSettled = () => page.waitForFunction(() => {
      const viewport = document.querySelector<HTMLElement>(".screen-ui-chart-viewport");
      if (viewport === null) return true;
      const svg = viewport.querySelector("svg");
      if (svg === null) return false;
      const bounds = viewport.getBoundingClientRect();
      const extent = svg.viewBox.baseVal;
      return bounds.width > 0 && bounds.height > 0 && Math.abs(bounds.width - extent.width) < 0.5 && Math.abs(bounds.height - extent.height) < 0.5;
    });
    const pair = async (name: string, graphics = true) => {
      for (const [width, height, size] of [[1280, 720, "hd"], [1920, 1080, "fhd"]] as const) {
        await page.setViewportSize({ width, height });
        await chartSettled();
        await snapshot(page, `${name}-${size}`, graphics);
      }
    };
    if (recovery) {
      phase = "wasm-failure";
      await page.route("**/*.wasm", async route => {
        if (wasmFailure.url !== "") { await route.continue(); return; }
        wasmFailure.url = route.request().url();
        expectedDiagnostics.push(
          { phase, source: "http", url: wasmFailure.url, text: "HTTP 503", observed: [] },
          { phase, source: "console", url: wasmFailure.url, text: "Failed to load resource: the server responded with a status of 503 (Service Unavailable)", observed: [] },
          { phase, source: "console", text: `Error: failed to fetch Wasm: 503 Service Unavailable fetching '${wasmFailure.url}'`, observed: [] }
        );
        await route.fulfill({ status: 503, contentType: "application/wasm", body: "Injected WASM request failure" });
      });
    }
    await page.goto(url.href, { waitUntil: "domcontentloaded" });
    if (recovery) {
      await page.getByText("ゲームを読み込めない。ページを再読み込みして再試行する。", { exact: true }).waitFor();
      await snapshot(page, "recovery-wasm-failed", false);
      assert.equal(await page.locator(".screen-ui-shell").count(), 0, "Failed WASM started the application owner");
      phase = "webgl-failure";
      expectedDiagnostics.push(
        { phase, source: "console", text: "THREE.WebGLRenderer: Injected WebGL2 acquisition failure", observed: [] },
        { phase, source: "console", text: "Error: Injected WebGL2 acquisition failure", observed: [] }
      );
      await page.locator("#application-reload").click();
      await scene("Boot");
      await control("game-boot-retry").waitFor();
      await page.getByText("表示を準備できない。再試行するか、ページを再読み込みする。", { exact: true }).waitFor();
      await snapshot(page, "recovery-webgl-failed", false);
      assert.equal(await page.locator(".presentation-canvas").count(), 0, "Failed renderer retained its canvas");
      phase = "renderer-observation-failure";
      expectedDiagnostics.push({ phase, source: "console", text: "Error: Injected WebGL2 observation failure", observed: [] });
      await click("game-boot-retry");
      await scene("Boot");
      await page.getByText("表示を準備できない。再試行するか、ページを再読み込みする。", { exact: true }).waitFor();
      await snapshot(page, "recovery-renderer-observation-failed", false);
      assert.equal(await page.locator(".presentation-canvas").count(), 0, "Failed renderer observation retained its canvas");
      phase = "framebuffer-failure";
      expectedDiagnostics.push({ phase, source: "console", text: "AggregateError: GPU resource creation failed (Error: Injected framebuffer attachment failure)", observed: [] });
      await page.evaluate(() => {
        const prototype = WebGL2RenderingContext.prototype;
        const deleteTexture = Object.getOwnPropertyDescriptor(prototype, "deleteTexture")?.value as WebGL2RenderingContext["deleteTexture"] | undefined;
        const deleteFramebuffer = Object.getOwnPropertyDescriptor(prototype, "deleteFramebuffer")?.value as WebGL2RenderingContext["deleteFramebuffer"] | undefined;
        const framebufferTexture2D = Object.getOwnPropertyDescriptor(prototype, "framebufferTexture2D")?.value as WebGL2RenderingContext["framebufferTexture2D"] | undefined;
        if (deleteTexture === undefined || deleteFramebuffer === undefined || framebufferTexture2D === undefined) throw new Error("WebGL2 resource APIs are unavailable");
        const observed: Window["__recoveryGpu"] = { framebufferFailures: 0, partialTextureCreated: false, partialTextureDeletes: 0,
          partialFramebufferCreated: false, partialFramebufferDeletes: 0, canvas: null };
        window.__recoveryGpu = observed;
        let partialTexture: WebGLTexture | null = null;
        let partialFramebuffer: WebGLFramebuffer | null = null;
        Object.defineProperty(prototype, "deleteTexture", { configurable: true, writable: true, value(this: WebGL2RenderingContext, texture: WebGLTexture | null): void {
          Reflect.apply(deleteTexture, this, [texture]);
          if (observed.framebufferFailures === 1 && texture !== null && texture === partialTexture) observed.partialTextureDeletes++;
        } });
        Object.defineProperty(prototype, "deleteFramebuffer", { configurable: true, writable: true, value(this: WebGL2RenderingContext, framebuffer: WebGLFramebuffer | null): void {
          Reflect.apply(deleteFramebuffer, this, [framebuffer]);
          if (observed.framebufferFailures === 1 && framebuffer !== null && framebuffer === partialFramebuffer) observed.partialFramebufferDeletes++;
        } });
        Object.defineProperty(prototype, "framebufferTexture2D", { configurable: true, writable: true,
          value(this: WebGL2RenderingContext, target: number, attachment: number, textureTarget: number, texture: WebGLTexture | null, level: number): void {
            if (target === this.FRAMEBUFFER && attachment === this.COLOR_ATTACHMENT0 && textureTarget === this.TEXTURE_2D && texture !== null && level === 0) {
              const binding: unknown = this.getParameter(this.FRAMEBUFFER_BINDING);
              if (binding instanceof WebGLFramebuffer) {
                Object.defineProperty(prototype, "framebufferTexture2D", { configurable: true, writable: true, value: framebufferTexture2D });
                partialTexture = texture;
                partialFramebuffer = binding;
                observed.framebufferFailures++;
                observed.partialTextureCreated = true;
                observed.partialFramebufferCreated = true;
                observed.canvas = this.canvas;
                throw new Error("Injected framebuffer attachment failure");
              }
            }
            Reflect.apply(framebufferTexture2D, this, [target, attachment, textureTarget, texture, level]);
          }
        });
      });
      await click("game-boot-retry");
      await scene("Boot");
      await page.getByText("表示を準備できない。再試行するか、ページを再読み込みする。", { exact: true }).waitFor();
      await snapshot(page, "recovery-framebuffer-failed", false);
      const partial = await page.evaluate(() => ({ framebufferFailures: window.__recoveryGpu.framebufferFailures,
        partialTextureCreated: window.__recoveryGpu.partialTextureCreated, partialTextureDeletes: window.__recoveryGpu.partialTextureDeletes,
        partialFramebufferCreated: window.__recoveryGpu.partialFramebufferCreated, partialFramebufferDeletes: window.__recoveryGpu.partialFramebufferDeletes }));
      report.recoveryGpu = partial;
      assert.deepEqual(partial, { framebufferFailures: 1, partialTextureCreated: true, partialTextureDeletes: 1,
        partialFramebufferCreated: true, partialFramebufferDeletes: 1 }, "Failed GPU candidate did not release its partial texture and framebuffer exactly once");
      phase = "delayed-compilation";
      const supported = await page.evaluate(async () => {
        const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas");
        const gl = canvas?.getContext("webgl2");
        if (gl == null) throw new Error("Recovery renderer context is unavailable");
        const extension = gl.getExtension("KHR_parallel_shader_compile");
        if (extension === null) return false;
        const prototype = WebGL2RenderingContext.prototype;
        const getProgramParameter = Object.getOwnPropertyDescriptor(prototype, "getProgramParameter")?.value as WebGL2RenderingContext["getProgramParameter"] | undefined;
        const deleteTexture = Object.getOwnPropertyDescriptor(prototype, "deleteTexture")?.value as WebGL2RenderingContext["deleteTexture"] | undefined;
        const deleteFramebuffer = Object.getOwnPropertyDescriptor(prototype, "deleteFramebuffer")?.value as WebGL2RenderingContext["deleteFramebuffer"] | undefined;
        const framebufferTexture2D = Object.getOwnPropertyDescriptor(prototype, "framebufferTexture2D")?.value as WebGL2RenderingContext["framebufferTexture2D"] | undefined;
        const runtime = await import(new URL("pkg/birdman_game_wasm.js", location.href).href) as typeof import("../../web/pkg/birdman_game_wasm.js");
        const update = Object.getOwnPropertyDescriptor(runtime.ApplicationRuntimeBridge.prototype, "update")?.value as ApplicationRuntimeBridge["update"] | undefined;
        if (getProgramParameter === undefined || deleteTexture === undefined || deleteFramebuffer === undefined || framebufferTexture2D === undefined || update === undefined) throw new Error("Recovery I/O observation APIs are unavailable");
        let texture: WebGLTexture | null = null;
        let framebuffer: WebGLFramebuffer | null = null;
        const observed: Window["__recoveryCompile"] = { hold: true, polls: 0, heldReadyPolls: 0, contextLost: false,
          textureDeletes: 0, framebufferDeletes: 0, requestId: null, inputs: [], requests: [], restore() {
            observed.hold = false;
            for (const [name, value] of [["getProgramParameter", getProgramParameter], ["deleteTexture", deleteTexture],
              ["deleteFramebuffer", deleteFramebuffer], ["framebufferTexture2D", framebufferTexture2D]] as const) {
              Object.defineProperty(prototype, name, { configurable: true, writable: true, value });
            }
            runtime.ApplicationRuntimeBridge.prototype.update = update;
            canvas?.removeEventListener("webglcontextlost", lost[0]);
          } };
        window.__recoveryCompile = observed;
        const lost: [() => void] = [() => { observed.contextLost = true; observed.hold = false; }];
        canvas?.addEventListener("webglcontextlost", lost[0]);
        runtime.ApplicationRuntimeBridge.prototype.update = function (this: ApplicationRuntimeBridge, ...args: Parameters<typeof update>): ReturnType<typeof update> {
          const result = Reflect.apply(update, this, args);
          const input = (JSON.parse(args[0]) as ApplicationObservation).input;
          const output = JSON.parse(result) as ApplicationOutput;
          if (observed.requestId === null) observed.requestId = output.requests.find(request => request.kind === "prepare_resources")?.request_id ?? null;
          if (input.kind === "resources" && input.request_id === observed.requestId) observed.inputs.push(input);
          for (const request of output.requests) if ((request.kind === "cancel_resources" || request.kind === "commit_resources") && request.request_id === observed.requestId) observed.requests.push(request);
          return result;
        };
        Object.defineProperty(prototype, "framebufferTexture2D", { configurable: true, writable: true,
          value(this: WebGL2RenderingContext, target: number, attachment: number, textureTarget: number, candidate: WebGLTexture | null, level: number): void {
            Reflect.apply(framebufferTexture2D, this, [target, attachment, textureTarget, candidate, level]);
            if (this === gl && target === this.FRAMEBUFFER && attachment === this.COLOR_ATTACHMENT0 && textureTarget === this.TEXTURE_2D && candidate !== null && level === 0) {
              const binding: unknown = this.getParameter(this.FRAMEBUFFER_BINDING);
              if (binding instanceof WebGLFramebuffer) {
                texture = candidate;
                framebuffer = binding;
                Object.defineProperty(prototype, "framebufferTexture2D", { configurable: true, writable: true, value: framebufferTexture2D });
              }
            }
          }
        });
        Object.defineProperty(prototype, "deleteTexture", { configurable: true, writable: true, value(this: WebGL2RenderingContext, candidate: WebGLTexture | null): void {
          Reflect.apply(deleteTexture, this, [candidate]);
          if (this === gl && candidate !== null && candidate === texture) observed.textureDeletes++;
        } });
        Object.defineProperty(prototype, "deleteFramebuffer", { configurable: true, writable: true, value(this: WebGL2RenderingContext, candidate: WebGLFramebuffer | null): void {
          Reflect.apply(deleteFramebuffer, this, [candidate]);
          if (this === gl && candidate !== null && candidate === framebuffer) observed.framebufferDeletes++;
        } });
        Object.defineProperty(prototype, "getProgramParameter", { configurable: true, writable: true, value(this: WebGL2RenderingContext, program: WebGLProgram, parameter: number): unknown {
          const result: unknown = Reflect.apply(getProgramParameter, this, [program, parameter]);
          if (this === gl && parameter === extension.COMPLETION_STATUS_KHR) {
            observed.polls++;
            if (this.isContextLost()) lost[0]();
            if (observed.hold && result === true) { observed.heldReadyPolls++; return false; }
          }
          return result;
        } });
        return true;
      });
      report.recoveryCompile = { supported, status: supported ? "pending" : "unavailable", verified: false };
      await click("game-boot-retry");
      if (supported) {
        await page.waitForFunction(() => window.__recoveryCompile.heldReadyPolls > 0 && window.__recoveryCompile.requestId !== null);
        await page.setViewportSize({ width: 1440, height: 900 });
        await page.waitForFunction(() => window.__recoveryCompile.requests.some(request => request.kind === "cancel_resources"));
        const held = await page.evaluate(() => ({ requestId: window.__recoveryCompile.requestId,
          polls: window.__recoveryCompile.polls, heldReadyPolls: window.__recoveryCompile.heldReadyPolls,
          textureDeletes: window.__recoveryCompile.textureDeletes, framebufferDeletes: window.__recoveryCompile.framebufferDeletes,
          requests: window.__recoveryCompile.requests, inputs: window.__recoveryCompile.inputs }));
        report.recoveryCompile = { supported, status: "pending", verified: false, held };
        assert.equal(held.textureDeletes, 0, "Cancelled preparation released its texture before real compilation completed");
        assert.equal(held.framebufferDeletes, 0, "Cancelled preparation released its framebuffer before real compilation completed");
        assert.equal(held.requests.filter(request => request.kind === "cancel_resources").length, 1);
        assert.equal(held.requests.filter(request => request.kind === "commit_resources").length, 0);
        assert.equal(held.inputs.length, 0, "Held preparation reported a terminal before real compilation completed");
        await page.evaluate(() => { window.__recoveryCompile.hold = false; });
        await page.waitForFunction(() => window.__recoveryCompile.inputs.some(input => input.observation.kind === "cancelled"));
        await scene("Title");
        const completed = await page.evaluate(() => {
          const observed = window.__recoveryCompile;
          observed.restore();
          return { requestId: observed.requestId, polls: observed.polls, heldReadyPolls: observed.heldReadyPolls,
            contextLost: observed.contextLost, textureDeletes: observed.textureDeletes, framebufferDeletes: observed.framebufferDeletes,
            requests: observed.requests, inputs: observed.inputs };
        });
        report.recoveryCompile = { supported, status: "pending", verified: false, held, completed };
        assert.equal(completed.contextLost, false);
        assert.equal(completed.textureDeletes, 1);
        assert.equal(completed.framebufferDeletes, 1);
        assert.equal(completed.requests.filter(request => request.kind === "cancel_resources").length, 1);
        assert.equal(completed.requests.filter(request => request.kind === "commit_resources").length, 0);
        assert.equal(completed.inputs.length, 1);
        assert.equal(completed.inputs[0]?.observation.kind, "cancelled");
      }
      await scene("Title");
      assert.equal(await page.getByText("描画資源を準備できない。", { exact: true }).count(), 0, "Recovered Title retained the Boot resource failure notification");
      assert.equal(await page.locator(".presentation-canvas").count(), 1, "GPU retry duplicated the renderer canvas");
      const retried = await page.evaluate(() => ({ partialTextureDeletes: window.__recoveryGpu.partialTextureDeletes,
        partialFramebufferDeletes: window.__recoveryGpu.partialFramebufferDeletes,
        candidateRetryRetainedCanvas: window.__recoveryGpu.canvas === document.querySelector(".presentation-canvas") }));
      report.recoveryGpu = { ...partial, ...retried };
      assert.deepEqual(retried, { partialTextureDeletes: 1, partialFramebufferDeletes: 1, candidateRetryRetainedCanvas: true }, "GPU candidate retry changed partial cleanup or recreated the renderer canvas");
      checks.push("Startup recovery: one failed WASM request, public reload, one failed WebGL2 acquisition, one failed initial renderer observation, one failed framebuffer attachment with partial texture/framebuffer cleanup and public Boot retries");
      if (supported) {
        Object.assign(report.recoveryCompile as object, { status: "verified", verified: true });
        checks.push("Delayed real shader completion: public resize cancels the same preparation, retains its texture/framebuffer until terminal, then releases each once without commit");
        await page.setViewportSize({ width: 1280, height: 720 });
        await page.waitForFunction(() => {
          const canvas = document.querySelector<HTMLCanvasElement>(".presentation-canvas");
          return canvas !== null && !canvas.hidden && canvas.width > 0 && canvas.height > 0 && canvas.width * 9 === canvas.height * 16;
        });
      }
    }
    await scene("Title");
    phase = "normal";
    report.build = await page.locator(".build-version").innerText();
    assert.ok(String(report.build).includes(sourceHead.slice(0, 8)), "Restart Vite for the current source HEAD before verification");
    report.graphics = await page.evaluate(() => {
      const gl = document.querySelector<HTMLCanvasElement>(".presentation-canvas")?.getContext("webgl2");
      if (gl == null) throw new Error("WebGL2 renderer was not created");
      const debug = gl.getExtension("WEBGL_debug_renderer_info");
      return { renderer: String(gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER)), version: String(gl.getParameter(gl.VERSION)), userAgent: navigator.userAgent };
    });
    if (options.includes("--adapter-observe")) {
      await page.evaluate(`(async () => {
        const runtime = await import(new URL("pkg/birdman_game_wasm.js", location.href).href);
        const update = runtime.ApplicationRuntimeBridge.prototype.update;
        const probe = { active: false, axes: [0, 0, 0, 0], latest: null, rows: [] };
        window.__adapterProbe = probe;
        runtime.ApplicationRuntimeBridge.prototype.update = new Proxy(update, { apply: (target, receiver, args) => {
          const result = Reflect.apply(target, receiver, args);
          const data = JSON.parse(args[0]);
          if (probe.active && (data.input.kind === "keyboard" || data.input.kind === "frame")) {
            const output = JSON.parse(result);
            const ui = output.drawing?.ui;
            if (output.drawing != null) probe.latest = { camera: output.drawing.scene.camera,
              world_from_aircraft: output.drawing.scene.world_from_aircraft, water_time_seconds: output.drawing.scene.water_time_seconds };
            probe.rows.push({ observed_ms: data.observed_ms, input: data.input, consumed: output.keyboard_consumed,
              water_time: output.drawing?.scene.water_time_seconds ?? null,
              scene: ui?.kind === "view" ? ui.view.scene : null,
              overlay: ui?.kind === "view" ? ui.view.active_overlay ?? null : null });
          }
          return result;
        } });
        const pad = { index: 0, connected: true, mapping: "standard", axes: probe.axes,
          buttons: [{ pressed: false }, { pressed: false }] };
        Object.defineProperty(navigator, "getGamepads", { configurable: true, value: () => [pad] });
        const event = new Event("gamepadconnected");
        Object.defineProperty(event, "gamepad", { value: pad });
        window.dispatchEvent(event);
      })()`);
    }
    if (!options.includes("--adapter-observe")) await pair("title");
    const phoneVr = { disabled: await control("boot-enter-phone-vr").isDisabled(), label: await control("boot-enter-phone-vr").innerText() };
    report.phoneVr = phoneVr;
    assert.equal(phoneVr.disabled, true, "Desktop without orientation samples must keep Phone VR unavailable");
    checks.push("Title: production scene pixels and controls at HD/FHD");
    await click("game-title-start");
    await scene("FlightSetup");
    if (options.includes("--adapter-observe")) {
      if (await control("presentation-render-scale-auto").getAttribute("aria-pressed") === "true") await click("presentation-render-scale-auto");
      await click("presentation-water-quality-low");
      await click("presentation-render-scale-50");
    } else {
      await click("game-setup-select-weather-0");
      await click("game-setup-select-assistance-3");
      await pair("setup-calm");
    }
    await click("game-setup-start");
    await scene("Briefing");
    await control("game-briefing-start").waitFor({ state: "visible" });
    if (!options.includes("--adapter-observe")) await pair("briefing");
    if (options.includes("--adapter-observe")) {
      await page.clock.pauseAt(await page.evaluate(() => Date.now() + 100));
      await page.evaluate(() => { window.__adapterProbe.active = true; });
    }
    await click("game-briefing-start");
    if (options.includes("--adapter-observe")) {
      for (let frame = 0; frame < 300 && !await control("game-flight-pause").isVisible(); frame++) await page.clock.runFor(16);
    } else {
      await scene("Flight");
    }
    if (options.includes("--adapter-observe") && await control("game-flight-resume").isVisible()) {
      await control("game-flight-resume").waitFor({ state: "visible" });
      await click("game-flight-resume");
    }
    try {
      await control("game-flight-pause").waitFor({ state: "visible" });
    } catch (cause) {
      if (options.includes("--adapter-observe")) {
        const state = await page.evaluate(() => ({ text: document.body.innerText, rows: window.__adapterProbe.rows.slice(-40) }));
        await writeFile(resolve(output, "adapter-failure.json"), JSON.stringify(state, null, 2) + "\n", "utf8");
      }
      throw cause;
    }
    if (options.includes("--adapter-observe")) {
      await page.clock.runFor(48);
      await page.evaluate(() => { window.__adapterProbe.axes[0] = 0.25; window.__adapterProbe.axes[1] = -0.25; });
      await page.clock.runFor(112);
      await page.evaluate(() => { window.__adapterProbe.axes[0] = 0; window.__adapterProbe.axes[1] = 0; });
      await page.clock.runFor(111);
      await page.keyboard.down("ArrowUp");
      await page.clock.runFor(112);
      await page.keyboard.up("ArrowUp");
      await page.clock.runFor(112);
      await page.keyboard.down("KeyP");
      await page.clock.runFor(16);
      await page.keyboard.up("KeyP");
      await page.clock.runFor(32);
      await page.screenshot({ path: resolve(output, "adapter-pause-hd.png") });
      const pausedDrawing = await page.evaluate(() => window.__adapterProbe.latest);
      await writeFile(resolve(output, "adapter-pause-drawing.json"), JSON.stringify(pausedDrawing, null, 2) + "\n", "utf8");
      await page.keyboard.down("KeyP");
      await page.clock.runFor(16);
      await page.keyboard.up("KeyP");
      await page.clock.runFor(32);
      await page.evaluate(() => { window.__adapterProbe.active = false; });
      const observations = await page.evaluate(() => window.__adapterProbe.rows);
      await writeFile(resolve(output, "adapter-observations.json"), JSON.stringify(observations, null, 2) + "\n", "utf8");
      assert.ok(observations.some(row => row.input.kind === "keyboard" && row.input.code === "ArrowUp" && row.input.pressed && row.consumed));
      assert.ok(observations.some(row => row.input.kind === "frame" && row.input.gamepads.some(pad => pad.axes[0] === 0.25 && pad.axes[1] === -0.25)));
      await page.clock.resume();
      await click("game-flight-abort");
      await scene("Result");
      await click("game-result-technical");
      const [flightRecord] = await Promise.all([page.waitForEvent("download"), click("game-flight-log-json")]);
      await flightRecord.saveAs(resolve(output, "adapter-record.json"));
      report.execution = "Production Web input adapters with synthetic gamepad and Playwright Clock";
      checks.push("Keyboard and gamepad observations through the production browser adapters");
      return;
    }
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
      if (chart === "map") {
        const previousFont = await page.evaluate(() => {
          const previous = document.documentElement.style.fontSize;
          document.documentElement.style.fontSize = "150%";
          return previous;
        });
        await chartSettled();
        await snapshot(page, "analysis-map-font150", false);
        await page.setViewportSize({ width: 1379, height: 821 });
        await chartSettled();
        await snapshot(page, "analysis-map-resized-font150", false);
        await click("game-analysis-altitude");
        await click("game-analysis-map");
        await page.evaluate(previous => { document.documentElement.style.fontSize = previous; }, previousFont);
        await page.setViewportSize({ width: 1920, height: 1080 });
        await chartSettled();
      }
      if (chart !== "map") {
        const cursor = control("game-analysis-cursor");
        await cursor.press("Home");
        const before = await cursor.inputValue();
        const viewport = page.locator(".screen-ui-chart-viewport");
        await viewport.scrollIntoViewIfNeeded();
        const bounds = await viewport.boundingBox();
        assert.ok(bounds !== null, `${chart}: chart viewport is unavailable`);
        await page.mouse.move(bounds.x + bounds.width * 0.35, bounds.y + bounds.height * 0.5);
        await page.mouse.down();
        await page.mouse.move(bounds.x + bounds.width * 0.7, bounds.y + bounds.height * 0.5, { steps: 4 });
        await page.mouse.up();
        await page.waitForFunction(previous => {
          const range = document.querySelector<HTMLInputElement>('[data-control-id="game-analysis-cursor"]');
          return range != null && range.value !== previous;
        }, before);
        chartInteractions.push({ chart, before, after: await cursor.inputValue() });
        if (chart === "speed") {
          await page.locator("#game-analysis-cursor-values").scrollIntoViewIfNeeded();
          await snapshot(page, "analysis-speed-fields", false);
        }
      }
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
    if (recovery) {
      const saved = page.locator('[data-control-id^="game-title-open-record-"]').first();
      await saved.waitFor();
      const savedControlId = await saved.getAttribute("data-control-id");
      assert.ok(savedControlId !== null);
      phase = "record-read-failure";
      expectedDiagnostics.push({ phase, source: "console", text: "Error: IndexedDB transaction aborted", observed: [] });
      await page.evaluate(() => {
        const original = Object.getOwnPropertyDescriptor(IDBObjectStore.prototype, "get")?.value as IDBObjectStore["get"] | undefined;
        if (original === undefined) throw new Error("IndexedDB record lookup is unavailable");
        Object.defineProperty(IDBObjectStore.prototype, "get", { configurable: true, writable: true,
          value(this: IDBObjectStore, query: IDBValidKey | IDBKeyRange): IDBRequest {
            const request = Reflect.apply(original, this, [query]);
            const transaction = this.transaction;
            if (transaction.db.name === "birdman-flight-records" && this.name === "records"
                && transaction.mode === "readonly" && transaction.objectStoreNames.length === 1) {
              Object.defineProperty(IDBObjectStore.prototype, "get", { configurable: true, writable: true, value: original });
              const observed = { requestSucceeded: false, aborts: 0, error: null as string | null };
              window.__recoveryRead = observed;
              transaction.addEventListener("abort", () => { observed.aborts++; observed.error = transaction.error?.message ?? null; }, { once: true });
              request.addEventListener("success", () => { observed.requestSucceeded = true; transaction.abort(); }, { once: true });
            }
            return request;
          }
        });
      });
      await click(savedControlId);
      await page.getByText("飛行記録を開けない。記録を選び直して再試行できる。", { exact: true }).waitFor();
      await scene("Title");
      await snapshot(page, "recovery-record-read-failed");
      const readAbort = await page.evaluate(() => window.__recoveryRead);
      assert.deepEqual(readAbort, { requestSucceeded: true, aborts: 1, error: null });
      report.recordReadAbort = { controlId: savedControlId, ...readAbort };
      phase = "record-read-retry";
      await click(savedControlId);
      await scene("Replay");
      await snapshot(page, "recovery-stored-replay");
      await click("game-replay-technical");
      assert.deepEqual(await download("json", "recovered-record"), original, "Stored read recovery changed the original JSON");
      assert.deepEqual(await download("csv", "recovered-record"), csv, "Stored read recovery changed the CSV");
      await click("game-replay-view-mode");
      await click("game-replay-view-mode");
      await control("game-analysis-map").waitFor();
      await chartSettled();
      await snapshot(page, "recovery-stored-analysis", false);
      await click("game-replay-return");
      await scene("Title");
      checks.push("Stored read recovery: successful real request followed by one readonly transaction abort, Japanese guidance, same Title control retry, Replay/Analysis and byte-identical JSON/CSV");
    }
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
  const successfulWasm = wasm.filter(response => response.status === 200);
  if (responses === null || responses.some(response => response.status === "rejected") || successfulWasm.length === 0
      || successfulWasm.some(response => response.sha256 !== wasmSha256)
      || wasm.some(response => response.status !== 200 && (!recovery || response.url !== wasmFailure.url || response.status !== wasmFailure.status || response.sha256 !== wasmFailure.sha256))
      || (recovery && wasm.filter(response => response.status === 503).length !== 1)) errors.push("Served WASM response/hash does not match the captured local artifact or the single injected failure");
  try { await browser?.close(); } catch (cause) { errors.push(`Browser cleanup: ${String(cause)}`); }
  try {
    assert.equal(git(["rev-parse", "HEAD"]).trim(), sourceHead, "HEAD changed during verification");
    assert.equal(git(diffArguments), sourcePatch, "Source changed during verification");
    assert.equal(git(untrackedArguments), "", "New source appeared during verification");
    assert.equal(hash(await readFile(wasmPath)), wasmSha256, "WASM changed during verification");
  } catch (cause) { errors.push(String(cause)); }
  for (const diagnostic of diagnostics) {
    const expected = expectedDiagnostics.find(value => value.phase === diagnostic.phase && value.source === diagnostic.source
      && value.text === diagnostic.text.split("\n")[0] && (value.url === undefined || value.url === diagnostic.url));
    if (expected === undefined) errors.push(`${diagnostic.phase}/${diagnostic.source} ${diagnostic.url}: ${diagnostic.text}`);
    else expected.observed.push(diagnostic);
  }
  for (const expected of expectedDiagnostics) if (expected.observed.length !== 1) errors.push(`Expected exactly one ${expected.phase}/${expected.source} diagnostic: ${expected.text}; observed ${String(expected.observed.length)}`);
  if (errors.length > 0) report.status = "failed";
  await writeFile(resolve(output, "result.json"), `${JSON.stringify(report, null, 2)}\n`, "utf8");
}
if (report.status !== "passed") process.exitCode = 1;
console.log(JSON.stringify({ status: report.status, sourceHead, output, checks, errors }, null, 2));
