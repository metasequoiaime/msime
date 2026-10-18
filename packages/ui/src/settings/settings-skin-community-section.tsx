import { CommunityPage, type CommunityPageProps } from "../community/community-page";
import { CommunitySkinsPage } from "../community/community-skins";
import type { SettingsPageModel } from "./use-settings-page-model";
import type {
  useSettingsCommunityViews,
  UseSettingsCommunityViewsOptions,
} from "./use-settings-community-views";

export interface SettingsSkinCommunitySectionProps {
  model: Pick<
    SettingsPageModel,
    "client" | "page" | "draft" | "setDraft" | "keyboardPreviewTheme" | "openAccountLogin"
  >;
  views: Pick<
    ReturnType<typeof useSettingsCommunityViews>,
    "skinTabs" | "desktopTabs" | "phoneSkinCommunityShown" | "skinCommunityShown"
  >;
  onSkinViewChange: UseSettingsCommunityViewsOptions["onSkinViewChange"];
  onSkinApplied?: CommunityPageProps["onSkinApplied"];
}

/** 皮肤页复用外壳的分类与覆盖状态，社区图库继续位于偏好表单之外。 */
export function SettingsSkinCommunitySection({
  model,
  views,
  onSkinViewChange,
  onSkinApplied,
}: SettingsSkinCommunitySectionProps) {
  const { client, page, draft, setDraft, keyboardPreviewTheme, openAccountLogin } = model;
  const {
    skinTabs,
    desktopTabs: desktopSkinTabs,
    phoneSkinCommunityShown,
    skinCommunityShown,
  } = views;
  return (
    <>
      {page === "skin" && desktopSkinTabs.tabList}
      {page === "skin" && skinTabs.tabList}
      {phoneSkinCommunityShown && client.communitySkins && (
        <div className="mt-6">
          {skinTabs.panel(
            "community",
            <CommunitySkinsPage
              client={client.communitySkins}
              theme={keyboardPreviewTheme}
              localSkinLibrary={client.customSkinLibrary}
              mobile
              onLogin={openAccountLogin}
              look="harmony"
              preferences={draft}
              // 「使用」和网格里的卡片一样先进入草稿，再由表单的自动保存写入。
              onApplyPreferences={setDraft}
              onSkinApplied={onSkinApplied}
              // 详情的历史记录留在皮肤页上，返回先关详情、再离开这一页。
              historyPage="skin"
            />,
          )}
        </div>
      )}
      {skinCommunityShown &&
        client.communityCandidateSkins &&
        desktopSkinTabs.panel(
          "community",
          <div className="mt-4">
            <CommunityPage
              candidateSkins={client.communityCandidateSkins}
              localSkins={client.scanSkinCatalog}
              openSkinDirectory={client.openSkinDirectory}
              readSkinImage={client.readSkinImage}
              onOpenSkinPage={() => onSkinViewChange("mine")}
              theme={keyboardPreviewTheme}
              onLogin={openAccountLogin}
            />
          </div>,
        )}
    </>
  );
}
