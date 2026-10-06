import type { ReactNode } from "react";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import {
  CommunityHomePage,
  CommunityResourcesPage,
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
      />
    );
  }

  if (resources) {
    return (
      <CommunityResourcesPage
        client={resources}
        kind={initialCategory === "reply" ? "reply" : "dictionary"}
        initialScope={initialScope}
        mobile={mobile}
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
