import type { DictionaryCollectionsClient } from "../dictionary/dictionary-collections";
import type {
  CommunityLocalDictionaryClient,
  CommunityResource,
  CommunityResourceClient,
  CommunitySharedWord,
} from "./community-resource-model";

export type CommunityDictionaryInstallResult =
  | { kind: "collection" }
  | { kind: "entries"; applied: number };

/** 把设置宿主的命名词库能力适配为社区图库需要的本机词库客户端。 */
export function createCommunityLocalDictionary(
  dictionary: CommunityLocalDictionaryClient | undefined,
  collections: DictionaryCollectionsClient | undefined,
): CommunityLocalDictionaryClient | undefined {
  if (!collections) return dictionary;
  const thirdPartyMatches = collections.thirdPartyMatches;
  return {
    ...dictionary,
    installCollection: async (resource) => {
      await collections.installCommunity(resource);
    },
    ...(thirdPartyMatches && {
      thirdPartyMatches: async (entries) =>
        (
          await thirdPartyMatches(
            entries.map((entry) => ({
              kind: entry.kind === "quick" ? ("quickPhrase" as const) : entry.kind,
              key: entry.code,
              value: entry.word,
              weight: entry.weight,
            })),
          )
        ).third_party_matches ?? [],
    }),
  };
}

/** 宿主能否把共享词库装成命名词库或导入本机词库。 */
export function canInstallCommunityDictionary(
  dictionary: CommunityLocalDictionaryClient | undefined,
): dictionary is CommunityLocalDictionaryClient {
  return Boolean(dictionary?.installCollection || dictionary?.import);
}

/** 命名词库优先，否则按词条类型导入；调用方根据结果保留各入口的提示。 */
export async function installCommunityDictionaryLocally(
  dictionary: CommunityLocalDictionaryClient,
  item: CommunityResource,
): Promise<CommunityDictionaryInstallResult | undefined> {
  if (dictionary.installCollection) {
    await dictionary.installCollection(item);
    return { kind: "collection" };
  }
  if (!dictionary.import) return undefined;
  const applied = await importCommunityDictionary(
    { import: dictionary.import.bind(dictionary) },
    item.content.entries ?? [],
  );
  return { kind: "entries", applied };
}

/** 回复模板的「添加」：收藏它，再把重新读到的最新一版存进本机「高情商回复」键盘，返回要提示的话。没有这个键盘的宿主只收藏。社区图库行上的「添加」和「AI 辅助」页「回复模板」一组的「添加」都走这里。 */
export async function addCommunityReply(
  client: Pick<CommunityResourceClient, "save" | "detail" | "storeReply">,
  item: CommunityResource,
): Promise<string> {
  await client.save(item.id, true);
  const latest = await client.detail(item.id);
  if (!client.storeReply) return `已收藏「${item.name}」`;
  await client.storeReply(latest);
  return `已添加「${item.name}」到高情商回复键盘`;
}

/** 把共享词库的词条导入本机词库，每种词条类型发一次请求，返回实际应用的词条数。 */
async function importCommunityDictionary(
  dictionary: Required<Pick<CommunityLocalDictionaryClient, "import">>,
  entries: readonly CommunitySharedWord[],
): Promise<number> {
  const groups = new Map<CommunitySharedWord["kind"], CommunitySharedWord[]>();
  for (const entry of entries) {
    const group = groups.get(entry.kind) ?? [];
    group.push(entry);
    groups.set(entry.kind, group);
  }
  let applied = 0;
  for (const [entryKind, group] of groups) {
    const kind = entryKind === "quick" ? "quick_phrase" : entryKind;
    const text = group.map((entry) => `${entry.word}\t${entry.code}\t${entry.weight}`).join("\n");
    const result = await dictionary.import(
      kind,
      "standard",
      text,
      `community-local-${Date.now()}-${entryKind}`,
    );
    applied += result.applied;
  }
  return applied;
}
