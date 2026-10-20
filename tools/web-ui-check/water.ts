import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import type { Page } from "playwright-core";
import type { Group, Scene, ShaderMaterial, WebGLRenderer } from "three";

declare global {
  interface Window {
    __waterCheck: { renderer: WebGLRenderer; scene: Scene; water: Group; waterMaterial: ShaderMaterial; threeModuleUrl: string };
  }
}

export async function observeWaterRenderer(page: Page): Promise<void> {
  await page.route("**/src/render/engines/three/three-renderer.ts*", async route => {
    const response = await route.fetch();
    const source = await response.text();
    const anchor = "scene.add(water);";
    assert.equal(source.split(anchor).length, 2, "Missing unique water renderer observation point");
    const threeModuleUrl = /from\s*["']([^"']*\/three\.js(?:\?[^"']*)?)["']/.exec(source)?.[1];
    assert.ok(threeModuleUrl !== undefined, "Missing Vite Three.js module URL");
    await route.fulfill({ response, body: source.replace(anchor,
      `${anchor}\nwindow.__waterCheck = { renderer, scene, water, waterMaterial, threeModuleUrl: ${JSON.stringify(threeModuleUrl)} };`) });
  });
}

export async function verifyWaterRenderer(page: Page, geometryUrl: string, output: string, quality: "low" | "medium" | "high"): Promise<unknown> {
  const result = await page.evaluate(async input => {
    const geometryModule = await import(input.geometry) as typeof import("../../web/src/render/engines/three/lake-water-geometry.js");
    const { renderer, scene, water, waterMaterial, threeModuleUrl } = window.__waterCheck;
    const three = await import(threeModuleUrl) as typeof import("three");
    const chunks = water.children.filter((node): node is import("three").Mesh<import("three").BufferGeometry, ShaderMaterial> => "isMesh" in node && node.isMesh === true);
    const segments = { low: 96, medium: 144, high: 224 }[input.quality];
    if (chunks.length !== 16 || chunks.some(chunk => chunk.geometry.getAttribute("position").count !== (segments / 4 + 1) ** 2)) {
      throw new Error(`Water ${input.quality}: missing expected chunk geometry`);
    }
    const first = chunks[0];
    if (first === undefined) throw new Error("Missing water chunk");
    const context = renderer.getContext();
    if (context.getError() !== context.NO_ERROR || context.isContextLost()) throw new Error("WebGL is unhealthy before water verification");
    const width = 640;
    const height = 360;
    const previousTarget = renderer.getRenderTarget();
    const previousViewport = renderer.getViewport(new three.Vector4());
    const previousScissor = renderer.getScissor(new three.Vector4());
    const previousScissorTest = renderer.getScissorTest();
    const previousShadowUpdate = renderer.shadowMap.autoUpdate;
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
    const measurements: { camera: string; chunks: number; submittedVertices: number; drawCalls: number; maximumChannelError: number; changedPixels: number }[] = [];
    const errors: number[] = [];
    const render = (camera: import("three").Camera): Uint8Array => {
      draws.length = 0;
      renderer.setRenderTarget(target);
      renderer.render(scene, camera);
      const pixels = new Uint8Array(width * height * 4);
      renderer.readRenderTargetPixels(target, 0, 0, width, height, pixels);
      const error = context.getError();
      if (error !== context.NO_ERROR) errors.push(error);
      return pixels;
    };
    const image = (pixels: Uint8Array): string => {
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
    };
    try {
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
        const chunkPixels = render(view.camera);
        const visible = new Set(draws);
        const submittedVertices = chunks.filter(chunk => visible.has(chunk.name))
          .reduce((sum, chunk) => sum + chunk.geometry.getAttribute("position").count, 0);
        const drawCalls = renderer.info.render.calls;
        for (const chunk of chunks) chunk.visible = false;
        monolith.visible = true;
        const referencePixels = render(view.camera);
        let maximumChannelError = 0;
        let changedPixels = 0;
        for (let pixel = 0; pixel < width * height; pixel++) {
          let changed = false;
          for (let channel = 0; channel < 4; channel++) {
            const delta = Math.abs((chunkPixels[pixel * 4 + channel] ?? 0) - (referencePixels[pixel * 4 + channel] ?? 0));
            maximumChannelError = Math.max(maximumChannelError, delta);
            changed ||= delta > 0;
          }
          if (changed) changedPixels++;
        }
        if (new Set(chunkPixels).size < 16) throw new Error("Water image has insufficient color variation");
        measurements.push({ camera: view.name, chunks: visible.size, submittedVertices, drawCalls, maximumChannelError, changedPixels });
        if (input.quality === "high" && ["heading-0", "left-eye"].includes(view.name)) images.push({ name: view.name, data: image(chunkPixels) });
      }
      return { quality: input.quality, segments, maxVerticesPerDraw: Math.max(...chunks.map(chunk => chunk.geometry.getAttribute("position").count)),
        reflectionEnabled: waterMaterial.uniforms.uReflectionEnabled?.value as number | undefined,
        shadowEnabled: renderer.shadowMap.enabled, measurements, errors, contextLost: context.isContextLost(), images,
        gpu: { renderer: context.getParameter(context.RENDERER) as string, vendor: context.getParameter(context.VENDOR) as string,
          maxVaryingVectors: context.getParameter(context.MAX_VARYING_VECTORS) as number } };
    } finally {
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
    }
  }, { geometry: geometryUrl, quality });
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
  return summary;
}
