import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import pkg from "./package.json" with { type: "json" };

// In dev, /ws and /api are proxied to a local corde-server so that the
// production default (`ws://${location.host}/ws`) also works with `vite dev`.
const SERVER = "127.0.0.1:8471";

const VERSION: string = pkg.version;

// `vite build --mode mobile`: the same interface for the iPhone application (`apps/mobile`,
// `src/mobile/env.ts`). The browser build is unchanged: `APP_BUILD` is false there and the
// application's modules are left out.
export default defineConfig({
  plugins: [react()],
  base: "./",
  define: {
    __CORDE_VERSION__: JSON.stringify(VERSION),
  },
  server: {
    port: 5173,
    proxy: {
      "/api": { target: `http://${SERVER}`, changeOrigin: true },
      "/ws": { target: `ws://${SERVER}`, ws: true, rewriteWsOrigin: true },
    },
  },
  build: {
    target: "es2022",
    sourcemap: false,
    chunkSizeWarningLimit: 900,
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
