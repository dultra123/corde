import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// In dev, /ws and /api are proxied to a local corde-server so that the
// production default (`ws://${location.host}/ws`) also works with `vite dev`.
const SERVER = "127.0.0.1:8471";

export default defineConfig({
  plugins: [react()],
  base: "./",
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
