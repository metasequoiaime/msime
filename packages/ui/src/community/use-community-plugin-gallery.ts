import { useMemo, useRef, useState } from "react";
import type { PluginCatalogResult, PluginPackage } from "../settings/plugin-types";
import { communityNeedsSignIn, communityPluginMessage } from "./community-helpers";
import {
  createCommunityGalleryClient,
  useCommunityGallery,
  type CommunityGalleryClient,
} from "./community-gallery";
import { useCommunityInstallState } from "./community-install-state";
import { communityInstallPreflight } from "./community-install-preflight";
import type {
  CommunityPlugin,
  CommunityPluginClient,
  CommunityPluginKind,
} from "./community-plugin-model";

export interface UseCommunityPluginGalleryOptions {
  client: CommunityPluginClient;
  /** 已安装包目录，供安装前判断同类型同标识的替换冲突。 */
  localPlugins?: () => Promise<PluginCatalogResult>;
  /** 当前图库的安装完成后通知调用方刷新本机目录。 */
  onInstalled?: (installed: PluginPackage) => void;
}

/** 插件图库的请求、筛选、安装确认与发布刷新；页面只负责展示。 */
export function useCommunityPluginGallery({
  client,
  localPlugins,
  onInstalled,
}: UseCommunityPluginGalleryOptions) {
  // 图库按偏移量和搜索词分页；类型保存在 ref 中，让「加载更多」沿用首屏的类型。
  const kindFilter = useRef<CommunityPluginKind | null>(null);
  const galleryClient = useMemo<CommunityGalleryClient<CommunityPlugin>>(
    () =>
      createCommunityGalleryClient(
        client,
        async (offset, search, mine) => {
          const page = await client.list(offset, search, kindFilter.current, mine ?? false);
          return { items: page.plugins, has_more: page.has_more, skipped: page.skipped };
        },
        (id) => client.delete(id),
      ),
    [client],
  );
  const gallery = useCommunityGallery({
    client: galleryClient,
    errorMessage: communityPluginMessage,
    needsSignIn: communityNeedsSignIn,
  });
  const {
    items: plugins,
    hasMore,
    listBusy,
    detailBusy,
    error,
    selected,
    actionBusy,
    actionNotice,
    mineOnly,
    signInRequired,
    confirmUnpublish,
    activeSearch,
    setActionNotice,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail: closeGalleryDetail,
    rateSelected,
    unpublishSelected,
    reportSelected,
    runAction,
  } = gallery;
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState<CommunityPluginKind | null>(null);
  const [publishOpen, setPublishOpen] = useState(false);
  const { installed, setInstalled, confirmReplace, setConfirmReplace, closeDetail } =
    useCommunityInstallState(actionBusy, closeGalleryDetail);

  const changeKind = async (next: CommunityPluginKind | null) => {
    const previous = kindFilter.current;
    if (next === previous) return;
    kindFilter.current = next;
    setKind(next);
    // 首屏失败会保留原类型的列表与偏移，因此也恢复类型，避免「加载更多」按旧偏移追加新类型。
    const listed = await requestList(activeSearch, false);
    if (!listed && kindFilter.current === next) {
      kindFilter.current = previous;
      setKind(previous);
    }
  };

  const install = async (replace: boolean) => {
    if (!selected) return;
    const target = selected;
    setInstalled(false);
    await runAction(
      async (currentClient) => {
        // 安装会整体替换同类型同标识的包，先请求确认；目录扫描失败时仍沿用本地 `.zip` 导入流程继续安装。
        if (!replace && localPlugins) {
          const preflight = await communityInstallPreflight(
            localPlugins,
            () => gallery.isCurrent(currentClient),
            (catalog) =>
              catalog.packages.some(
                (item) => item.kind === target.kind && item.id === target.plugin_id,
              ),
          );
          if (preflight === "stale") return;
          if (preflight === "conflict") {
            setConfirmReplace(true);
            return;
          }
        }
        const pack = await client.install(target.id, target.kind, target.plugin_id);
        if (!gallery.isCurrent(currentClient)) return;
        setConfirmReplace(false);
        setInstalled(true);
        onInstalled?.(pack);
      },
      { clearNotice: true },
    );
  };

  const publishDone = async () => {
    setPublishOpen(false);
    setActionNotice("已发布到社区。");
    await requestList(activeSearch, false);
  };

  return {
    plugins,
    hasMore,
    listBusy,
    detailBusy,
    error,
    selected,
    actionBusy,
    actionNotice,
    mineOnly,
    signInRequired,
    confirmUnpublish,
    activeSearch,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    unpublishSelected,
    reportSelected,
    search,
    setSearch,
    kind,
    changeKind,
    publishOpen,
    setPublishOpen,
    installed,
    confirmReplace,
    setConfirmReplace,
    install,
    publishDone,
  };
}

export type CommunityPluginGallery = ReturnType<typeof useCommunityPluginGallery>;
