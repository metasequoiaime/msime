import { useLayoutEffect, useRef } from "react";
import {
  CommunityResourcesPage,
  type CommunityLocalDictionaryClient,
  type CommunityResourceClient,
} from "../community/community-resources";
import { CommunitySkinsPage, type CommunitySkinClient } from "../community/community-skins";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import { fullScreenColumn, fullScreenSurface } from "../core/full-screen-style";
import type { Preferences } from "../index";
import { SettingsPageHeader } from "./settings-page-header";
import { communityGalleryPage, type CommunityGalleryView } from "./use-community-gallery-subview";

const titles = {
  skin: "社区皮肤",
  dictionary: "社区词库",
  reply: "社区回复模板",
} as const satisfies Record<CommunityGalleryView["category"], string>;

/**
 * 触屏宿主在功能页里打开的完整社区图库：「皮肤」页的「浏览更多皮肤」、「词库」页的「浏览更多词库」、「AI 辅助」页的「浏览更多回复模板」，以及「我的」里社区作品各行。和 AI 皮肤抽卡一样以页面背景覆盖整个设置界面，标题前带返回按钮；搜索、分类、详情、评分、举报、试用、发布、我的作品和收藏都在图库里。
 *
 * 详情的历史记录留在所在的功能页上（`historyPage`），返回先关详情、再关图库。
 */
export function CommunityGallerySubview({
  view,
  skins,
  resources,
  theme,
  localDictionary,
  localSkinLibrary,
  onLogin,
  preferences,
  onApplyPreferences,
  onSkinApplied,
  look,
  onClose,
}: {
  view: CommunityGalleryView;
  skins?: CommunitySkinClient;
  resources?: CommunityResourceClient;
  theme: "light" | "dark";
  localDictionary?: CommunityLocalDictionaryClient;
  localSkinLibrary?: CustomSkinLibraryClient;
  onLogin?: () => void;
  preferences?: Preferences;
  onApplyPreferences?: (next: Preferences) => void | Promise<void>;
  onSkinApplied?: (id: string) => void;
  look?: "harmony";
  onClose: () => void;
}) {
  const title = titles[view.category];
  const historyPage = communityGalleryPage(view.category);
  const column = useRef<HTMLElement>(null);
  // 和 AI 皮肤抽卡的全屏视图一样，打开时让标题和返回按钮出现在屏幕顶部，而不是停在功能页原来的滚动位置。
  useLayoutEffect(() => {
    column.current?.scrollIntoView?.({ block: "start" });
  }, []);
  const gallery =
    view.category === "skin"
      ? skins && (
          <CommunitySkinsPage
            client={skins}
            theme={theme}
            initialMine={view.initialMine}
            localSkinLibrary={localSkinLibrary}
            mobile
            onLogin={onLogin}
            look={look}
            preferences={preferences}
            onApplyPreferences={onApplyPreferences}
            onSkinApplied={onSkinApplied}
            historyPage={historyPage}
          />
        )
      : resources && (
          <CommunityResourcesPage
            client={resources}
            kind={view.category}
            initialScope={view.scope}
            localDictionary={localDictionary}
            mobile
            look={look}
            historyPage={historyPage}
          />
        );
  return (
    <div className={fullScreenSurface} role="dialog" aria-modal="true" aria-label={title}>
      <section ref={column} className={fullScreenColumn}>
        <SettingsPageHeader title={title} onBack={onClose} level={2} />
        {gallery}
      </section>
    </div>
  );
}
