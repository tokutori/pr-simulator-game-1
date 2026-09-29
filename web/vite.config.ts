import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { verifiedAssets } from "../tools/asset-check/vite-plugin.js";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
let revision = process.env.GITHUB_SHA?.slice(0, 8);
if (revision === undefined) {
  try {
    revision = execFileSync("git", ["rev-parse", "--short=8", "HEAD"], {
      cwd: repositoryRoot,
      encoding: "utf8"
    }).trim();
  } catch {
    revision = "unknown";
  }
}
const buildLabel = `v${process.env.npm_package_version ?? "0.1.0"} · ${revision} · ${new Date().toISOString().slice(0, 16)}Z`;

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  base: "/pr-simulator-game-1/",
  publicDir: false,
  define: { __APP_BUILD_LABEL__: JSON.stringify(buildLabel) },
  plugins: [verifiedAssets(fileURLToPath(new URL("..", import.meta.url)))],
  build: {
    outDir: "dist", emptyOutDir: true,
    rolldownOptions: {
      input: {
        game: fileURLToPath(new URL("./index.html", import.meta.url)),
        airframePreview: fileURLToPath(new URL("./airframe-preview.html", import.meta.url))
      }
    }
  }
});
