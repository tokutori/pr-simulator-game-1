import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { transform } from "esbuild";
import { chromium } from "playwright-core";
import { PerspectiveCamera, StereoCamera } from "three";
import type { Browser } from "playwright-core";
import type { GameUiView } from "../../web/src/generated/game-ui-view.js";
import { captureConfiguredViewerFrame } from "../../web/src/render/engines/three/viewer-frame.js";

declare global {
  interface Window {
    __uiPauseEvents: readonly { at: number; reason: number }[];
    __uiViewJson: string | null;
  }
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
  const measureHud = async (forwardClear: boolean) => {
    const measured = await page.evaluate(clear => {
      const root = document.querySelector<HTMLElement>('.flight-hud[aria-hidden="false"]');
      const menu = document.querySelector<HTMLElement>(".screen-ui-shell");
      if (root === null || menu === null) throw new Error("The live HUD/menu is unavailable");
      const geometry = {
        rect(node: Element) {
          const bounds = node.getBoundingClientRect();
          return { left: bounds.left, top: bounds.top, right: bounds.right, bottom: bounds.bottom, width: bounds.width, height: bounds.height };
        },
        overlap(first: Pick<DOMRect, "left" | "right" | "top" | "bottom">, second: Pick<DOMRect, "left" | "right" | "top" | "bottom">) {
          return first.left < second.right - 1 && second.left < first.right - 1 && first.top < second.bottom - 1 && second.top < first.bottom - 1;
        }
      };
      const issues: string[] = [];
      const rootBounds = geometry.rect(root);
      const style = getComputedStyle(root);
      const menuBounds = geometry.rect(menu);
      if (rootBounds.left < -1 || rootBounds.right > innerWidth + 1 || rootBounds.top < -1 || rootBounds.bottom > innerHeight + 1) issues.push("HUD outside viewport");
      if (clear && rootBounds.top < innerHeight * 0.5) issues.push("HUD enters the forward upper half");
      if (style.backgroundColor !== "rgba(0, 0, 0, 0)" || style.overflowX !== "visible" || style.overflowY !== "visible") issues.push("HUD background/scroll");
      const nodes = [...root.querySelectorAll<HTMLElement>("[data-node]")].map(node => {
        const bounds = geometry.rect(node);
        const nodeStyle = getComputedStyle(node);
        const id = node.dataset.node ?? "unknown";
        if (node.scrollWidth > node.clientWidth + 1 || node.scrollHeight > node.clientHeight + 1) issues.push(`${id}: content overflow`);
        if (geometry.overlap(bounds, menuBounds)) issues.push(`${id}: overlaps the menu`);
        const range = document.createRange();
        const header = node.querySelector(".flight-hud-header");
        if (header === null) throw new Error("Missing HUD header");
        range.selectNodeContents(header);
        const textBounds = range.getBoundingClientRect();
        const fontSize = Number.parseFloat(nodeStyle.fontSize);
        if (fontSize < 14 || textBounds.left < bounds.left - 1 || textBounds.right > bounds.right + 1 || textBounds.bottom > bounds.bottom + 1) issues.push(`${id}: text size/bounds`);
        return { id, bounds, fontSize, contentWidth: node.scrollWidth, contentHeight: node.scrollHeight };
      });
      for (let index = 0; index < nodes.length; index++) for (const other of nodes.slice(0, index)) {
        const node = nodes[index];
        if (node !== undefined && geometry.overlap(node.bounds, other.bounds)) issues.push(`${node.id}: overlaps ${other.id}`);
      }
      const glyphs = [...root.querySelectorAll<SVGTextElement>("svg text")].filter(node => node.textContent !== "").map(node => {
        const matrix = node.getScreenCTM();
        const pixels = Number.parseFloat(getComputedStyle(node).fontSize) * (matrix === null ? 0 : Math.hypot(matrix.a, matrix.b));
        if (pixels < 13.9) issues.push(`Instrument label below 14px: ${node.textContent}`);
        return { text: node.textContent, fontSize: pixels };
      });
      const opaqueAdi = [...root.querySelectorAll<SVGElement>('[data-instrument="attitude"] rect, [data-instrument="attitude"] path')]
        .filter(node => node.closest("defs") === null && ![null, "none"].includes(node.getAttribute("fill")));
      if (opaqueAdi.length > 0) issues.push("ADI has opaque area fills");
      return { viewport: { width: innerWidth, height: innerHeight, rootFontSize: getComputedStyle(document.documentElement).fontSize },
        bounds: rootBounds, nodes, glyphs, issues };
    }, forwardClear);
    if (measured.issues.length > 0) report.hudFailure = measured;
    assert.deepEqual(measured.issues, []);
    return measured;
  };
  const noEmptyLabelPrefix = async () => {
    const values = await page.locator(".screen-ui-status, .flight-hud output").allTextContents();
    assert.deepEqual(values.filter(value => /^\s*[:：]/u.test(value)), [], "An empty label must not add a leading colon");
  };
  const creditsScroll = async (phase: "title" | "flight_paused", image: string) => {
    await page.setViewportSize({ width: 360, height: 240 });
    const shell = page.locator(".screen-ui-shell");
    const original = await shell.elementHandle();
    assert.ok(original !== null);
    try {
      const before = await shell.evaluate(node => { node.scrollTop = 120; return node.scrollTop; });
      assert.ok(before > 0, "Credits entry must start from a genuinely scrolled menu");
      await control("game-credits-open").dispatchEvent("click");
      await control("game-credits-close").waitFor({ state: "visible" });
      assert.equal(await shell.getAttribute("data-overlay"), "Credits");
      assert.equal(await shell.evaluate(node => node.scrollTop), 0);
      await noEmptyLabelPrefix();
      await screenshot(image);
      const retained = await shell.evaluate(async node => {
        node.scrollTop = 120;
        const before = node.scrollTop;
        await new Promise<void>(complete => { requestAnimationFrame(() => { requestAnimationFrame(() => { complete(); }); }); });
        return { before, after: node.scrollTop };
      });
      assert.ok(retained.before > 0, "Credits must contain scrollable content");
      assert.equal(retained.after, retained.before, "The same Credits scope must preserve scroll across frames");
      await control("game-credits-close").dispatchEvent("click");
      await control("game-credits-open").waitFor({ state: "visible" });
      assert.equal(await shell.evaluate(node => node.scrollTop), 0);
      assert.equal(await original.evaluate(node => node === document.querySelector(".screen-ui-shell")), true);
      assert.equal(await page.evaluate(() => window.__liveAuto.phase), phase);
      return { phase, before, retained };
    } finally { await original.dispose(); }
  };
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
      const viewDescriptor = Object.getOwnPropertyDescriptor(prototype, "game_ui_view_json");
      if (viewDescriptor === undefined || typeof viewDescriptor.value !== "function") throw new Error("Production shared UI projection is unavailable");
      const readView = viewDescriptor.value as (...args: unknown[]) => unknown;
      window.__uiViewJson = null;
      Object.defineProperty(prototype, "game_ui_view_json", { ...viewDescriptor, value(this: unknown, ...args: unknown[]): unknown {
        const returned: unknown = Reflect.apply(readView, this, args);
        if (typeof returned === "string") window.__uiViewJson = returned;
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
    report.creditsScroll = [await creditsScroll("title", "00-credits")];
    await page.setViewportSize({ width: 1280, height: 720 });
    await click("game-credits-open");
    await page.getByRole("heading", { name: "クレジット・利用条件", exact: true }).waitFor({ state: "visible" });
    for (const source of ["osm", "jaxa", "copernicus", "jma", "font", "earcut", "ryu", "time"]) {
      assert.ok(await page.locator(`#game-credits-${source}`).count() > 0, `Missing credit: ${source}`);
    }
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
    await noEmptyLabelPrefix();
    report.vrChoiceLayout = await page.evaluate(async addresses => {
      if (window.__uiViewJson === null) throw new Error("The production shared Setup View was not captured");
      const shared = JSON.parse(window.__uiViewJson) as GameUiView;
      if (shared.scene !== "FlightSetup") throw new Error("VR text projection requires the current shared Setup View");
      const viewModule = await import(addresses.view) as typeof import("../../web/src/app/game-view.js");
      const layoutModule = await import(addresses.layout) as typeof import("../../web/src/presentation/menu-layout.js");
      const canvasModule = await import(addresses.canvas) as typeof import("../../web/src/presentation/menu-canvas.js");
      const panel = viewModule.createGameViewModel(shared).panels[0];
      if (panel === undefined) throw new Error("Missing shared Setup panel");
      const request = layoutModule.requestMenuLayout(panel, panel.size, { theme: shared.theme, fontMeters: 0.04,
        font: { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 },
        locale: "ja", previousLabel: "前頁", nextLabel: "次頁", caption: shared.description });
      const size = canvasModule.menuCanvasSize(panel.size);
      const canvas = document.createElement("canvas");
      canvas.width = size.width; canvas.height = size.height;
      const context = canvas.getContext("2d");
      if (context === null) throw new Error("Browser Canvas measurement is unavailable");
      const measured = canvasModule.measureMenuLayout(canvasModule.browserMenuContext(context), request, size.width, size.height);
      if (measured.kind !== "ready") throw new Error(measured.reason);
      const layout = layoutModule.layoutMeasuredMenu(request, measured.measurements);
      if (layout.kind !== "ready") throw new Error(layout.reason);
      const choices = layout.document.controls.filter(node => node.control.kind === "button"
        && node.control.presentation?.kind === "choice" && node.control.presentation.description.length === 0);
      if (choices.length === 0) throw new Error("Missing empty-description choices");
      for (const choice of choices) {
        const label = choice.texts.find(text => text.request.slot.kind === "control" && text.request.slot.role === "label");
        if (label === undefined || label.request.value.endsWith("\n") || label.lines.some(line => line.value.length === 0)) {
          throw new Error("An empty choice description added a VR label line");
        }
        const expected = label.lines.length * request.lineHeightMeters + request.paddingMeters * 2;
        if (Math.abs(choice.bounds.height - expected) > 1e-10) throw new Error("An empty choice description added VR height");
      }
      return { emptyDescriptions: choices.length, measuredIn: "production VR Menu layout and browser Canvas" };
    }, { view: new URL("../src/app/game-view.ts", moduleUrl).href,
      layout: new URL("../src/presentation/menu-layout.ts", moduleUrl).href, canvas: new URL("../src/presentation/menu-canvas.ts", moduleUrl).href });
    report.portraitViewport = { width: 360, height: 640, selectedDescriptions };
    await screenshot("01-setup-portrait");
    await page.setViewportSize({ width: 1280, height: 720 });
    await screenshot("01-setup");
    await click("game-setup-start");
    await scene("Briefing");
    const briefingConditions = await page.locator("#game-briefing-conditions").innerText();
    const briefingWeather = await page.locator("#game-weather-conditions").innerText();
    await noEmptyLabelPrefix();
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
    await page.waitForFunction(() => window.__liveAuto.phase === "flight_paused" || window.__liveAuto.phase === "flight_running" && (window.__liveAuto.tick ?? 0) >= 10);
    if (await page.evaluate(() => window.__liveAuto.phase) === "flight_paused") {
      report.startupPause = await page.evaluate(() => ({ tick: window.__liveAuto.tick,
        snapshotJson: window.__liveAuto.snapshotJson, pauseEvents: window.__uiPauseEvents,
        text: document.querySelector(".screen-ui-shell")?.textContent }));
      findings.push("Flight suspended before the initial Running observation; continued once through the public Resume action");
      await click("game-flight-resume");
    }
    await page.waitForFunction(() => window.__liveAuto.phase === "flight_running" && (window.__liveAuto.tick ?? 0) >= 10);
    const hud = page.locator('.flight-hud[aria-hidden="false"]');
    await hud.waitFor({ state: "visible" });
    const kinds = await hud.locator("[data-kind]").evaluateAll(nodes => [...new Set(nodes.map(node => node.getAttribute("data-kind")))]);
    assert.deepEqual([...kinds].sort(), ["attitude", "direction", "heading_tape", "linear_gauge", "text"]);
    assert.ok(await hud.locator("output").count() > 0);
    assert.equal(await hud.locator("a, details").count(), 0);
    assert.doesNotMatch(await hud.innerText(), /緯度|経度|実機構成は未確認|incidence|intent|Hold|PILOT TARGET|COPERNICUS/);
    await noEmptyLabelPrefix();
    report.hudKinds = kinds;
    report.runningHud = await measureHud(true);
    await click("game-flight-pause");
    await page.waitForFunction(() => window.__liveAuto.phase === "flight_paused");
    await screenshot("03-paused-hud");
    const hudMeasurements = [];
    for (const [width, height, font] of [[1280, 720, 16], [1920, 1080, 16], [1600, 900, 20], [1920, 1080, 24]] as const) {
      await page.setViewportSize({ width, height });
      await page.evaluate(size => { document.documentElement.style.fontSize = `${String(size)}px`; }, font);
      await page.evaluate(() => new Promise<void>(complete => { requestAnimationFrame(() => { requestAnimationFrame(() => { complete(); }); }); }));
      hudMeasurements.push(await measureHud(false));
      await screenshot(`03-hud-${String(width)}x${String(height)}-font${String(font)}`);
    }
    report.hudMeasurements = hudMeasurements;
    await page.evaluate(() => { document.documentElement.style.removeProperty("font-size"); });
    await page.setViewportSize({ width: 1280, height: 720 });
    for (const [open, back, name] of [["game-pause-open-settings", "game-pause-settings-back", "settings"], ["game-pause-open-help", "game-pause-help-back", "help"]] as const) {
      await click(open);
      await control(back).waitFor({ state: "visible" });
      await measureHud(false);
      await screenshot(`03-hud-pause-${name}`);
      await click(back);
    }
    const headScenarios = [
      { name: "landscape", aspect: 1280 / 720, maximumWidthMeters: 2.16 },
      { name: "portrait", aspect: 720 / 1280, maximumWidthMeters: 2.16 },
      { name: "narrow-fallback", aspect: 1280 / 720, maximumWidthMeters: 0.85 }
    ].map(scenario => {
      const camera = new PerspectiveCamera(60, scenario.aspect, 0.05, 100);
      camera.updateMatrixWorld(true);
      const stereo = new StereoCamera();
      stereo.aspect = 0.5;
      return { ...scenario, viewer: captureConfiguredViewerFrame(camera, stereo) };
    });
    const profiler = await context.newCDPSession(page);
    try {
      await profiler.send("Profiler.enable");
      await profiler.send("Profiler.startPreciseCoverage", { callCount: true, detailed: false });
      const headResults = [];
      for (const scenario of headScenarios) {
      const headMeasurement = await page.evaluate(async input => {
        if (window.__uiViewJson === null) throw new Error("Missing shared HUD View");
        const shared = JSON.parse(window.__uiViewJson) as GameUiView;
        if (shared.hud === null) throw new Error("Missing shared HUD");
        const modelModule = await import(input.model) as typeof import("../../web/src/presentation/flight-hud-model.js");
        const viewModule = await import(input.view) as typeof import("../../web/src/presentation/head-hud-view.js");
        const canvasModule = await import(input.canvas) as typeof import("../../web/src/presentation/head-hud-canvas.js");
        const model = modelModule.projectSharedFlightHudModel(shared.hud, shared.theme);
        const measurements = [];
        let image: string | null = null;
        for (const scenario of input.scenarios) {
          const elapsed = [];
          let last: ReturnType<typeof viewModule.createHeadHudView> = { kind: "absent" };
          for (let iteration = 0; iteration < 30; iteration++) {
            const start = performance.now();
            last = viewModule.createHeadHudView(model, scenario.viewer, "ja", { ...viewModule.DEFAULT_HEAD_HUD_PROFILE, maximumWidthMeters: scenario.maximumWidthMeters });
            elapsed.push(performance.now() - start);
          }
          elapsed.sort((first, second) => first - second);
          if (scenario.name !== "narrow-fallback" && last.kind !== "visible") throw new Error(`Head HUD ${scenario.name}: ${last.kind}`);
          if (last.kind === "visible") {
            const size = canvasModule.headHudCanvasSize(last.layer);
            const canvas = document.createElement("canvas");
            canvas.width = size.width; canvas.height = size.height;
            const drawing = canvas.getContext("2d");
            if (drawing === null) throw new Error("Missing Head Canvas");
            const drawingContext = canvasModule.browserHeadHudContext(drawing);
            const prepared = canvasModule.prepareHeadHudPaint(drawingContext, last, size.width, size.height);
            const validated = canvasModule.validateHeadHudPaint(prepared, scenario.viewer);
            if (validated.kind !== "ready") throw new Error(`Head Canvas ${scenario.name}: ${validated.kind}`);
            canvasModule.drawHeadHud(drawingContext, validated);
            if (scenario.name === "landscape") image = canvas.toDataURL("image/png");
          }
          measurements.push({ name: scenario.name, outcome: last.kind, samples: elapsed.length, medianMilliseconds: elapsed[15], p95Milliseconds: elapsed[28] });
        }
        return { measurements, image, hudBytes: new TextEncoder().encode(JSON.stringify({ layout: model.layout, groups: model.groups })).length };
      }, { scenarios: [scenario], model: new URL("../src/presentation/flight-hud-model.ts", moduleUrl).href,
        view: new URL("../src/presentation/head-hud-view.ts", moduleUrl).href, canvas: new URL("../src/presentation/head-hud-canvas.ts", moduleUrl).href });
      const coverage = await profiler.send("Profiler.takePreciseCoverage");
      const calls = coverage.result.flatMap(script => script.functions).filter(item => item.functionName === "game_ui_head_hud_fit_json")
        .reduce((sum, item) => sum + (item.ranges[0]?.count ?? 0), 0);
      assert.ok(calls > 0, "The production shared Head fit was not profiled");
      headResults.push({ ...headMeasurement.measurements[0], hudBytes: headMeasurement.hudBytes, wasmCallsPerFrame: calls / 30,
        maximumInputBytesPerFrame: calls / 30 * (headMeasurement.hudBytes + 1024) });
      if (scenario.name === "landscape") {
        if (headMeasurement.image === null) throw new Error("Missing Head HUD image");
        await writeFile(resolve(output, "03-head-hud.png"), Buffer.from(headMeasurement.image.slice("data:image/png;base64,".length), "base64"));
      }
      }
      report.headHud = { measurements: headResults, measurement: "30 calls per scenario, CDP coverage enabled; input bound includes the 1024-byte space limit; not a frame-rate acceptance" };
      await profiler.send("Profiler.stopPreciseCoverage");
    } finally { await profiler.detach(); }
    checks.push("HD/FHD・文字拡大の下端HUDは全値・14px文字高・非重複を保持し、実CanvasのHead HUDは両眼と前方中央へ収まる");
    const pausedTick = await page.evaluate(() => window.__liveAuto.tick);
    report.pausedCreditsScroll = await creditsScroll("flight_paused", "03-paused-credits");
    await page.setViewportSize({ width: 1280, height: 720 });
    assert.equal(await page.evaluate(() => window.__liveAuto.phase), "flight_paused");
    assert.equal(await page.evaluate(() => window.__liveAuto.tick), pausedTick);
    await click("game-flight-resume");
    await page.waitForFunction(() => window.__liveAuto.phase === "flight_running");
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
    report.replayHud = await measureHud(false);
    await screenshot("05-replay-hud");
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
    checks.push("空labelの先頭コロンを除き、Credits往復はscrollを初期化し、同scopeと空説明のVR行高を保持する");
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
