import { defineConfig } from "vite";
import { svelte, vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// The host serves `dist/` (host/src/link/web.rs). `npm run dev` proxies the link to a host
// started with `curia-host serve --port 47100`.
export default defineConfig({
  plugins: [svelte({ preprocess: vitePreprocess() })],
  build: { outDir: "dist", emptyOutDir: true, target: "es2022" },
  server: { proxy: { "/ws": { target: "ws://127.0.0.1:47100", ws: true } } },
});
