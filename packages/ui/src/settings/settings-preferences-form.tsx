import { SettingsSearchPage } from "./settings-page-fieldset";
import { canReloadSettingsPage, canRestoreDefaultsOnPage } from "./settings-page-visibility";
import { SettingsFormFooter } from "./settings-form-footer";
import { SettingsFormFrame } from "./settings-form-frame";
import { SettingsFormContext } from "./settings-form-context";
import { VoiceSettingsPage } from "./pages/voice-page";
import { AiSettingsPage } from "./pages/ai-page";
import { InputSettingsPage } from "./pages/input-page";
import { ExpressionSettingsPage } from "./pages/expression-page";
import { DeveloperSettingsPage } from "./pages/developer-page";
import { McpSettingsPage } from "./pages/mcp-page";
import { DictionarySettingsPage } from "./pages/dictionary-page";
import { AppearanceSettingsPage } from "./pages/appearance-page";
import { SkinSettingsPage } from "./pages/skin-page";
import { FloatingToolbarSettingsPage } from "./pages/floating-toolbar-page";
import { ScreenKeyboardSettingsPage } from "./pages/screen-keyboard-page";
import { ShortcutSettingsPage } from "./pages/shortcuts-page";
import { ToolsSettingsPage } from "./pages/tools-page";
import { PluginsSettingsPage } from "./pages/plugins-page";
import { AboutSettingsPage } from "./pages/about-page";
import { UsageReportingSettingsPage } from "./pages/usage-reporting-page";
import { PrivacySettingsPage } from "./pages/privacy-page";
import { HelpSettingsPage } from "./help-settings-page";
import { HandwritingSettingsPage } from "./pages/handwriting-page";
import { FeedbackSettingsPage } from "./pages/feedback-page";
import type { SettingsFormModel } from "./settings-form-context";
import type { useSettingsCommunityViews } from "./use-settings-community-views";
import type { createSettingsExternalActions } from "./settings-external-actions";

type CommunityViews = ReturnType<typeof useSettingsCommunityViews>;

export interface SettingsPreferencesFormProps {
  model: SettingsFormModel;
  skinTabs: CommunityViews["skinTabs"];
  desktopSkinTabs: CommunityViews["desktopTabs"];
  skinCommunityShown: boolean;
  pluginCommunityShown: boolean;
  externalActions: Pick<
    ReturnType<typeof createSettingsExternalActions>,
    "onOpenDocumentation" | "onOpenSystemKeyboardSettings"
  >;
  onRestoreDefaults: () => void;
  reloadSettings: () => Promise<void>;
}

/** 偏好页面共用同一份已加载模型；挂载条件与图库仍由外壳负责。 */
export function SettingsPreferencesForm({
  model,
  skinTabs,
  desktopSkinTabs,
  skinCommunityShown,
  pluginCommunityShown,
  externalActions,
  onRestoreDefaults,
  reloadSettings,
}: SettingsPreferencesFormProps) {
  const {
    client,
    draft,
    busy,
    page,
    saveState,
    saveError,
    retrySave,
    loadFailed,
    error,
    mobilePlatform,
    macosPlatform,
    linuxPlatform,
    iosPlatform,
    androidPlatform,
    handwritingOnScreenKeyboardPage,
  } = model;
  return (
    <SettingsFormFrame showReload={false} busy={busy}>
      <SettingsFormContext.Provider value={model}>
        <SettingsSearchPage.Provider value="skin">
          <SkinSettingsPage
            hidden={skinCommunityShown}
            tabs={skinTabs}
            desktopTabs={desktopSkinTabs}
          />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="appearance">
          <AppearanceSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="floating-toolbar">
          <FloatingToolbarSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="input">
          <InputSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="expression">
          <ExpressionSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="ai">
          <AiSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="shortcuts">
          <ShortcutSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="dictionary">
          <DictionarySettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="screen-keyboard">
          <ScreenKeyboardSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="voice">
          <VoiceSettingsPage />
        </SettingsSearchPage.Provider>
        {/* 手写并进屏幕键盘页时，那一页自己画手写的分组。 */}
        {!handwritingOnScreenKeyboardPage && (
          <SettingsSearchPage.Provider value="handwriting">
            <HandwritingSettingsPage />
          </SettingsSearchPage.Provider>
        )}
        <SettingsSearchPage.Provider value="tools">
          <ToolsSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="plugins">
          <PluginsSettingsPage hidden={pluginCommunityShown} />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="developer">
          <DeveloperSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="mcp">
          <McpSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="feedback">
          <FeedbackSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="help">
          <HelpSettingsPage
            busy={busy}
            hidden={page !== "help"}
            macos={macosPlatform}
            linux={linuxPlatform}
            edition={client.host?.edition}
            mobile={mobilePlatform}
            ios={iosPlatform}
            android={androidPlatform}
            platformHelpIntro={model.platformHelpIntro}
            platformQuickStart={model.platformQuickStart}
            platformNetworkDescription={model.platformNetworkDescription}
            onOpenDocumentation={externalActions.onOpenDocumentation}
            onOpenSystemKeyboardSettings={externalActions.onOpenSystemKeyboardSettings}
          />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="about">
          <AboutSettingsPage />
        </SettingsSearchPage.Provider>
        <SettingsSearchPage.Provider value="usage-reporting">
          <UsageReportingSettingsPage />
        </SettingsSearchPage.Provider>
        {/* 只在打开着时渲染：它和「关于」用同一个统计开关，常驻的话桌面上会有两个。 */}
        {page === "privacy" && (
          <SettingsSearchPage.Provider value="privacy">
            <PrivacySettingsPage />
          </SettingsSearchPage.Provider>
        )}
      </SettingsFormContext.Provider>
      <SettingsFormFooter
        draft={draft}
        busy={busy}
        // 保存失败由页面顶部的提示说明并重试（SettingsPageStatus），这一行不再重复同一个错误和按钮。
        saveState={saveState === "failed" ? "idle" : saveState}
        saveError={saveError}
        showRestoreDefaults={
          Boolean(client.loadDefaultPreferences) &&
          canRestoreDefaultsOnPage(page) &&
          // macOS 的屏幕键盘页（并进手写之后）只剩两个打开按钮，没有一项偏好，页脚的全局「恢复默认设置」在那里只会让人以为它恢复这一页。
          !(
            page === "screen-keyboard" &&
            !mobilePlatform &&
            !model.showTouchKeyboardHeight &&
            !model.showTouchKeyboardSpacing
          ) &&
          !skinCommunityShown &&
          !pluginCommunityShown
        }
        onRestoreDefaults={onRestoreDefaults}
        onRetry={() => void retrySave()}
        // 设置自动保存，重新读取只用于从保存失败、加载失败或页面错误中恢复。
        onReload={
          canReloadSettingsPage(page) && (saveState === "failed" || loadFailed || !!error)
            ? () => void reloadSettings()
            : undefined
        }
      />
    </SettingsFormFrame>
  );
}
