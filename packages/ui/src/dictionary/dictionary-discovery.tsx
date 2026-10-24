import { useEffect, useState } from "react";
import type {
  CommunityResource,
  CommunityResourceClient,
} from "../community/community-resource-model";
import { communityResourceSubtitle } from "../community/community-resource-model";
import { GroupList, NavRow } from "../core/platform-controls";
import { useAsyncGeneration } from "../settings/use-async-generation";
import * as settings from "../settings/settings-style";
import type { DictionaryCollectionsView } from "./dictionary-collections";
import { Badge } from "./dictionary-detail-parts";

/** 「发现词库」列出的社区词库数，与 Android 的 `DISCOVER_LIMIT` 相同。 */
const DISCOVER_LIMIT = 8;

/** 「发现词库」：社区词库的前几个，「添加」后成为一个独立的词库；已经装过的显示「已添加」。最后一行「浏览更多词库」切到底部「社区」标签的「词库」分类，搜索、详情、发布、我的作品和收藏都在那里。 */
export function DiscoverDictionaries({
  community,
  view,
  busy,
  onInstall,
  onBrowseMore,
}: {
  community: CommunityResourceClient;
  view: DictionaryCollectionsView | null;
  busy: boolean;
  onInstall: (resource: CommunityResource) => Promise<DictionaryCollectionsView | undefined>;
  onBrowseMore: () => void;
}) {
  const [items, setItems] = useState<CommunityResource[] | null>(null);
  const [failure, setFailure] = useState("");
  const [adding, setAdding] = useState<string | null>(null);
  const discoveryGeneration = useAsyncGeneration(community);
  useEffect(() => {
    const current = discoveryGeneration.current;
    community
      .list("dictionary", "", "", 0)
      .then(
        (page) =>
          discoveryGeneration.current === current && setItems(page.items.slice(0, DISCOVER_LIMIT)),
      )
      .catch(
        () =>
          discoveryGeneration.current === current && setFailure("社区词库暂时读不到，请稍后再试。"),
      );
  }, [community]);
  const installed = (id: string) =>
    view?.collections.some(
      (item) => item.source.type === "community" && item.source.resource_id === id,
    ) ?? false;
  return (
    <GroupList title="发现词库">
      {items === null || items.length === 0 ? (
        <p className={settings.groupNote} role={failure ? "alert" : undefined}>
          {failure || (items === null ? "正在读取社区词库…" : "社区里还没有词库。")}
        </p>
      ) : (
        items.map((item) => {
          const done = installed(item.id);
          const pending = adding === item.id;
          return (
            <NavRow
              key={item.id}
              variant="me"
              icon={<Badge>{Array.from(item.name)[0] ?? "词"}</Badge>}
              title={item.name}
              description={communityResourceSubtitle(item)}
              trailing={
                <button
                  type="button"
                  className="secondary"
                  aria-label={`添加词库 ${item.name}`}
                  disabled={done || pending || busy}
                  onClick={async () => {
                    setAdding(item.id);
                    try {
                      await onInstall(item);
                    } finally {
                      setAdding(null);
                    }
                  }}
                >
                  {done ? "已添加" : pending ? "添加中" : "添加"}
                </button>
              }
            />
          );
        })
      )}
      <NavRow variant="me" title="浏览更多词库" onClick={onBrowseMore} />
    </GroupList>
  );
}
