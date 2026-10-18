import { AccountPage } from "../account/account-page";
import { desktopDownloadUrl } from "./app-resources";
import { useAccountPageActions } from "./use-account-page-actions";
import type { SettingsPageModel, SettingsPageProps } from "./use-settings-page-model";

export interface SettingsAccountPageProps {
  model: Pick<
    SettingsPageModel,
    | "client"
    | "draft"
    | "androidPlatform"
    | "iosPlatform"
    | "harmonyPlatform"
    | "mobilePlatform"
    | "macosPlatform"
    | "accountLoginReturnPage"
    | "finishAccountLogin"
    | "openLocalDesigns"
    | "openCommunity"
    | "selectPage"
    | "setDictionaryTab"
    | "setError"
    | "availablePages"
    | "openExternalUrl"
    | "currentAppVersion"
  >;
  onReplayOnboarding?: SettingsPageProps["onReplayOnboarding"];
}

/** 账号页消费外壳已有的导航与模型，认证和页面状态继续由账号组件持有。 */
export function SettingsAccountPage({ model, onReplayOnboarding }: SettingsAccountPageProps) {
  const {
    client,
    draft,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    macosPlatform,
    accountLoginReturnPage,
    finishAccountLogin,
    openLocalDesigns,
    openCommunity,
    selectPage,
    setDictionaryTab,
    setError,
    availablePages,
    openExternalUrl,
    currentAppVersion,
  } = model;
  const accountActions = useAccountPageActions({
    hasAccountLoginReturnPage: Boolean(accountLoginReturnPage),
    finishAccountLogin,
    openLocalDesigns: client.customTouchKeyboardSkins ? openLocalDesigns : undefined,
    // Windows 的社区页只有词包与回复模板，没有皮肤画廊；它的候选窗口皮肤社区在主题页。
    openCommunity: client.communityResources ? openCommunity : undefined,
    openCloudDictionary:
      client.openCloudDictionary && !macosPlatform
        ? () => client.openCloudDictionary!()
        : undefined,
    // macOS 只从输入法菜单打开云剪贴板，由菜单提供粘贴目标的输入会话。
    openCloudClipboard:
      client.openCloudClipboard && !macosPlatform ? () => client.openCloudClipboard!() : undefined,
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    onReplayOnboarding,
    openAbout: () => selectPage("about"),
    setError,
  });
  return (
    <AccountPage
      {...accountActions}
      client={client.account}
      appIcon={client.appIcon}
      platform={
        androidPlatform ? "android" : iosPlatform ? "ios" : harmonyPlatform ? "harmony" : undefined
      }
      mobile={mobilePlatform}
      communityHasSkins={Boolean(client.communitySkins)}
      onOpenCloudDictionary={
        !mobilePlatform && client.cloudDictionary
          ? () => {
              selectPage("dictionary");
              setDictionaryTab("cloud");
            }
          : accountActions.onOpenCloudDictionary
      }
      onOpenFeedback={
        mobilePlatform && availablePages.some((item) => item.id === "feedback")
          ? () => selectPage("feedback")
          : undefined
      }
      preferences={draft}
      onOpenPage={selectPage}
      appTheme={client.appTheme}
      onOpenUrl={
        client.openExternalUrl
          ? (url: string) => {
              void openExternalUrl(url);
            }
          : undefined
      }
      copyText={client.copyText}
      desktopDownloadUrl={desktopDownloadUrl}
      appVersion={currentAppVersion}
    />
  );
}
