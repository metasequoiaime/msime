import { useEffect, useMemo, useRef, useState } from "react";
import { useToast } from "../core/toast";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { communityNeedsSignIn, communitySkinMessage } from "./community-helpers";
import {
  createCommunityGalleryClient,
  useCommunityGallery,
  type CommunityGalleryClient,
} from "./community-gallery";
import { useCommunityDetailHistory } from "./use-community-detail-history";
import { useCommunitySkinCategoryFilter } from "./use-community-skin-category-filter";
import {
  communitySkinCategoryLabels,
  type CommunitySkinCategory,
} from "./community-skin-category-model";
import type {
  CommunitySkin,
  CommunitySkinClient,
  CommunitySkinTrial,
} from "./community-skin-model";

export interface UseCommunitySkinGalleryOptions {
  client: CommunitySkinClient;
  initialMine?: boolean;
  mobile?: boolean;
  historyPage?: string;
  harmony?: boolean;
}

/** 社区皮肤图库的请求、详情历史、试用恢复、分类切换和发布刷新状态；页面只负责显示。 */
export function useCommunitySkinGallery({
  client,
  initialMine = false,
  mobile = false,
  historyPage = "community",
  harmony = false,
}: UseCommunitySkinGalleryOptions) {
  const categoryFilter = useCommunitySkinCategoryFilter();
  const categoryRequest = categoryFilter.request;
  const galleryClient = useMemo<CommunityGalleryClient<CommunitySkin>>(
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
    initialMine,
    errorMessage: communitySkinMessage,
    needsSignIn: communityNeedsSignIn,
  });
  const {
    items,
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
    setSelected,
    setActionNotice,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open: openGallery,
    closeDetail: closeGalleryDetail,
    rateSelected,
    updateSelected,
    unpublishSelected,
    reportSelected,
    runAction,
  } = gallery;
  const [search, setSearch] = useState("");
  const [trial, setTrial] = useState<CommunitySkinTrial | null>(null);
  const [publishOpen, setPublishOpen] = useState(false);
  const trialRef = useRef<CommunitySkinTrial | null>(null);
  useEffect(() => {
    return () => {
      const pending = trialRef.current;
      trialRef.current = null;
      setTrial(null);
      if (pending) void client.finishTrial(pending.id, false).catch(() => undefined);
    };
  }, [client]);

  const toast = useToast();

  const closeDetail = async (fromHistory = false) => {
    if (actionBusy) return;
    if (trial) {
      let finished = false;
      await runAction(async (generation) => {
        await client.finishTrial(trial.id, false);
        if (!gallery.isCurrent(generation)) return;
        trialRef.current = null;
        setTrial(null);
        finished = true;
      });
      if (!finished) return;
    }
    closeGalleryDetail();
    if (
      !fromHistory &&
      mobile &&
      typeof window !== "undefined" &&
      window.history.state?.communityDetail?.kind === "skin"
    ) {
      window.history.back();
    }
  };

  const open = (skin: CommunitySkin) => {
    if (mobile && typeof window !== "undefined") {
      pushMobileSettingsState({
        page: historyPage,
        communityDetail: { kind: "skin", id: skin.id },
      });
    }
    openGallery(skin);
  };

  useCommunityDetailHistory({
    mobile,
    kind: "skin",
    selectedId: selected?.id ?? null,
    onClose: () => void closeDetail(true),
  });

  const download = async () => {
    if (!selected) return;
    await runAction(
      async (currentClient) => {
        const result = await client.download(selected.id, selected.name);
        if (!gallery.isCurrent(currentClient)) return;
        setSelected((current) => (current ? { ...current, design: result.skin.design } : current));
        trialRef.current = result.trial;
        setTrial(result.trial);
        setActionNotice("已下载并开始试用；关闭此页会恢复原皮肤。");
      },
      { clearNotice: true },
    );
  };

  const finishTrial = async (keep: boolean) => {
    if (!trial) return;
    const pending = trial;
    await runAction(async (currentClient) => {
      await client.finishTrial(pending.id, keep);
      if (!gallery.isCurrent(currentClient)) return;
      trialRef.current = null;
      setTrial(null);
      setActionNotice(keep ? "已保留这款皮肤。" : "已恢复试用前的皮肤。");
    });
  };

  const changeCategory = (next: CommunitySkinCategory | null) =>
    categoryFilter.change(next, () => requestList(activeSearch, false));

  const changeOwnCategory = async (next: CommunitySkinCategory) => {
    await updateSelected(
      (id) => client.setCategory(id, next),
      `已改为「${communitySkinCategoryLabels[next]}」分类。`,
    );
  };

  const unpublish = () => {
    if (trial) return;
    void unpublishSelected("已下架这款皮肤；其他用户将无法再下载，已有本地副本不会受影响。");
  };

  const publishDone = async () => {
    setPublishOpen(false);
    setActionNotice("已发布到社区。");
    if (harmony) toast("已发布到社区。");
    await requestList(activeSearch, false);
  };

  return {
    items,
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
    search,
    setSearch,
    trial,
    publishOpen,
    setPublishOpen,
    categoryFilter,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    updateSelected,
    unpublish,
    unpublishSelected,
    reportSelected,
    runAction,
    download,
    finishTrial,
    changeCategory,
    changeOwnCategory,
    publishDone,
    setActionNotice,
  };
}

export type CommunitySkinGallery = ReturnType<typeof useCommunitySkinGallery>;
