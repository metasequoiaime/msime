import { useEffect, useRef, useState } from "react";
import { useToast } from "../core/toast";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { appendUniqueById, resourceMessage } from "./community-helpers";
import { useCommunityClientLifecycle } from "./use-community-client-lifecycle";
import { useCommunityDetailHistory } from "./use-community-detail-history";
import {
  addCommunityReply,
  canInstallCommunityDictionary,
  installCommunityDictionaryLocally,
} from "./community-resource-actions";
import type {
  CommunityResourceClient,
  CommunityResourceKind,
  CommunityResourceScope,
  CommunityResource,
  CommunityLocalDictionaryClient,
} from "./community-resource-model";

export interface CommunityResourceGalleryOptions {
  client: Pick<CommunityResourceClient, "list" | "detail" | "apply" | "save" | "storeReply">;
  kind: CommunityResourceKind;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  mobile?: boolean;
  /** 手机上打开详情时压入的历史记录所属的设置页。触屏宿主在「词库」或「AI 辅助」页里打开这个图库，记录要留在那一页，返回手势才会先关详情、再关图库；缺省是桌面的「社区」页。 */
  historyPage?: string;
}

/** 资源图库的查询、分页、详情历史与逐项添加状态；显示外观由页面持有。 */
export function useCommunityResourceGallery({
  client,
  kind,
  initialScope = "",
  localDictionary,
  mobile = false,
  historyPage = "community",
}: CommunityResourceGalleryOptions) {
  const [scope, setScope] = useState<CommunityResourceScope>(initialScope);
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<CommunityResource[]>([]);
  const [more, setMore] = useState(false);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<CommunityResource | null>(null);
  const [editing, setEditing] = useState(false);
  const { mounted, clientGeneration } = useCommunityClientLifecycle(client, kind, scope);
  const activeSearch = useRef("");
  const load = async (append = false, query = activeSearch.current) => {
    const current = ++clientGeneration.current;
    setBusy(true);
    setError("");
    const offset = append ? items.length : 0;
    try {
      const page = await client.list(kind, scope, query, offset);
      if (!mounted.current || current !== clientGeneration.current) return;
      setItems((value) => (append ? appendUniqueById(value, page.items) : page.items));
      setMore(page.has_more);
      if (!append) activeSearch.current = query;
    } catch (loadError) {
      if (mounted.current && current === clientGeneration.current) {
        setError(resourceMessage(loadError));
        // 现有行属于上一次查询或范围；新请求失败后不能用它们的行数继续分页。
        if (!append) setMore(false);
      }
    } finally {
      if (mounted.current && current === clientGeneration.current) setBusy(false);
    }
  };
  useEffect(() => {
    void load();
  }, [client, kind, scope]);
  const openDetail = (item: CommunityResource) => {
    if (mobile && typeof window !== "undefined") {
      pushMobileSettingsState({ page: historyPage, communityDetail: { kind, id: item.id } });
    }
    setSelected(item);
  };
  const closeDetail = () => {
    if (
      mobile &&
      typeof window !== "undefined" &&
      window.history.state?.communityDetail?.kind === kind
    ) {
      window.history.back();
      return;
    }
    setSelected(null);
  };
  useCommunityDetailHistory({
    mobile,
    kind,
    selectedId: selected?.id ?? null,
    onClose: () => setSelected(null),
  });
  const toast = useToast();
  // 本次会话已添加和正在添加的条目，按 id 记录。没有宿主会报告哪些共享词库或回复模板已在设备上，所以每行初始都是「添加」；再次添加只会导入相同的词条或保留同一个模板，不产生任何变化。
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set());
  const [adding, setAdding] = useState<ReadonlySet<string>>(() => new Set());
  const addGeneration = useAsyncGeneration(client);
  const addingNow = useRef(new Map<string, number>());
  useEffect(() => {
    addingNow.current.clear();
    setAdded(new Set());
    setAdding(new Set());
  }, [client]);
  /** 「添加」：与详情页主按钮同一路径，之后弹出 toast。宿主能本地导入时，词库进入本机词库，不能时进入云词库；回复模板保存给「高情商回复」键盘使用。 */
  const add = async (item: CommunityResource) => {
    const generation = addGeneration.current;
    if (addingNow.current.has(item.id)) return;
    addingNow.current.set(item.id, generation);
    setAdding((current) => new Set(current).add(item.id));
    const current = () => mounted.current && generation === addGeneration.current;
    try {
      if (item.kind === "dictionary") {
        // 列表里的副本可能比服务器将导入的版本旧，所以词条取自重新获取的详情，与详情页的做法相同。
        const latest = await client.detail(item.id);
        if (!current()) return;
        if (canInstallCommunityDictionary(localDictionary)) {
          const result = await installCommunityDictionaryLocally(localDictionary, latest);
          if (!current()) return;
          toast(
            result?.kind === "collection"
              ? `已添加「${latest.name}」，可以在「词库」里停用或删除`
              : result?.kind === "entries"
                ? `已添加「${latest.name}」，本机词库应用 ${result.applied} 个词条`
                : "",
          );
        } else {
          const result = await client.apply(latest.id, latest.revision);
          if (!current()) return;
          toast(`已添加「${item.name}」，云端词库新增或更新 ${result.imported} 个词条`);
        }
      } else {
        const message = await addCommunityReply(client, item);
        if (!current()) return;
        toast(message);
      }
      if (!current()) return;
      setAdded((current) => new Set(current).add(item.id));
    } catch (failure) {
      if (current()) toast(resourceMessage(failure));
    } finally {
      if (addingNow.current.get(item.id) === generation) {
        addingNow.current.delete(item.id);
        setAdding((current) => {
          const next = new Set(current);
          next.delete(item.id);
          return next;
        });
      }
    }
  };
  const onPublished = async () => {
    setEditing(false);
    await load();
  };

  return {
    scope,
    search,
    items,
    more,
    busy,
    error,
    selected,
    editing,
    added,
    adding,
    onScopeChange: setScope,
    onSearchChange: setSearch,
    load,
    openDetail,
    closeDetail,
    add,
    openEditor: () => setEditing(true),
    closeEditor: () => setEditing(false),
    onPublished,
  };
}
