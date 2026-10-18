import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  // Historical Native diagnostic copies are evidence, not executable tests.
  // Do NOT use singleFork/singleThread for whole-shard runs: one long-lived
  // worker RPC channel dies after ~7+ minutes of sync Boundary hashing and
  // surfaces as [vitest-worker] onTaskUpdate timeouts after all tests pass.
  // Keep forks + fileParallelism=false + maxWorkers=1 so each file gets a
  // fresh child_process/RPC session, sequenced one at a time.
  test: {
    setupFiles: ["./scripts/testing/boundary-rpc-yield.mjs"],
    exclude: [...configDefaults.exclude, "**/.ai-bridge/**", "**/local-fixtures/**",
      // Phase4 negative probes copy source into owned directories. A crashed
      // probe may survive cleanup; its mirrored tests are never test entries.
      "**/.codex-ui-phase4-probe-*/**"],
    pool: "forks",
    fileParallelism: false,
    teardownTimeout: 120_000,
    poolOptions: {
      forks: {
        singleFork: false,
        isolate: true,
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
