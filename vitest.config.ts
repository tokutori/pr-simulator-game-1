import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "web/src/**/*.{test,spec}.?(c|m)[jt]s?(x)",
      "tools/**/*.{test,spec}.?(c|m)[jt]s?(x)"
    ],
    maxWorkers: 2
  }
});
