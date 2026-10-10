import type { ReactNode } from "react";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import {
  CommunityHomePage,
  type CommunityLocalDictionaryClient,
  type CommunityResourceClient,
  type CommunityResourceKind,
  type CommunityResourceScope,
} from "./community-resources";
import { CommunitySkinsPage, type CommunitySkinClient } from "./community-skins";
import {
  CommunityCandidateSkinsPage,
  type CandidateSkinCommunityClient,
} from "./community-candidate-skins";
import { useCandidateSkinSync } from "./candidate-skin-sync";
import type { SkinCatalog } from "../skin/external-skins";
import { notifySkinCatalogChanged } from "../skin/skin-catalog-changes";
import type { SkinImageReader } from "../skin/skin-image";
import type { Preferences } from "../index";

export interface CommunityPageProps {
  skins?: CommunitySkinClient;
  resources?: CommunityResourceClient;
  /** Desktop community commands for candidate-window skin packages. */
  candidateSkins?: CandidateSkinCommunityClient;
  /** The installed external skins, which the candidate gallery publishes from and checks before replacing one. */
  localSkins?: () => Promise<SkinCatalog>;
  openSkinDirectory?: () => Promise<void>;
  /** Reads an installed package's images, so publishing can draw a preview for a package without one. */
  readSkinImage?: SkinImageReader;
  /** Returns to 我的皮肤 on 主题, where an installed candidate-window skin is enabled. */
  onOpenSkinPage?: () => void;
  theme: "light" | "dark";
  initialMine?: boolean;
  initialCategory?: "skin" | CommunityResourceKind;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  localSkinLibrary?: CustomSkinLibraryClient;
  mobile?: boolean;
  onLogin?: () => void;
  /** Resets the stateful gallery when the account destination changes. */
  destinationKey?: string;
  /** HarmonyOS 手机的「社区」标签页，绘制重新设计的图库：胶囊分段、带「获取 / 使用」胶囊的双列皮肤卡片，以及带「添加」胶囊的行。其他宿主不设置它，图库保持原样。 */
  look?: "harmony";
  /** 正在编辑的偏好设置；皮肤图库读取它们，把键盘上正在用的设计标为「使用中」。 */
  preferences?: Preferences;
  /** 应用一项偏好设置变更。皮肤图库的「使用」通过它把库里的设计用到键盘上，与皮肤页选中已保存设计的方式相同。 */
  onApplyPreferences?: (next: Preferences) => void | Promise<void>;
  /** 「使用」刚应用某个皮肤后，以该皮肤的社区 id 调用，供宿主统计皮肤更换。 */
  onSkinApplied?: (id: string) => void;
}

/** The community page: the gallery the host supports, keeping the external skin directory in step with the signed-in user's library in the background. The directory itself is listed on 主题, whose 社区皮肤 tab is where desktop hosts mount this page for candidate-window skins; plugin packs are shared from the 插件 page. */
export function CommunityPage(props: CommunityPageProps): ReactNode {
  const { skins, resources, candidateSkins } = props;
  // 同步下载或删除了本机皮肤时通知主题页重扫目录。`onInstalled` 只负责把新装的皮肤同步到云端皮肤库，装入时的重扫通知由图库自己发出。
  const sync = useCandidateSkinSync(candidateSkins, notifySkinCatalogChanged);
  if (!skins && !resources && !candidateSkins) return null;
  return <CommunityGallery {...props} onInstalled={sync.run} />;
}

/** Selects the community surface supported by the host while preserving its destination state. */
function CommunityGallery({
  skins,
  resources,
  candidateSkins,
  localSkins,
  openSkinDirectory,
  readSkinImage,
  onOpenSkinPage,
  theme,
  initialMine = false,
  initialCategory = "skin",
  initialScope = "",
  localDictionary,
  localSkinLibrary,
  mobile = false,
  onLogin,
  destinationKey,
  look,
  preferences,
  onApplyPreferences,
  onSkinApplied,
  onInstalled,
}: CommunityPageProps & { onInstalled: () => void }): ReactNode {
  if (skins && resources) {
    return (
      <CommunityHomePage
        key={destinationKey}
        skins={skins}
        resources={resources}
        theme={theme}
        initialMine={initialMine}
        initialCategory={initialCategory}
        initialScope={initialScope}
        localDictionary={localDictionary}
        localSkinLibrary={localSkinLibrary}
        mobile={mobile}
        onLogin={onLogin}
        look={look}
        preferences={preferences}
        onApplyPreferences={onApplyPreferences}
        onSkinApplied={onSkinApplied}
      />
    );
  }

  if (skins) {
    return (
      <CommunitySkinsPage
        key={destinationKey}
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
    );
  }

  // 只有词包与回复模板、没有皮肤画廊的宿主（Windows）：同一个首页，只是没有「皮肤」分类。
  if (resources) {
    return (
      <CommunityHomePage
        key={destinationKey}
        resources={resources}
        theme={theme}
        initialCategory={initialCategory}
        initialScope={initialScope}
        localDictionary={localDictionary}
        mobile={mobile}
        onLogin={onLogin}
        look={look}
      />
    );
  }

  if (candidateSkins) {
    return (
      <CommunityCandidateSkinsPage
        key={destinationKey}
        client={candidateSkins}
        localSkins={localSkins}
        openSkinDirectory={openSkinDirectory}
        readSkinImage={readSkinImage}
        onOpenSkinPage={onOpenSkinPage}
        onInstalled={onInstalled}
        onLogin={onLogin}
      />
    );
  }

  return null;
}
