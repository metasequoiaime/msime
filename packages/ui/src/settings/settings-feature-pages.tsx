import { HomePage } from "../keyboard/home-page";
import { TryKeyboardPage } from "../keyboard/try-keyboard-page";
import { ChatPage } from "../chat/chat-page";
import { SettingsAccountPage, type SettingsAccountPageProps } from "./settings-account-page";
import { SettingsCommunityPage, type SettingsCommunityPageProps } from "./settings-community-page";
import { communityGalleryPage } from "./use-community-gallery-subview";
import { CommunityGallerySubview } from "./community-gallery-subview";
import { TypingStatisticsPage } from "./typing-statistics";
import { VocabularyReviewPage } from "./vocabulary-review";
import { touchKeyLayoutStyle } from "../keyboard/touch-key-layout-styles";
import type { SettingsPageModel } from "./use-settings-page-model";
import type { useSettingsShellState } from "./use-settings-shell-state";

export interface SettingsFeaturePagesProps {
  model: Pick<
    SettingsPageModel,
    | keyof SettingsAccountPageProps["model"]
    | keyof SettingsCommunityPageProps["model"]
    | "page"
    | "settingsPlatform"
    | "surfaceThemesFollow"
    | "showAiGeneration"
    | "showCommunityContribution"
    | "mobileSecondaryGroups"
    | "communityGallery"
    | "closeCommunityGallery"
  >;
  chatState: Pick<
    ReturnType<typeof useSettingsShellState>,
    "chatMounted" | "chatFocusRequest" | "setChatFocusRequest"
  >;
  onOpenPage: Parameters<typeof HomePage>[0]["onOpenPage"];
  onOpenChat?: Parameters<typeof HomePage>[0]["onOpenChat"];
  onReplayOnboarding?: SettingsAccountPageProps["onReplayOnboarding"];
  localDictionary?: SettingsCommunityPageProps["localDictionary"];
  onSkinApplied?: SettingsCommunityPageProps["onSkinApplied"];
}

/** 组合偏好表单之外的功能页，模型与聊天生命周期继续由外壳持有。 */
export function SettingsFeaturePages({
  model,
  chatState,
  onOpenPage,
  onOpenChat,
  onReplayOnboarding,
  localDictionary,
  onSkinApplied,
}: SettingsFeaturePagesProps) {
  const {
    client,
    draft,
    setDraft,
    page,
    mobilePlatform,
    iosPlatform,
    harmonyPlatform,
    settingsPlatform,
    surfaceThemesFollow,
    showAiGeneration,
    showCommunityContribution,
    mobileSecondaryGroups,
    openAccountLogin,
    openExternalUrl,
    communityGallery,
    closeCommunityGallery,
    keyboardPreviewTheme,
    availablePages,
  } = model;
  const { chatMounted, chatFocusRequest, setChatFocusRequest } = chatState;
  return (
    <>
      {client.home && draft && page === "home" && (
        <HomePage
          preferences={draft}
          actions={client.home}
          onOpenPage={onOpenPage}
          onOpenChat={
            onOpenChat &&
            (() => {
              setChatFocusRequest((count) => count + 1);
              onOpenChat();
            })
          }
          touchLayout={mobilePlatform}
          ios={iosPlatform}
          keyboardSurfaceTheme={surfaceThemesFollow ? undefined : draft.screen_keyboard_theme}
          rootPages={mobileSecondaryGroups
            .flatMap((group) => group.pages)
            .map((item) => ({ id: item.id, title: item.title }))}
        />
      )}
      {client.home && page === "try-keyboard" && (
        <TryKeyboardPage
          actions={client.home}
          switchAway={!harmonyPlatform}
          chat={showAiGeneration ? client.chat : undefined}
          onLogin={openAccountLogin}
          onOpenUrl={
            client.openExternalUrl
              ? (url: string) => {
                  void openExternalUrl(url);
                }
              : undefined
          }
        />
      )}
      {/* 与导航用同一个判断（`settingsPageProjections` 的 `hasAccount`）：鸿蒙手机拒绝隐私授权后没有账号客户端，「我的」仍要能打开。 */}
      {page === "account" && availablePages.some((item) => item.id === "account") && (
        <SettingsAccountPage model={model} onReplayOnboarding={onReplayOnboarding} />
      )}
      {/* 触屏上 AI 对话从「AI 辅助」二级页进入：第一次打开后一直挂着，切换标签只是隐藏，对话记录和进行中的回复都不丢。 */}
      {client.chat && showAiGeneration && (page === "chat" || (mobilePlatform && chatMounted)) && (
        <div className={page === "chat" ? "contents" : undefined} hidden={page !== "chat"}>
          <ChatPage
            client={client.chat}
            touch={mobilePlatform}
            active={page === "chat"}
            focusRequest={chatFocusRequest}
            setup={client.home}
            onLogin={openAccountLogin}
          />
        </div>
      )}
      {page === "community" && (
        <SettingsCommunityPage
          model={model}
          localDictionary={localDictionary}
          onSkinApplied={onSkinApplied}
        />
      )}
      {/* 触屏宿主在功能页里打开的完整社区图库。画在设置表单之外：图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交。 */}
      {communityGallery && page === communityGalleryPage(communityGallery.category) && (
        <CommunityGallerySubview
          key={`${communityGallery.category}:${communityGallery.scope}:${communityGallery.initialMine}`}
          view={communityGallery}
          skins={client.communitySkins}
          resources={client.communityResources}
          theme={keyboardPreviewTheme}
          localDictionary={localDictionary}
          localSkinLibrary={client.customSkinLibrary}
          onLogin={openAccountLogin}
          preferences={draft}
          onApplyPreferences={setDraft}
          onSkinApplied={onSkinApplied}
          look={harmonyPlatform && mobilePlatform ? "harmony" : undefined}
          contributions={showCommunityContribution}
          onClose={closeCommunityGallery}
        />
      )}
      {client.typingStatistics && page === "typing-statistics" && (
        <TypingStatisticsPage
          client={client.typingStatistics}
          mobile={mobilePlatform}
          platform={client.host?.platform}
          openSystemSettings={client.openSystemKeyboardSettings}
          aiGeneration={showAiGeneration}
          look={
            settingsPlatform === "harmony"
              ? "harmony"
              : settingsPlatform === "hm2"
                ? "hm2"
                : undefined
          }
          // 只有 HarmonyOS 手机的键盘读按键布局样式；2in1 画的是整排键盘，热力图照旧。
          layoutStyle={
            harmonyPlatform && mobilePlatform && draft ? touchKeyLayoutStyle(draft) : undefined
          }
        />
      )}
      {client.vocabularyReview && page === "vocabulary" && (
        <VocabularyReviewPage
          client={client.vocabularyReview}
          mobile={mobilePlatform}
          openPanel={client.openVocabulary}
          saveExport={client.saveExport}
        />
      )}
    </>
  );
}
