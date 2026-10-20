import type {
  CloudDictionaryEntry,
  CloudDictionaryPanelClient,
  DictionaryClient,
  HostPlatform,
} from "@msime/ui";
import { cloudDictionaryCapabilities, isMobileHost } from "../input/mobile-host-capabilities";

/**
 * 设置页与独立窗口共用宿主能力和移动端单词条下载装配。
 *
 * `saveHostExport` 是宿主把导出文件写进「下载」的命令（`save_export`）。Linux 的 WebKitGTK 和 macOS 一样没有下载处理器，下载链接不可靠，所以 Linux 的面板用它保存云词库导出和完整备份；其他平台不用它，Windows 的 WebView2 照旧走下载链接。
 */
export function createDesktopCloudDictionaryClient(
  client: CloudDictionaryPanelClient,
  platform: HostPlatform | undefined,
  dictionaryClient: Pick<DictionaryClient, "importPersonal">,
  saveHostExport?: (name: string, contents: string) => Promise<unknown>,
): CloudDictionaryPanelClient {
  return {
    ...client,
    ...cloudDictionaryCapabilities(platform, client.request),
    ...(platform === "linux" && saveHostExport
      ? {
          saveExport: async (name: string, contents: string) => {
            await saveHostExport(name, contents);
          },
        }
      : {}),
    ...(isMobileHost(platform)
      ? {
          downloadToLocal: (entry: CloudDictionaryEntry) =>
            downloadCloudEntryToLocal(entry, dictionaryClient),
        }
      : {}),
  };
}

async function downloadCloudEntryToLocal(
  entry: CloudDictionaryEntry,
  dictionaryClient: Pick<DictionaryClient, "importPersonal">,
): Promise<void> {
  if (!dictionaryClient.importPersonal)
    throw new Error("personal dictionary import is unavailable");
  const text = JSON.stringify({
    format: "msime-personal-dictionary",
    version: 1,
    entries: [
      {
        kind: entry.kind === "quick" ? "quickPhrase" : entry.kind,
        key: entry.code,
        value: entry.word,
        weight: entry.weight,
      },
    ],
  });
  await dictionaryClient.importPersonal(text, `ui-cloud-download-${Date.now()}`);
}
