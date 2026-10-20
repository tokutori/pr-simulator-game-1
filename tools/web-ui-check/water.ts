import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { Page } from "playwright-core";
import type { Group, Scene, ShaderMaterial, WebGLRenderer } from "three";
import type { ScreenRenderAutoSnapshot } from "../../web/src/generated/screen-render-auto.js";

declare global {
  interface Window {
    __waterCheck: { renderer: WebGLRenderer; scene: Scene; water: Group; waterMaterial: ShaderMaterial; threeModuleUrl: string; auto?: ScreenRenderAutoSnapshot; suspendLoop: boolean;
      preparation: { stage: string; milliseconds: number }[] };
  }
}

export interface WaterShaderReference {
  readonly sourceHead: string;
  readonly vertexShader: string;
  readonly fragmentShader: string;
}

export async function observeWaterRenderer(page: Page): Promise<void> {
  await page.route("**/src/render/engines/three/three-renderer.ts*", async route => {
    const response = await route.fetch();
    const source = await response.text();
    const anchor = "scene.add(water);";
    assert.equal(source.split(anchor).length, 2, "Missing unique water renderer observation point");
    const threeModuleUrl = /from\s*["']([^"']*\/three\.js(?:\?[^"']*)?)["']/.exec(source)?.[1];
    assert.ok(threeModuleUrl !== undefined, "Missing Vite Three.js module URL");
    const snapshotAnchor = "screenAutoBinding.onSnapshot(snapshot);";
    assert.equal(source.split(snapshotAnchor).length, 2, "Missing unique Auto observation point");
    const generationStages = [
      ["createProductionLakeDetailLayer(condition, false)", "detail-near-generation"],
      ["createProductionLakeDetailLayer(condition, true)", "detail-far-generation"],
      ["createBirdmanAirframe()", "airframe-generation"]
    ];
    let observed = source;
    for (const [expression, stage] of generationStages) {
      assert.ok(expression !== undefined && stage !== undefined);
      assert.equal(observed.split(expression).length, 2, `Missing unique ${stage} observation point`);
      observed = observed.replace(expression, `observePreparation(${JSON.stringify(stage)}, () => ${expression})`);
    }
    await route.fulfill({ response, body: `const preparationTimings = [];
      function observePreparation(stage, run) {
        const started = performance.now();
        try { return run(); } finally { preparationTimings.push({ stage, milliseconds: performance.now() - started }); }
      }\n` + observed.replace(anchor,
      `${anchor}\nwindow.__waterCheck = { renderer, scene, water, waterMaterial, threeModuleUrl: ${JSON.stringify(threeModuleUrl)}, suspendLoop: false, preparation: preparationTimings };
      const observedSetAnimationLoop = renderer.setAnimationLoop.bind(renderer);
      renderer.setAnimationLoop = callback => observedSetAnimationLoop(callback === null ? null : (...args) => {
        if (!window.__waterCheck.suspendLoop) callback(...args);
      });
      const observedCompileAsync = renderer.compileAsync.bind(renderer);
      renderer.compileAsync = (...args) => {
        const started = performance.now();
        return observedCompileAsync(...args).finally(() => preparationTimings.push({ stage: 'material-compile-wall', milliseconds: performance.now() - started }));
      };
      const observedInitTexture = renderer.initTexture.bind(renderer);
      renderer.initTexture = texture => observePreparation('texture-upload-submit', () => observedInitTexture(texture));
      const observedRender = renderer.render.bind(renderer);
      const submittedPasses = new Set();
      renderer.render = (...args) => {
        const stage = renderer.getRenderTarget() === null ? 'first-main-submit' : 'first-target-submit';
        if (submittedPasses.has(stage)) return observedRender(...args);
        submittedPasses.add(stage);
        return observePreparation(stage, () => observedRender(...args));
      };`)
      .replace(snapshotAnchor, `${snapshotAnchor}\nwindow.__waterCheck.auto = snapshot;`) });
  });
}

export async function verifyWaterRenderer(page: Page, geometryUrl: string, output: string, quality: "low" | "medium" | "high", reference?: WaterShaderReference): Promise<unknown> {
  const result = await page.evaluate(async input => {
    const geometryModule = await import(input.geometry) as typeof import("../../web/src/render/engines/three/lake-water-geometry.js");
    const { renderer, scene, water, waterMaterial, threeModuleUrl } = window.__waterCheck;
    const three = await import(threeModuleUrl) as typeof import("three");
    const { GpuWorkTimer } = await import(new URL("gpu-work-timer.ts", input.geometry).href) as typeof import("../../web/src/render/engines/three/gpu-work-timer.js");
    const chunks = water.children.filter((node): node is import("three").Mesh<import("three").BufferGeometry, ShaderMaterial> => "isMesh" in node && node.isMesh === true);
    const segments = { low: 96, medium: 144, high: 224 }[input.quality];
    if (chunks.length !== 16 || chunks.some(chunk => chunk.geometry.getAttribute("position").count !== (segments / 4 + 1) ** 2)) {
      throw new Error(`Water ${input.quality}: missing expected chunk geometry`);
    }
    const first = chunks[0];
    if (first === undefined) throw new Error("Missing water chunk");
    const context = renderer.getContext();
    if (!(context instanceof WebGL2RenderingContext)) throw new Error("Water verification requires WebGL2");
    const timer = new GpuWorkTimer(context);
    const timerAvailability = timer.availability();
    const farWater = scene.getObjectByName("lake-biwa-distant-water-backing");
    if (!(farWater instanceof three.Mesh) || !(farWater.material instanceof three.ShaderMaterial)) throw new Error("Missing far water material");
    const farMaterial = farWater.material;
    const production = { vertexShader: waterMaterial.vertexShader, fragmentShader: waterMaterial.fragmentShader, farFragmentShader: farMaterial.fragmentShader };
    const timeUniform = waterMaterial.uniforms.uTimeSeconds;
    if (timeUniform === undefined || typeof timeUniform.value !== "number") throw new Error("Missing water time");
    const previousTime = timeUniform.value;
    let referenceActive = false;
    const shaders = { select(useReference: boolean) {
      if (useReference === referenceActive) return;
      if (useReference) {
        if (input.reference === undefined) throw new Error("Missing reference shader");
        waterMaterial.vertexShader = input.reference.vertexShader;
        waterMaterial.fragmentShader = input.reference.fragmentShader;
        farMaterial.fragmentShader = input.reference.fragmentShader;
      } else {
        waterMaterial.vertexShader = production.vertexShader;
        waterMaterial.fragmentShader = production.fragmentShader;
        farMaterial.fragmentShader = production.farFragmentShader;
      }
      waterMaterial.needsUpdate = true;
      farMaterial.needsUpdate = true;
      referenceActive = useReference;
    } };
    if (context.getError() !== context.NO_ERROR || context.isContextLost()) throw new Error("WebGL is unhealthy before water verification");
    const width = 640;
    const height = 360;
    const previousTarget = renderer.getRenderTarget();
    const previousViewport = renderer.getViewport(new three.Vector4());
    const previousScissor = renderer.getScissor(new three.Vector4());
    const previousScissorTest = renderer.getScissorTest();
    const previousShadowUpdate = renderer.shadowMap.autoUpdate;
    const previousLoopSuspension = window.__waterCheck.suspendLoop;
    const retained = chunks.map(chunk => ({ chunk, visible: chunk.visible,
      before: Object.getOwnPropertyDescriptor(chunk, "onBeforeRender") }));
    const target = new three.WebGLRenderTarget(width, height);
    const original = geometryModule.createLakeGeometry(segments);
    const monolith = first.clone();
    monolith.geometry = original;
    monolith.frustumCulled = false;
    monolith.visible = false;
    water.add(monolith);
    const draws: string[] = [];
    const images: { name: string; data: string }[] = [];
    const measurements: { camera: string; chunks: number; submittedVertices: number; drawCalls: number; maximumChannelError: number; changedPixels: number;
      chunkSubmitMs: number; chunkReadbackMs: number; monolithSubmitMs: number; monolithReadbackMs: number }[] = [];
    const errors: number[] = [];
    const shaderMeasurements: { camera: string; timeSeconds: number; maximumChannelError: number; changedPixels: number }[] = [];
    const gpuSamples: { shader: "reference" | "production"; milliseconds: number }[] = [];
    const gpuTimingFailures: string[] = [];
    let submitMs = 0;
    let readbackMs = 0;
    const probe = {
    compare(actual: Uint8Array, expected: Uint8Array): { maximumChannelError: number; changedPixels: number } {
      let maximumChannelError = 0;
      let changedPixels = 0;
      for (let pixel = 0; pixel < width * height; pixel++) {
        let changed = false;
        for (let channel = 0; channel < 4; channel++) {
          const delta = Math.abs((actual[pixel * 4 + channel] ?? 0) - (expected[pixel * 4 + channel] ?? 0));
          maximumChannelError = Math.max(maximumChannelError, delta);
          changed ||= delta > 0;
        }
        if (changed) changedPixels++;
      }
      return { maximumChannelError, changedPixels };
    },
    async gpu(camera: import("three").Camera, shader: "reference" | "production"): Promise<void> {
      renderer.setRenderTarget(target);
      const source = { epoch: 1, frame: gpuSamples.length, source: 1 };
      const observation = timer.begin(source);
      if (observation !== null) { gpuTimingFailures.push(observation.kind); return; }
      let complete = false;
      try { renderer.render(scene, camera); complete = true; } finally { timer.end(complete); }
      const deadline = performance.now() + 3000;
      while (performance.now() < deadline) {
        const observations = timer.poll();
        for (const result of observations) {
          if (result.kind === "sample") { gpuSamples.push({ shader, milliseconds: result.milliseconds }); return; }
          gpuTimingFailures.push(result.kind);
          return;
        }
        await new Promise<void>(complete => { setTimeout(complete, 10); });
      }
      timer.clear();
      gpuTimingFailures.push("timeout-or-query-busy");
    },
    render(camera: import("three").Camera): Uint8Array {
      draws.length = 0;
      renderer.setRenderTarget(target);
      const started = performance.now();
      renderer.render(scene, camera);
      const submitted = performance.now();
      const pixels = new Uint8Array(width * height * 4);
      renderer.readRenderTargetPixels(target, 0, 0, width, height, pixels);
      submitMs = submitted - started;
      readbackMs = performance.now() - submitted;
      const error = context.getError();
      if (error !== context.NO_ERROR) errors.push(error);
      return pixels;
    },
    image(pixels: Uint8Array): string {
      const canvas = document.createElement("canvas");
      canvas.width = width; canvas.height = height;
      const drawing = canvas.getContext("2d");
      if (drawing === null) throw new Error("Missing image capture Canvas");
      const data = drawing.createImageData(width, height);
      for (let row = 0; row < height; row++) {
        data.data.set(pixels.subarray(row * width * 4, (row + 1) * width * 4), (height - row - 1) * width * 4);
      }
      drawing.putImageData(data, 0, 0);
      return canvas.toDataURL("image/png");
    }
    };
    try {
      window.__waterCheck.suspendLoop = true;
      renderer.shadowMap.autoUpdate = false;
      renderer.setScissorTest(false);
      for (const { chunk } of retained) chunk.onBeforeRender = () => { draws.push(chunk.name); };
      scene.updateMatrixWorld(true);
      const center = water.getWorldPosition(new three.Vector3());
      const camera = new three.PerspectiveCamera(60, width / height, 0.05, 100_000);
      camera.position.copy(center).add(new three.Vector3(0, 10, 0));
      const cameras: { name: string; camera: import("three").Camera }[] = [];
      for (const degrees of [0, 90, 180, 270]) {
        const radians = degrees * Math.PI / 180;
        camera.lookAt(center.x + Math.sin(radians) * 1000, center.y - 45, center.z - Math.cos(radians) * 1000);
        camera.updateMatrixWorld(true);
        cameras.push({ name: `heading-${String(degrees)}`, camera: camera.clone() });
      }
      camera.lookAt(center.x, center.y - 45, center.z - 1000);
      camera.updateMatrixWorld(true);
      const stereo = new three.StereoCamera();
      stereo.aspect = 0.5;
      stereo.update(camera);
      cameras.push({ name: "left-eye", camera: stereo.cameraL }, { name: "right-eye", camera: stereo.cameraR });
      for (const view of cameras) {
        for (const chunk of chunks) chunk.visible = true;
        monolith.visible = false;
        const chunkPixels = probe.render(view.camera);
        const chunkSubmitMs = submitMs;
        const chunkReadbackMs = readbackMs;
        const visible = new Set(draws);
        const submittedVertices = chunks.filter(chunk => visible.has(chunk.name))
          .reduce((sum, chunk) => sum + chunk.geometry.getAttribute("position").count, 0);
        const drawCalls = renderer.info.render.calls;
        for (const chunk of chunks) chunk.visible = false;
        monolith.visible = true;
        const referencePixels = probe.render(view.camera);
        const difference = probe.compare(chunkPixels, referencePixels);
        if (new Set(chunkPixels).size < 16) throw new Error("Water image has insufficient color variation");
        measurements.push({ camera: view.name, chunks: visible.size, submittedVertices, drawCalls, ...difference,
          chunkSubmitMs, chunkReadbackMs, monolithSubmitMs: submitMs, monolithReadbackMs: readbackMs });
        if (input.quality === "high" && ["heading-0", "left-eye"].includes(view.name)) images.push({ name: view.name, data: probe.image(chunkPixels) });
        if (input.reference !== undefined) {
          for (const chunk of chunks) chunk.visible = true;
          monolith.visible = false;
          for (const elapsed of [0, 0.03, 1]) {
            timeUniform.value = previousTime + elapsed;
            const pixels = probe.render(view.camera);
            shaders.select(true);
            const originalPixels = probe.render(view.camera);
            shaders.select(false);
            shaderMeasurements.push({ camera: view.name, timeSeconds: timeUniform.value as number, ...probe.compare(pixels, originalPixels) });
          }
          timeUniform.value = previousTime;
          if (view.name === "heading-0" && timerAvailability === null) {
            for (let sample = 0; sample < 6; sample++) {
              for (const useReference of sample % 2 === 0 ? [true, false] : [false, true]) {
                shaders.select(useReference);
                probe.render(view.camera);
                await probe.gpu(view.camera, useReference ? "reference" : "production");
              }
            }
            shaders.select(false);
          }
        }
      }
      const precision = context.getShaderPrecisionFormat(context.FRAGMENT_SHADER, context.HIGH_FLOAT);
      const drawingSize = renderer.getDrawingBufferSize(new three.Vector2());
      return { quality: input.quality, segments, maxVerticesPerDraw: Math.max(...chunks.map(chunk => chunk.geometry.getAttribute("position").count)),
        timing: "CPU wall-clock durations of command submission and synchronous readback, in milliseconds",
        drawingBuffer: { width: drawingSize.x, height: drawingSize.y, pixelRatio: renderer.getPixelRatio() },
        reflectionEnabled: waterMaterial.uniforms.uReflectionEnabled?.value as number | undefined,
        shadowEnabled: renderer.shadowMap.enabled, measurements, errors, contextLost: context.isContextLost(), images,
        shaderComparison: input.reference === undefined ? null : { sourceHead: input.reference.sourceHead, measurements: shaderMeasurements,
          gpuTiming: { target: { width, height }, camera: "heading-0", shadowAutoUpdate: false, animationLoopSuspended: true, samples: gpuSamples,
            availability: timerAvailability, failures: gpuTimingFailures } },
        gpu: { renderer: context.getParameter(context.RENDERER) as string, vendor: context.getParameter(context.VENDOR) as string,
          maxVaryingVectors: context.getParameter(context.MAX_VARYING_VECTORS) as number,
          maxFragmentUniformVectors: context.getParameter(context.MAX_FRAGMENT_UNIFORM_VECTORS) as number,
          timerQueryAvailable: context.getExtension("EXT_disjoint_timer_query_webgl2") !== null,
          parallelShaderCompileAvailable: context.getExtension("KHR_parallel_shader_compile") !== null,
          fragmentHighFloat: precision === null ? null : { rangeMin: precision.rangeMin, rangeMax: precision.rangeMax, precision: precision.precision } } };
    } finally {
      try { timer.clear(); } finally {
        shaders.select(false);
        timeUniform.value = previousTime;
        for (const { chunk, visible, before } of retained) {
          chunk.visible = visible;
          if (before === undefined) Reflect.deleteProperty(chunk, "onBeforeRender");
          else Object.defineProperty(chunk, "onBeforeRender", before);
        }
        water.remove(monolith);
        original.dispose();
        target.dispose();
        renderer.shadowMap.autoUpdate = previousShadowUpdate;
        renderer.setRenderTarget(previousTarget);
        renderer.setViewport(previousViewport);
        renderer.setScissor(previousScissor);
        renderer.setScissorTest(previousScissorTest);
        window.__waterCheck.suspendLoop = previousLoopSuspension;
      }
    }
  }, { geometry: geometryUrl, quality, reference });
  for (const image of result.images) await writeFile(resolve(output, `water-${quality}-${image.name}.png`), Buffer.from(image.data.split(",")[1] ?? "", "base64"));
  const summary = { ...result, images: result.images.map(image => image.name) };
  await writeFile(resolve(output, `water-${quality}.json`), `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  for (const view of result.measurements) {
    assert.ok(view.chunks > 0 && view.chunks < 16, `${quality} ${view.camera}: expected partial chunk culling`);
    assert.ok(view.submittedVertices < (result.segments + 1) ** 2, `${quality} ${view.camera}: expected reduced submitted vertices`);
    assert.ok(view.maximumChannelError <= 1, `${quality} ${view.camera}: chunk/monolith image mismatch ${String(view.maximumChannelError)}`);
  }
  assert.deepEqual(result.errors, []);
  assert.equal(result.contextLost, false);
  for (const view of result.shaderComparison?.measurements ?? []) {
    assert.ok(view.maximumChannelError <= 1, `${quality} ${view.camera}: shader image mismatch ${String(view.maximumChannelError)}`);
  }
  return summary;
}
