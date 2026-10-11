import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  // Historical Native diagnostic copies are evidence, not executable tests.
  // Forks + singleFork keep one child process for the whole shard so heavy
  // sync Boundary hashing cannot leave a threads/RPC worker half-closed.
  // Raised teardownTimeout covers Windows GC after ~15–17 minute shards;
  // without it vitest reports onTaskUpdate timeouts after all tests passed.
  test: {
    exclude: [...configDefaults.exclude, "**/.ai-bridge/**", "**/local-fixtures/**"],
    pool: "forks",
    fileParallelism: false,
    teardownTimeout: 120_000,
    poolOptions: {
      forks: {
        singleFork: true,
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
