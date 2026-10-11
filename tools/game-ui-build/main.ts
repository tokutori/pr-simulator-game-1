import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { generateGameUiFiles, parseSchemaBundle } from "./generator.js";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const mode = process.argv[2];
if ((mode !== "--write" && mode !== "--check") || process.argv.length !== 3) {
  throw new Error("Usage: tsx tools/game-ui-build/main.ts <--write|--check>");
}
const exporter = spawnSync("cargo", ["run", "-p", "birdman-game-cli", "--locked", "--features", "schema", "--", "export-game-ui-schema", "bundle"], {
  cwd: root,
  encoding: "utf8",
  stdio: ["inherit", "pipe", "inherit"],
  windowsHide: true,
  maxBuffer: 16 * 1024 * 1024
});
if (exporter.error !== undefined) throw exporter.error;
if (exporter.status !== 0) throw new Error(`Game UI schema exporter exited with status ${String(exporter.status)}`);
const files = await generateGameUiFiles(parseSchemaBundle(exporter.stdout), root);
const stale: string[] = [];
for (const file of files) {
  const filename = resolve(root, file.path);
  if (mode === "--write") {
    await mkdir(dirname(filename), { recursive: true });
    await writeFile(filename, file.content, "utf8");
  } else {
    let current: string | undefined;
    try {
      current = await readFile(filename, "utf8");
    } catch (cause) {
      if (typeof cause !== "object" || cause === null || !("code" in cause) || cause.code !== "ENOENT") throw cause;
    }
    if (current !== file.content) stale.push(file.path);
  }
}
if (stale.length > 0) throw new Error(`Generated game UI files are stale: ${stale.join(", ")}. Run npm run generate:game-ui.`);
process.stdout.write(`${mode === "--write" ? "Generated" : "Verified"} ${String(files.length)} game UI contract files\n`);
