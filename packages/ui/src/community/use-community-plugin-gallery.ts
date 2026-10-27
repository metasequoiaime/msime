import { useMemo, useRef, useState } from "react";
import type { PluginCatalogResult, PluginPackage } from "../settings/plugin-types";
import { communityNeedsSignIn, communityPluginMessage } from "./community-helpers";
import {
  createCommunityGalleryClient,
  useCommunityGallery,
  type CommunityGalleryClient,
} from "./community-gallery";
import { useCommunityInstallState } from "./community-install-state";
import { useCommunityDetailHistory } from "./use-community-detail-history";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
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
  /** 装好一个脚本插件后打开它在「我的插件」里的授权页：脚本插件装好后不会运行，要用户启用并授权。没有时详情页只提示去哪里授权。 */
  onOpenScript?: (installed: PluginPackage) => void;
  /** 只列这几类（触屏插件页，见 `touchPluginKinds`）：服务端给的其他类型由这里滤掉，和宿主滤掉的新类型一样计入下一页的偏移量。缺省时不滤。 */
  kinds?: readonly CommunityPluginKind[];
  /** 触屏设置：打开详情推入一条历史记录，系统返回键关掉详情。 */
  mobile?: boolean;
}

/** 插件图库的请求、筛选、安装确认与发布刷新；页面只负责展示。 */
export function useCommunityPluginGallery({
  client,
  localPlugins,
  onInstalled,
  onOpenScript,
  kinds,
  mobile = false,
}: UseCommunityPluginGalleryOptions) {
  // 图库按偏移量和搜索词分页；类型保存在 ref 中，让「加载更多」沿用首屏的类型。
  const kindFilter = useRef<CommunityPluginKind | null>(null);
  // 只按内容比较，调用方每次渲染给一个新数组时不重建图库客户端。
  const allowedKinds = kinds?.join(",") ?? null;
  const galleryClient = useMemo<CommunityGalleryClient<CommunityPlugin>>(
    () =>
      createCommunityGalleryClient(
        client,
        async (offset, search, mine) => {
          const page = await client.list(offset, search, kindFilter.current, mine ?? false);
          const allowed = allowedKinds?.split(",");
          const items = allowed
            ? page.plugins.filter((plugin) => allowed.includes(plugin.kind))
            : page.plugins;
          return {
            items,
            has_more: page.has_more,
            skipped: (page.skipped ?? 0) + page.plugins.length - items.length,
          };
        },
        (id) => client.delete(id),
      ),
    [client, allowedKinds],
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
  const {
    installed,
    setInstalled,
    confirmReplace,
    setConfirmReplace,
    closeDetail: closeInstallDetail,
  } = useCommunityInstallState(actionBusy, closeGalleryDetail);
  // 触屏设置里详情有自己的历史记录：标题栏的返回交给系统返回，由 popstate 关掉详情。
  const openDetail = (plugin: CommunityPlugin) => {
    if (mobile && typeof window !== "undefined") {
      pushMobileSettingsState({
        page: "plugins",
        communityDetail: { kind: "plugin", id: plugin.id },
      });
    }
    void open(plugin);
  };
  const closeDetail = () => {
    // 安装、评分进行中不关，与 `useCommunityInstallState` 一致；先查这一条，免得退掉了历史记录详情却还开着。
    if (actionBusy) return;
    if (
      mobile &&
      typeof window !== "undefined" &&
      window.history.state?.communityDetail?.kind === "plugin"
    ) {
      window.history.back();
      return;
    }
    closeInstallDetail();
  };
  useCommunityDetailHistory({
    mobile,
    kind: "plugin",
    selectedId: selected?.id ?? null,
    onClose: closeInstallDetail,
  });

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
        if (pack.kind === "script") onOpenScript?.(pack);
      },
      { clearNotice: true },
    );
  };

  const publishDone = async (published?: CommunityPlugin) => {
    setPublishOpen(false);
    setActionNotice(
      published?.kind === "script"
        ? "已提交审核。脚本插件要经过管理员审核，通过后才会出现在社区里；审核前只有你自己在「我的作品」里看得到。"
        : "已发布到社区。",
    );
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
    open: openDetail,
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
