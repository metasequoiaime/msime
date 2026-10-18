import type {
  CloudDictionaryEntry,
  CloudDictionaryPanelClient,
  DictionaryClient,
  HostPlatform,
} from "@msime/ui";
import { cloudDictionaryCapabilities, isMobileHost } from "../input/mobile-host-capabilities";

/** 设置页与独立窗口共用宿主能力和移动端单词条下载装配。 */
export function createDesktopCloudDictionaryClient(
  client: CloudDictionaryPanelClient,
  platform: HostPlatform | undefined,
  dictionaryClient: Pick<DictionaryClient, "importPersonal">,
): CloudDictionaryPanelClient {
  return {
    ...client,
    ...cloudDictionaryCapabilities(platform, client.request),
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
