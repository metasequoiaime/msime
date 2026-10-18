import { CommunityPage, type CommunityPageProps } from "../community/community-page";
import { CommunitySkinsPage } from "../community/community-skins";
import { CommunityResourcesPage } from "../community/community-resources";
import type { SettingsPageModel } from "./use-settings-page-model";

export interface SettingsCommunityPageProps {
  model: Pick<
    SettingsPageModel,
    | "client"
    | "draft"
    | "setDraft"
    | "communityDestination"
    | "initialCommunityMine"
    | "initialCommunityCategory"
    | "initialCommunityScope"
    | "keyboardPreviewTheme"
    | "mobilePlatform"
    | "harmonyPlatform"
    | "openAccountLogin"
    | "communityTabs"
  >;
  localDictionary?: CommunityPageProps["localDictionary"];
  onSkinApplied?: CommunityPageProps["onSkinApplied"];
}

/** 社区根页复用外壳已有分类与草稿，图库保持在偏好表单之外。 */
export function SettingsCommunityPage({
  model,
  localDictionary,
  onSkinApplied,
}: SettingsCommunityPageProps) {
  const {
    client,
    draft,
    setDraft,
    communityDestination,
    initialCommunityMine,
    initialCommunityCategory,
    initialCommunityScope,
    keyboardPreviewTheme,
    mobilePlatform,
    harmonyPlatform,
    openAccountLogin,
    communityTabs,
  } = model;
  return (
    <>
      {!mobilePlatform && (
        <CommunityPage
          destinationKey={communityDestination}
          skins={client.communitySkins}
          resources={client.communityResources}
          theme={keyboardPreviewTheme}
          initialMine={initialCommunityMine}
          initialCategory={initialCommunityCategory}
          initialScope={initialCommunityScope}
          localDictionary={localDictionary}
          localSkinLibrary={client.customSkinLibrary}
          mobile={mobilePlatform}
          onLogin={openAccountLogin}
          preferences={draft}
          // 从皮肤库应用的皮肤和其他偏好修改一样：先进入草稿，再由表单的自动保存写入。
          onApplyPreferences={setDraft}
          onSkinApplied={onSkinApplied}
          look={harmonyPlatform && mobilePlatform ? "harmony" : undefined}
        />
      )}
      {/* 触屏的「社区」标签根页：顶部「皮肤 / 词库」分类，每类是完整的社区图库（搜索、分类、详情、获取或添加、举报、发布都在里面），只挂载选中的那一类。和皮肤页的「社区」分类一样画在设置表单之外：图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交。详情的历史记录留在这一页，返回先关详情。 */}
      {mobilePlatform && communityTabs.tabList}
      {mobilePlatform && (
        <div className={communityTabs.tabList ? "mt-6" : undefined}>
          {communityTabs.selected === "skin" &&
            client.communitySkins &&
            communityTabs.panel(
              "skin",
              <CommunitySkinsPage
                client={client.communitySkins}
                theme={keyboardPreviewTheme}
                localSkinLibrary={client.customSkinLibrary}
                mobile
                onLogin={openAccountLogin}
                look={harmonyPlatform ? "harmony" : undefined}
                preferences={draft}
                // 「使用」和皮肤页的网格一样先进入草稿，再由表单的自动保存写入。
                onApplyPreferences={setDraft}
                onSkinApplied={onSkinApplied}
                historyPage="community"
              />,
            )}
          {communityTabs.selected === "dictionary" &&
            client.communityResources &&
            communityTabs.panel(
              "dictionary",
              <CommunityResourcesPage
                client={client.communityResources}
                kind="dictionary"
                localDictionary={localDictionary}
                mobile
                look={harmonyPlatform ? "harmony" : undefined}
                historyPage="community"
              />,
            )}
        </div>
      )}
    </>
  );
}
