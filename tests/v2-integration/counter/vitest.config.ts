import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "node",
    testTimeout: 60_000,
    setupFiles: ["./setup.ts"],
    teardownTimeout: 20000,
    isolate: true,
    fileParallelism: false,
    maxConcurrency: 1,
  },
});
