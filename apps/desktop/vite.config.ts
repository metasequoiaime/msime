import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";
export default defineConfig({
  plugins: [react(), tailwind()],
  server: { port: 1420, strictPort: true },
  clearScreen: false,
  test: {
    css: {
      include: [
        /styles\.css/,
        /variables\.css/,
        /skin-candidate-decorations\.css/,
        /skin-toolbar-preview\.css/,
        /external-skin-geometry\.css/,
      ],
    },
    // These render the whole settings page, and some drive it through dozens
    // of changes; the font-size test alone re-renders 42 times. Vitest isolates
    // each file in its own worker - 60-odd of them here - so they also compete
    // for the machine while doing it. At the 5 s default, five tests failed on
    // timing alone while passing individually, which reads as a broken suite
    // rather than a slow one.
    //
    // 20 s was not enough either. On a machine also running a build, the suite
    // fails a different handful every run - eight one time, two the next, each
    // of them a timeout and each passing alone - and a gate that names a
    // different culprit every run teaches people to ignore it. The tests are
    // not near this budget when the machine is idle; the number exists for when
    // it is not.
    testTimeout: 60000,
    hookTimeout: 60000,
    // Keep the isolated jsdom workers from saturating the host. The default is
    // the machine's full parallelism, which made the suite intermittently fail
    // with assertion-level races under load even though the affected files
    // passed alone and with a bounded worker pool.
    maxWorkers: 8,
    setupFiles: ["tests/support/reset-history.ts"],
  },
});
