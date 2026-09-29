// Bundle a synthetic settings host with the real UI and styles for local tests.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
const require = createRequire(new URL("../apps/desktop/package.json", import.meta.url));
const { build } = await import(require.resolve("vite"));
// The shared stylesheet is Tailwind v4 source (`@import "tailwindcss/..."`, `@utility`, `@custom-variant`); without the plugin those rules never reach settings.css.
const { default: tailwind } = await import(require.resolve("@tailwindcss/vite"));
if (!process.argv[2]) throw new Error("pass a temporary output directory");
await build({
  configFile: false, publicDir: false,
  // Resolve `tailwindcss` from the desktop app, which owns that dependency, as its own build does.
  root: fileURLToPath(new URL("../apps/desktop", import.meta.url)),
  plugins: [tailwind()],
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: {
    outDir: resolve(process.argv[2]), emptyOutDir: false,
    lib: { entry: fileURLToPath(new URL("../apps/desktop/src/browser-fixtures/settings.tsx", import.meta.url)), formats: ["es"], cssFileName: "settings" },
    rollupOptions: { output: { entryFileNames: "settings.js" } },
  },
});
