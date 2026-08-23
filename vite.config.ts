import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const version = process.env.npm_package_version ?? "0.0.0";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  // Tailwind is a Vite plugin rather than a PostCSS step, and it needs no configuration file:
  // every value it resolves against is declared in `src/styles/tokens.css`.
  plugins: [react(), tailwindcss()],

  // Imports inside `src/` are absolute from the project root — `@/i18n`, never `../../i18n` —
  // so that moving a file does not rewrite a column of dots. The alias is declared here and
  // in `tsconfig.json`, and the two have to agree.
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // The version the first screen shows, taken from `package.json` at build time rather than
  // written on the screen where it would drift from the manifest within a release.
  define: {
    __APP_VERSION__: JSON.stringify(version),
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
      // 3. tell Vite to ignore watching the Rust side of the repository. `target` is named
      //    as well as `src-tauri` because the workspace root is here, so a cargo build
      //    rewrites thousands of files one directory up from the frontend.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
}));
