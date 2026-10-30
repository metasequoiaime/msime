// Tauri runs this before every `tauri build` / `tauri dev` / `tauri android|ios build` (beforeBuildCommand and beforeDevCommand in src-tauri/tauri.conf.json). The product on each OS is the native host under platforms/<os>; this Tauri project is a shared component those hosts launch, so building it on its own produces something that is not the input method and must not be run, debugged or accepted as one (AGENTS.md). The platform build scripts that legitimately drive the Tauri CLI set MSIME_PLATFORM_BUILD=1; anything else stops here before a single crate compiles.
import process from "node:process";

if (process.env.MSIME_PLATFORM_BUILD !== "1") {
  const entries = {
    win32:
      "Windows: platforms/windows/Build-Client.ps1 (build) or platforms/windows/installer/test.ps1 (build and install)",
    darwin:
      "macOS: platforms/macos (see platforms/macos/README.md); iOS: platforms/ios/build-app.sh",
    linux: "Linux: platforms/linux/build-container.sh; Android: platforms/android/build-apk.sh",
  };
  console.error(
    [
      "",
      "错误：不能直接构建或运行 apps/desktop 的 Tauri 工程。",
      "水杉输入法在每个系统上的产品都是 platforms/<os> 下的原生宿主，Tauri 工程只是宿主按需拉起的公共组件，单独编出来的不是输入法。",
      "",
      "Error: do not build or run the Tauri project in apps/desktop directly.",
      "The product on every OS is the native host under platforms/<os>; this Tauri project is a shared component the host launches, and building it alone does not produce the input method.",
      "",
      `Use the platform entry instead — ${entries[process.platform] ?? "see platforms/<os>/README.md"}.`,
      "The platform build scripts set MSIME_PLATFORM_BUILD=1 when they need the Tauri component.",
      "",
    ].join("\n"),
  );
  process.exit(1);
}
