import eslint from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["node_modules/**", "target/**", "web/dist/**", "web/pkg/**", ".local/**"] },
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked.map((config) => ({ ...config, files: ["**/*.ts"] })),
  {
    files: ["**/*.ts"],
    languageOptions: { parserOptions: { project: ["./tsconfig.web.json", "./tsconfig.tools.json"], tsconfigRootDir: import.meta.dirname } },
    rules: { "@typescript-eslint/switch-exhaustiveness-check": "error" }
  },
  {
    files: ["web/src/**/*.ts"],
    rules: {
      "no-restricted-imports": ["error", {
        patterns: [{ group: ["node:*", "**/tools/**", "**/vite.config.*"], message: "Node.js and build tooling are unavailable in browser code." }],
        paths: ["assert", "buffer", "child_process", "crypto", "events", "fs", "http", "https", "os", "path", "process", "stream", "url", "util"]
      }]
    }
  }
);
