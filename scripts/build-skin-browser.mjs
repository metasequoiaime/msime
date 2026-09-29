// Bundle the actual skin helpers (including browser parser dependencies) for
// test-skin-palette-csp.py. No app host, user data or CI is involved.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
const require = createRequire(new URL("../apps/desktop/package.json", import.meta.url));
const { build } = await import(require.resolve("vite"));
if (!process.argv[2]) throw new Error("pass a temporary output directory");
await build({
  configFile: false,
  publicDir: false,
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: {
    outDir: resolve(process.argv[2]),
    emptyOutDir: false,
    lib: {
      entry: Object.fromEntries(["skin-palette", "skin-toolbar-css", "toolbar-images", "toolbar-fonts"].map(name =>
        [name, fileURLToPath(new URL("../packages/ui/src/skin/" + name + ".ts", import.meta.url))])),
      formats: ["es"],
    },
    rollupOptions: { output: { entryFileNames: "[name].js" } },
  },
});
