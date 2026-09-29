import { hapTasks } from "@ohos/hvigor-ohos-plugin";
import * as fs from "fs";
import * as path from "path";

/** The settings page is built by platforms/harmony/stage-settings.sh rather than committed. A HAP without it installs and runs but opens a blank settings window with no error anywhere, so hvigor stops here and says what to run instead. */
function settingsPagePlugin() {
  return {
    pluginId: "msimeSettingsPagePlugin",
    apply(node: { getNodePath(): string }): void {
      const page: string = path.join(
        node.getNodePath(),
        "src",
        "main",
        "resources",
        "rawfile",
        "settings",
        "index.html",
      );
      if (!fs.existsSync(page) || fs.statSync(page).size === 0) {
        throw new Error(
          `${page} is missing: run bash platforms/harmony/stage-settings.sh (after pnpm install at the repository root) before hvigorw.`,
        );
      }
    },
  };
}

export default {
  system: hapTasks /* Built-in plugin of Hvigor. It cannot be modified. */,
  plugins: [settingsPagePlugin()] /* Custom plugin to extend the functionality of Hvigor. */,
};
