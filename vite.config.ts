/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import tailwindcss from "@tailwindcss/vite";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;
// Map the unprefixed env var contributors actually set. Vite only exposes VITE_*.
if (process.env.PULSE_MAIL_DEMO === "1") {
  process.env.VITE_PULSE_MAIL_DEMO = "1";
}

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [solid(), tailwindcss()],
  // Tauri expects a fixed port and wants Rust errors left visible.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
}));
