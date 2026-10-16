import { useState } from "react";
import { CommunitySkinsPage } from "./community-skins";
import { CommunityResourcesPage } from "./community-resources";
import { CommunityCategoryTabs } from "./community-category-tabs";
import { CommunityPageShell } from "./community-page-shell";
import type { CommunityDestinationCategory } from "./community-destination";
import type { CommunitySkinClient } from "./community-skin-model";
import type {
  CommunityLocalDictionaryClient,
  CommunityResourceClient,
  CommunityResourceScope,
} from "./community-resource-model";
import type { Preferences } from "../core/preferences-model";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import * as style from "./community-style";

export interface CommunityHomePageProps {
  /** 键盘皮肤画廊。Windows 没有（它的候选窗口皮肤社区在主题页），这时只有「词库」和「回复模板」两个分类。 */
  skins?: CommunitySkinClient;
  resources: CommunityResourceClient;
  theme: "light" | "dark";
  initialMine?: boolean;
  initialCategory?: CommunityDestinationCategory;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  /**
   * 皮肤画廊用于发布的已保存设计。
   *
   * 这里原先缺了它，而同时有皮肤和资源的宿主渲染的正是这个页面，所以恰恰在这些宿主上（HarmonyOS 和 Android）「发布我的设计」从未绘制，发布流程根本没有入口。桌面端路径直接渲染 `CommunitySkinsPage`，没有问题。
   */
  localSkinLibrary?: CustomSkinLibraryClient;
  mobile?: boolean;
  onLogin?: () => void;
  /** HarmonyOS 手机的「社区」页：在重新设计的画廊上方放一个「皮肤 | 词库 | 短语」胶囊切换控件。其他宿主不设置。 */
  look?: "harmony";
  /** 转交给皮肤画廊，用于「使用」和「使用中」。 */
  preferences?: Preferences;
  onApplyPreferences?: (next: Preferences) => void | Promise<void>;
  onSkinApplied?: (id: string) => void;
}

export function CommunityHomePage({
  skins,
  resources,
  theme,
  initialMine = false,
  initialCategory = "skin",
  initialScope = "",
  localDictionary,
  localSkinLibrary,
  mobile = false,
  onLogin,
  look,
  preferences,
  onApplyPreferences,
  onSkinApplied,
}: CommunityHomePageProps) {
  const [category, setCategory] = useState<CommunityDestinationCategory>(
    skins || initialCategory !== "skin" ? initialCategory : "dictionary",
  );
  const harmony = look === "harmony";
  const body = (
    <>
      <CommunityCategoryTabs
        category={category}
        hasSkins={!!skins}
        look={look}
        onCategoryChange={setCategory}
      />
      {category === "skin" && skins ? (
        <CommunitySkinsPage
          client={skins}
          theme={theme}
          initialMine={initialMine}
          localSkinLibrary={localSkinLibrary}
          mobile={mobile}
          onLogin={onLogin}
          look={look}
          preferences={preferences}
          onApplyPreferences={onApplyPreferences}
          onSkinApplied={onSkinApplied}
        />
      ) : (
        <CommunityResourcesPage
          client={resources}
          kind={category === "skin" ? "dictionary" : category}
          initialScope={initialScope}
          localDictionary={localDictionary}
          mobile={mobile}
          look={look}
        />
      )}
    </>
  );
  return harmony ? (
    <div className={style.harmonyPage}>{body}</div>
  ) : (
    <CommunityPageShell>{body}</CommunityPageShell>
  );
}
