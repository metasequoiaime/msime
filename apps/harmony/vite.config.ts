import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";
import { existsSync, readFileSync, writeFileSync, rmSync, readdirSync } from "node:fs";
import { join } from "node:path";

/**
 * The settings UI as the HarmonyOS host serves it.
 *
 * Built into the HAP's rawfile directory and loaded through $rawfile, so the window needs no network
 * and no server. That comes with one constraint that shapes the whole config: a resource:// document
 * has a null origin, and the webview refuses to fetch a module script or a stylesheet across it, so
 * the bundle has to arrive as one file with nothing left to fetch. Hence a classic script rather
 * than a module, every asset inlined as a data URI, and the pass below that folds the script and the
 * styles into the HTML itself.
 */
function singleFile(): Plugin {
  return {
    name: "msime-single-file",
    enforce: "post",
    closeBundle() {
      const directory = join(
        import.meta.dirname,
        "../../platforms/harmony/entry/src/main/resources/rawfile/settings",
      );
      const page = join(directory, "index.html");
      let html = readFileSync(page, "utf8");
      for (const name of readdirSync(directory)) {
        if (name === "index.html") continue;
        // A bundle that happens to contain the literal "</script>" in a string would close the tag
        // it is being inlined into, and the rest of it would be parsed as markup.
        const body = readFileSync(join(directory, name), "utf8").replace(
          /<\/(script|style)/gi,
          "<\\/$1",
        );
        if (name.endsWith(".js")) {
          // Moved to the end of the body rather than inlined where it stood. A classic script runs
          // the moment it is parsed, and vite puts it in the head, so in place it would look for the
          // root element before the body had been parsed and find nothing — a blank window and not
          // one error to explain it.
          html = html.replace(new RegExp(`<script[^>]*src="[^"]*${name}"[^>]*></script>`), "");
          // A function replacer, not a string: in a string replacement `$\`` means "everything before
          // the match", and this bundle is full of backticks, so a string would splice the head of the
          // document into the middle of the script wherever one appeared.
          html = html.replace("</body>", () => `<script>${body}</script></body>`);
        } else if (name.endsWith(".css")) {
          html = html.replace(
            new RegExp(`<link[^>]*href="[^"]*${name}"[^>]*>`),
            () => `<style>${body}</style>`,
          );
        }
        rmSync(join(directory, name));
      }
      // Vite inlines the assets named through `new URL(..., import.meta.url)`, but not the ones it
      // judges too large, and what it leaves behind is a path to a file this build never emits and
      // the loop above would have deleted anyway — a broken image with nothing to fetch it from. The
      // app logo was over that line until it was redrawn, and nothing is over it today. Kept because
      // the failure is silent: the page renders, one image is simply missing, and no build step says
      // so. Inline the stragglers from source and the rule that nothing is left to fetch holds.
      html = html.replace(/\.\/assets\/([\w.-]+\.svg)/g, (reference, name: string) => {
        const file = join(import.meta.dirname, "../../packages/ui/src/assets", name);
        if (!existsSync(file)) return reference;
        return `data:image/svg+xml;base64,${readFileSync(file, "base64")}`;
      });
      writeFileSync(page, html);
    },
  };
}

export default defineConfig({
  // singleFile runs last on purpose: it folds whatever CSS the build emitted into the HTML, so
  // Tailwind has to have generated its utilities before it reads the directory.
  plugins: [react(), tailwind(), singleFile()],
  base: "./",
  // packages/ui names its icons with `new URL("./assets/x.svg", import.meta.url)`, which vite turns
  // into the inlined data URI at build time. A classic script has no import.meta for any that survive
  // the transform, and `new URL(x, undefined)` throws where it would merely have been wrong, taking
  // the whole page down with it. Naming the document gives those a base to resolve against.
  define: { "import.meta.url": JSON.stringify("resource://rawfile/settings/index.html") },
  build: {
    outDir: "../../platforms/harmony/entry/src/main/resources/rawfile/settings",
    emptyOutDir: true,
    // Nothing is fetched, so nothing may be left out of the one file that is read.
    assetsInlineLimit: Number.MAX_SAFE_INTEGER,
    cssCodeSplit: false,
    // resource:// 页面必须将脚本、样式和图标打入单文件，预算以未压缩产物计。
    chunkSizeWarningLimit: 2048,
    rollupOptions: {
      output: {
        format: "iife",
        codeSplitting: false,
        entryFileNames: "settings.js",
        assetFileNames: "settings.[ext]",
      },
    },
  },
});
