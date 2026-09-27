import { createHash } from "node:crypto";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { stringify } from "smol-toml";
import { build } from "vite";
import { expect, it } from "vitest";
import { verifiedAssets } from "./vite-plugin.js";

it("builds a manifest-registered asset through the same import path", async () => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    const image = "<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/test.svg"), image, "utf8");
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [{
      path: "assets/test.svg", sha256: createHash("sha256").update(image).digest("hex"),
      source: "original:test", source_version: "1", license: "MIT", license_url: "LICENSE",
      attribution: "Test", processing: "none"
    }] }), "utf8");
    await writeFile(join(root, "web/index.html"), "<script type=\"module\" src=\"/src/main.ts\"></script>", "utf8");
    await writeFile(join(root, "web/src/main.ts"), "import url from '../../assets/test.svg'; document.body.textContent = url;", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } })).resolves.toBeDefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it("builds a registered icon referenced by HTML and a module", async () => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    const icon = Buffer.from([0, 0, 1, 0, 1, 0]);
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/test.ico"), icon);
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [{
      path: "assets/test.ico", sha256: createHash("sha256").update(icon).digest("hex"),
      source: "original:test", source_version: "1", license: "MIT", license_url: "LICENSE",
      attribution: "Test", processing: "none"
    }] }), "utf8");
    await writeFile(join(root, "web/index.html"),
      "<link rel=\"icon\" href=\"../assets/test.ico\"><script type=\"module\" src=\"/src/main.ts\"></script>", "utf8");
    await writeFile(join(root, "web/src/main.ts"),
      "import url from '../../assets/test.ico'; document.body.textContent = url;", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } })).resolves.toBeDefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it("builds a registered JSON resource through a static new URL", async () => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    const scenario = "{\"name\":\"test\"}";
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/scenario.json"), scenario, "utf8");
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [{
      path: "assets/scenario.json", sha256: createHash("sha256").update(scenario).digest("hex"),
      source: "original:test", source_version: "1", license: "MIT", license_url: "LICENSE",
      attribution: "Test", processing: "none"
    }] }), "utf8");
    await writeFile(join(root, "web/index.html"), "<script type=\"module\" src=\"/src/main.ts\"></script>", "utf8");
    await writeFile(join(root, "web/src/main.ts"),
      "document.body.textContent = new URL('../../assets/scenario.json', import.meta.url).href;", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } })).resolves.toBeDefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it("builds a Vite worker URL without treating source code as a manifest asset", async () => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [] }), "utf8");
    await writeFile(join(root, "web/index.html"), "<script type=\"module\" src=\"/src/main.ts\"></script>", "utf8");
    await writeFile(join(root, "web/src/main.ts"),
      "new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });", "utf8");
    await writeFile(join(root, "web/src/worker.ts"), "postMessage('ready');", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } })).resolves.toBeDefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it("builds a registered HTML resource whose path contains spaces", async () => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    const image = "<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/lake map.svg"), image, "utf8");
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [{
      path: "assets/lake map.svg", sha256: createHash("sha256").update(image).digest("hex"),
      source: "original:test", source_version: "1", license: "MIT", license_url: "LICENSE",
      attribution: "Test", processing: "none"
    }] }), "utf8");
    await writeFile(join(root, "web/index.html"), "<img src=\"../assets/lake map.svg\">", "utf8");
    await writeFile(join(root, "web/src/main.ts"), "document.body.textContent = 'ready';", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } })).resolves.toBeDefined();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it("checks every HTML srcset candidate", async () => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    const image = "<svg xmlns=\"http://www.w3.org/2000/svg\"/>";
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/registered.svg"), image, "utf8");
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [{
      path: "assets/registered.svg", sha256: createHash("sha256").update(image).digest("hex"),
      source: "original:test", source_version: "1", license: "MIT", license_url: "LICENSE",
      attribution: "Test", processing: "none"
    }] }), "utf8");
    await writeFile(join(root, "unregistered.svg"), image, "utf8");
    await writeFile(join(root, "web/index.html"),
      "<img srcset=\"../assets/registered.svg 1x, ../unregistered.svg 2x\">", "utf8");
    await writeFile(join(root, "web/src/main.ts"), "document.body.textContent = 'ready';", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } }))
      .rejects.toThrow("Unregistered build asset");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

it.each([
  ["module import", "<script type=\"module\" src=\"/src/main.ts\"></script>", "import url from '../../unmanaged.svg'; document.body.textContent = url;"],
  ["new URL", "<script type=\"module\" src=\"/src/main.ts\"></script>", "document.body.textContent = new URL('../../unmanaged.svg', import.meta.url).href;"],
  ["new URL JSON", "<script type=\"module\" src=\"/src/main.ts\"></script>", "document.body.textContent = new URL('../../unmanaged.json', import.meta.url).href;"],
  ["new URL data file", "<script type=\"module\" src=\"/src/main.ts\"></script>", "document.body.textContent = new URL('../../unmanaged.dat', import.meta.url).href;"],
  ["dynamic new URL", "<script type=\"module\" src=\"/src/main.ts\"></script>", "const image = '../../unmanaged.svg'; document.body.textContent = new URL(image, import.meta.url).href;"],
  ["CSS URL", "<script type=\"module\" src=\"/src/main.ts\"></script>", "import './style.css';"],
  ["HTML URL", "<img src=\"../unmanaged.svg\">", ""],
  ["HTML URL with spaces", "<img src=\"../lake map.svg\">", ""],
  ["unquoted HTML URL", "<img src=../unmanaged.svg>", ""],
  ["HTML srcset candidate", "<img srcset=\"../unmanaged.svg 1x, ../unmanaged.svg 2x\">", ""],
  ["inline HTML style", "<style>body{background:url('../unmanaged.svg')}</style>", ""],
  ["HTML icon", "<link rel=\"icon\" href=\"../unmanaged.ico\">", ""],
  ["module icon", "<script type=\"module\" src=\"/src/main.ts\"></script>",
    "import url from '../../unmanaged.ico'; document.body.textContent = url;"]
])("rejects an unregistered %s even when Vite inlines it", async (kind, html, source) => {
  const root = await mkdtemp(join(tmpdir(), "birdman-build-assets-"));
  if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-build-assets-")) {
    throw new Error("Temporary cleanup escaped the test directory");
  }
  try {
    await mkdir(join(root, "assets"));
    await mkdir(join(root, "web/src"), { recursive: true });
    await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [] }), "utf8");
    await writeFile(join(root, "web/index.html"), html, "utf8");
    await writeFile(join(root, "unmanaged.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>", "utf8");
    await writeFile(join(root, "unmanaged.ico"), Buffer.from([0, 0, 1, 0, 1, 0]));
    await writeFile(join(root, "unmanaged.json"), "{}", "utf8");
    await writeFile(join(root, "unmanaged.dat"), Buffer.from([0, 1, 2]));
    await writeFile(join(root, "lake map.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>", "utf8");
    await writeFile(join(root, "web/src/main.ts"), source, "utf8");
    if (kind === "CSS URL") await writeFile(join(root, "web/src/style.css"), "body { background: url('../../unmanaged.svg'); }", "utf8");
    await expect(build({ configFile: false, root: join(root, "web"), publicDir: false,
      plugins: [verifiedAssets(root)], logLevel: "silent", build: { outDir: "dist" } }))
      .rejects.toThrow(kind === "dynamic new URL" ? "Unsupported dynamic asset URL" : "Unregistered build asset");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
