import { useMemo, useState } from "react";
import { errorCode } from "../core/error-code";
import type { SkinCatalog } from "../skin/external-skins";
import { notifySkinCatalogChanged } from "../skin/skin-catalog-changes";
import { useCandidateSkinSync } from "./candidate-skin-sync";
import { candidateSkinMessage, communityNeedsSignIn } from "./community-helpers";
import {
  createCommunityGalleryClient,
  useCommunityGallery,
  type CommunityGalleryClient,
} from "./community-gallery";
import { useCommunityInstallState } from "./community-install-state";
import { communityInstallPreflight } from "./community-install-preflight";
import { useCommunitySkinCategoryFilter } from "./use-community-skin-category-filter";
import type { CandidateSkinPreviewLoader } from "./community-candidate-skin-preview";
import {
  type CandidateSkinVisibility,
  type CandidateSkinCategory,
  candidateSkinCategoryLabels,
  type CommunityCandidateSkin,
  type CandidateSkinCommunityClient,
} from "./community-candidate-skin-model";

export interface UseCommunityCandidateSkinGalleryOptions {
  client: CandidateSkinCommunityClient;
  /** 已安装包目录，用于下载前判断是否需要确认替换；缺省时由安装接口检查。 */
  localSkins?: () => Promise<SkinCatalog>;
}

const CANDIDATE_SKIN_PREVIEW_CACHE_CAPACITY = 16;

/** 候选窗口皮肤图库的请求、预览缓存、安装、作者操作与发布刷新；页面只负责展示。 */
export function useCommunityCandidateSkinGallery({
  client,
  localSkins,
}: UseCommunityCandidateSkinGalleryOptions) {
  const sync = useCandidateSkinSync(client, notifySkinCatalogChanged);
  const categoryFilter = useCommunitySkinCategoryFilter();
  const categoryRequest = categoryFilter.request;
  const galleryClient = useMemo<CommunityGalleryClient<CommunityCandidateSkin>>(
    () =>
      createCommunityGalleryClient(
        client,
        async (offset, search, mine) => {
          const page = await client.list(offset, search, mine ?? false, categoryRequest.current);
          return { items: page.skins, has_more: page.has_more };
        },
        (id) => client.unpublish(id),
      ),
    [client, categoryRequest],
  );
  const gallery = useCommunityGallery({
    client: galleryClient,
    errorMessage: candidateSkinMessage,
    needsSignIn: communityNeedsSignIn,
  });
  const {
    items: skins,
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
    updateSelected,
    unpublishSelected,
    reportSelected,
    runAction,
  } = gallery;
  const [search, setSearch] = useState("");
  const [publishOpen, setPublishOpen] = useState(false);
  const { installed, setInstalled, confirmReplace, setConfirmReplace, closeDetail } =
    useCommunityInstallState(actionBusy, closeGalleryDetail);
  const changeCategory = (next: CandidateSkinCategory | null) =>
    categoryFilter.change(next, () => requestList(activeSearch, false));

  // 每个客户端的卡片和详情共用同一个包的预览；更换客户端后不沿用旧缓存。
  const loadPreview = useMemo<CandidateSkinPreviewLoader>(() => {
    const cache = new Map<string, Promise<string>>();
    return (id) => {
      const cached = cache.get(id);
      if (cached) {
        cache.delete(id);
        cache.set(id, cached);
        return cached;
      }
      const pending = client.preview(id).then((value) => value.dataUrl);
      pending.catch(() => {
        if (cache.get(id) === pending) cache.delete(id);
      });
      cache.set(id, pending);
      while (cache.size > CANDIDATE_SKIN_PREVIEW_CACHE_CAPACITY) {
        const oldest = cache.keys().next().value;
        if (oldest === undefined) break;
        cache.delete(oldest);
      }
      return pending;
    };
  }, [client]);

  const install = async (replace: boolean) => {
    if (!selected) return;
    const target = selected;
    setInstalled(false);
    await runAction(
      async (currentClient) => {
        // 下载前先确认，避免传输用户即将拒绝的约 3 MB 包；扫描失败时仍由安装接口的 `candidate_skin_exists` 阻止覆盖。
        if (!replace && localSkins) {
          const preflight = await communityInstallPreflight(
            localSkins,
            () => gallery.isCurrent(currentClient),
            (catalog) => catalog.packages.some((item) => item.id === target.package_id),
          );
          if (preflight === "stale") return;
          if (preflight === "conflict") {
            setConfirmReplace(true);
            return;
          }
        }
        await client.install(target.id, replace);
        // 皮肤已经写进目录，即使图库随后换了客户端，主题页也该重扫。
        notifySkinCatalogChanged();
        if (!gallery.isCurrent(currentClient)) return;
        setConfirmReplace(false);
        setInstalled(true);
      },
      {
        clearNotice: true,
        ignoreError: (actionError) => errorCode(actionError) === "candidate_skin_exists",
        onError: (actionError) => {
          if (errorCode(actionError) === "candidate_skin_exists") setConfirmReplace(true);
        },
      },
    );
  };

  const changeVisibility = async (visibility: CandidateSkinVisibility) => {
    await updateSelected(
      (id) => client.setVisibility(id, visibility),
      visibility === "public"
        ? "已公开，其他用户现在可以下载这款皮肤。"
        : "已设为私有，只有你能看到这款皮肤。",
    );
  };

  const changeOwnCategory = async (next: CandidateSkinCategory) => {
    await updateSelected(
      (id) => client.setCategory(id, next),
      `已改为「${candidateSkinCategoryLabels[next]}」分类。`,
    );
  };

  const unpublish = () => {
    void unpublishSelected(
      "已下架这款皮肤；其他用户将无法再下载。本地皮肤会保留，并在下次同步时作为私有皮肤存回你的皮肤库。",
    );
  };

  const publishDone = async (published: CommunityCandidateSkin) => {
    setPublishOpen(false);
    setActionNotice(
      published.visibility === "private" ? "已保存到你的皮肤库，仅自己可见。" : "已发布到社区。",
    );
    await requestList(activeSearch, false);
  };

  return {
    sync,
    skins,
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
    reportSelected,
    search,
    setSearch,
    publishOpen,
    setPublishOpen,
    installed,
    confirmReplace,
    setConfirmReplace,
    categoryFilter,
    changeCategory,
    loadPreview,
    install,
    changeVisibility,
    changeOwnCategory,
    unpublish,
    publishDone,
  };
}

export type CommunityCandidateSkinGallery = ReturnType<typeof useCommunityCandidateSkinGallery>;
