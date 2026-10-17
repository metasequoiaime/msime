import { useEffect, useRef } from "react";
import type {
  CommunityDestinationCategory,
  CommunityDestinationView,
} from "../community/community-destination";
import type { SettingsPageId } from "./settings-page-registry";
import { useMobileSettingsSubview } from "./use-mobile-settings-subview";

/** 触屏宿主上一个打开着的完整社区图库：哪一类（皮肤、词库、回复模板），以及打开时的范围。 */
export type CommunityGalleryView = CommunityDestinationView;

/** 各类社区内容所在的功能页：皮肤在「皮肤」页，词库在「词库」页，回复模板是 AI 高情商回复用的，在「AI 辅助」页。 */
export function communityGalleryPage(
  category: CommunityDestinationCategory,
): Extract<SettingsPageId, "skin" | "dictionary" | "ai"> {
  return category === "skin" ? "skin" : category === "dictionary" ? "dictionary" : "ai";
}

/** 触屏「社区」标签根页顶部的两类图库，皮肤在前。回复模板不在其中：它是 AI 高情商回复用的，仍在「AI 辅助」页上打开。 */
export const communityTabCategories = [
  { id: "skin", title: "皮肤" },
  { id: "dictionary", title: "词库" },
] as const;

function galleryView(value: unknown): CommunityGalleryView | null {
  if (!value || typeof value !== "object") return null;
  const { category, scope, initialMine } = value as Record<string, unknown>;
  if (category !== "skin" && category !== "dictionary" && category !== "reply") return null;
  if (scope !== "" && scope !== "mine" && scope !== "saved") return null;
  return { category, scope, initialMine: initialMine === true };
}

export interface CommunityGallerySubviewOptions {
  mobile: boolean;
  page: SettingsPageId;
  selectPage: (page: SettingsPageId) => void;
  /** 宿主有没有键盘皮肤图库；没有时，要打开皮肤图库的请求改为打开词库图库。 */
  hasSkins: boolean;
}

/**
 * 触屏宿主上，功能页上打开的完整社区图库：「我的」里的作品与收藏，以及回复模板。浏览整类皮肤或词库的入口去「社区」标签的对应分类，不经过这里（见设置外壳的 `openCommunityGallery`）。
 *
 * 打开时先去到它所在的功能页，再压入一条带 `communityGallery` 的历史记录，和皮肤页的 `skinSubpage` 一样：系统返回手势先关掉图库里的详情（详情的记录也留在这一页，见 `historyPage`），再关掉图库，最后才离开这一页。`popstate` 时图库的开关只取自历史记录，所以前进后退都与记录一致。离开所在的功能页时图库随之关上，下次进来不会自己打开。
 */
export function useCommunityGallerySubview({
  mobile,
  page,
  selectPage,
  hasSkins,
}: CommunityGallerySubviewOptions) {
  const {
    value: gallery,
    open: openGallery,
    close: closeGallery,
    reset: resetGallery,
  } = useMobileSettingsSubview<CommunityGalleryView>({
    mobile,
    stateKey: "communityGallery",
    decode: (state) => {
      const view = galleryView(state?.communityGallery);
      return view && state?.page === communityGalleryPage(view.category) ? view : null;
    },
    encode: (view) => ({ page: communityGalleryPage(view.category), communityGallery: view }),
    isActiveState: (state) => Boolean(state?.communityGallery),
  });
  // 页面切换时图库随之关闭；不会影响浏览器历史记录，返回手势仍按当前记录恢复。
  const previousPage = useRef(page);
  useEffect(() => {
    if (previousPage.current === page) return;
    previousPage.current = page;
    if (gallery && communityGalleryPage(gallery.category) !== page) resetGallery();
  }, [gallery, page, resetGallery]);

  const visibleGallery = mobile ? gallery : null;

  const openCommunityGallery = (requested: CommunityGalleryView) => {
    const view: CommunityGalleryView =
      requested.category === "skin" && !hasSkins
        ? { category: "dictionary", scope: requested.scope, initialMine: false }
        : requested;
    const host = communityGalleryPage(view.category);
    selectPage(host);
    openGallery(view);
  };

  const closeCommunityGallery = () => closeGallery();

  return {
    communityGallery: visibleGallery,
    openCommunityGallery,
    closeCommunityGallery,
  } as const;
}
