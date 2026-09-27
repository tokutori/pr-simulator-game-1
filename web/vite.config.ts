import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { verifiedAssets } from "../tools/asset-check/vite-plugin.js";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  base: "/pr-simulator-game-1/",
  publicDir: false,
  plugins: [verifiedAssets(fileURLToPath(new URL("..", import.meta.url)))],
  build: { outDir: "dist", emptyOutDir: true }
});
