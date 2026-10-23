import { useLayoutEffect, useState } from "react";
import { candidateTabs } from "../candidate-tabs";
import { useSettingsTabs } from "../use-settings-tabs";
import { AppearanceCandidatePreview } from "../../candidate/appearance-candidate-preview";
import { CandidateFontControls } from "../../candidate/candidate-font-controls";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { CandidateFollowCursorSection } from "../candidate-follow-cursor-section";
import { CandidatePageNumberSection } from "../candidate-page-number-section";
import { CandidateAppLogoSection } from "../candidate-app-logo-section";
import { CandidateExpandSection } from "../candidate-expand-section";
import { CandidateFontSizeSliderRow, CandidateSizingSection } from "../candidate-sizing-section";
import { CandidatePageSizeSection } from "../candidate-page-size-section";
import { offeredCandidatePageSizes } from "../candidate-page-size";
import { CandidateLayoutSection } from "../candidate-layout-section";
import { PreeditSettingsSection } from "../preedit-settings-section";
import { appearanceSettingsPreferences } from "../appearance-settings-preferences";
import { CandidatePanelLimitSection } from "../candidate-panel-limit-section";
import { CandidateFontUnsupportedNotice } from "../candidate-font-unsupported-notice";
import { createAppearanceSettingsActions } from "../appearance-settings-actions";
import {
  CandidateScaleRow,
  CandidateWindowStyleRows,
  CandidateWindowStyleSection,
} from "../candidate-window-style-section";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { CandidateFontSettingsRows } from "../candidate-font-settings-rows";
import { CandidateBadgeSettingsSection } from "../candidate-badge-settings-section";

/** 候选窗口按功能分区，共用预览；手机保留直列和折叠区。 */
export function AppearanceSettingsPage() {
  // 用户点了「自定义…」但字体还停在某个预设上时，靠它把字体框亮出来；选回预设就收起。
  const [customFont, setCustomFont] = useState(false);
  const {
    client,
    harmonyPlatform,
    mobilePlatform,
    host,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showShuangpinPreedit,
    showCandidateFollowCursor,
    showCandidateWindowScale,
    showCandidateWindowOpacity,
    showCandidateCornerRadius,
    showCandidatePadding,
    showCandidateItemGeometry,
    snapshot,
    draft,
    previewCustomTheme,
    setDraft,
    busy,
    page,
    settingsContentRef,
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
  } = useSettingsForm();
  const tabs = useSettingsTabs(candidateTabs, "候选窗口设置", !mobilePlatform);
  useLayoutEffect(() => {
    if (page === "appearance" && !mobilePlatform && settingsContentRef.current) {
      settingsContentRef.current.scrollTop = 0;
    }
  }, [tabs.selected, page, mobilePlatform, settingsContentRef]);
  const appearanceActions = createAppearanceSettingsActions({
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
  });
  const showPageNumber = host?.candidate_page_number === true;
  const showAppLogo = host?.app_logo === true;
  const showExpandCandidates = host?.expand_candidates === true;
  const surfaceName = mobilePlatform ? "候选栏" : "候选窗口";
  const shuangpinPreeditHere = Boolean(client.touchKeyboardSchemes) && showShuangpinPreedit;
  if (harmonyPlatform && mobilePlatform) {
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "appearance"} ariaLabel={surfaceName}>
        <HarmonyPhoneCandidateGroups />
      </SettingsPageFieldset>
    );
  }
  // 桌面预览随当前样例伸缩；手机保留原有的最高排布占位。
  const layoutFixed = host?.fixed_candidate_layout !== undefined;
  const pageSizeFixed = host?.fixed_candidate_page_size !== undefined;
  const previewReserve =
    !mobilePlatform || (layoutFixed && pageSizeFixed)
      ? undefined
      : {
          orientation: layoutFixed ? (draft.candidate_layout ?? "vertical") : ("vertical" as const),
          count: pageSizeFixed
            ? draft.candidate_page_size
            : Math.max(
                ...offeredCandidatePageSizes(
                  draft.candidate_page_size,
                  host?.max_candidate_page_size,
                ),
              ),
        };
  // 字体不属于预设（例如霞鹜文楷），或用户刚点了「自定义…」时显示字体框。
  const layoutControls = (
    <>
      {/* iOS 候选栏的排列和每页数量都是固定的，这两行在那里不出现。 */}
      <CandidateLayoutSection
        value={draft.candidate_layout}
        fixed={layoutFixed}
        onChange={(candidate_layout) => appearanceActions.onPreferencesChange({ candidate_layout })}
      />
      <CandidatePageSizeSection
        value={draft.candidate_page_size}
        fixed={pageSizeFixed}
        max={host?.max_candidate_page_size}
        onChange={(candidate_page_size) =>
          appearanceActions.onPreferencesChange({ candidate_page_size })
        }
      />
    </>
  );
  const fontControls = (
    <>
      {showCandidateFontControls ? (
        <>
          <CandidateFontSettingsRows
            preferences={draft}
            platform={host?.platform}
            readFonts={client.listFontFamilies}
            showFontSize
            showPreeditFont={false}
            englishFont={false}
            onChange={appearanceActions.onPreferencesChange}
            custom={customFont}
            onCustomChange={setCustomFont}
          />
        </>
      ) : (
        <CandidateFontUnsupportedNotice />
      )}
    </>
  );
  const extraLayoutControls = (
    <>
      {showExpandCandidates && (
        <CandidateExpandSection
          value={draft.expand_candidates}
          showButton={draft.show_candidate_expand_button}
          onShowButtonChange={(show_candidate_expand_button) =>
            appearanceActions.onPreferencesChange({ show_candidate_expand_button })
          }
          onChange={(expand_candidates) =>
            appearanceActions.onPreferencesChange({ expand_candidates })
          }
        />
      )}
      {showPageNumber && (
        <CandidatePageNumberSection
          value={draft.show_candidate_page_number}
          platform={host?.platform}
          onChange={(show_candidate_page_number) =>
            appearanceActions.onPreferencesChange({ show_candidate_page_number })
          }
        />
      )}
      {showAppLogo && (
        <CandidateAppLogoSection
          value={draft.show_app_logo}
          onChange={(show_app_logo) => appearanceActions.onPreferencesChange({ show_app_logo })}
        />
      )}
      {showCandidateFollowCursor && (
        <CandidateFollowCursorSection
          value={draft.candidate_follow_cursor}
          onChange={(candidate_follow_cursor) =>
            appearanceActions.onPreferencesChange({ candidate_follow_cursor })
          }
        />
      )}
    </>
  );
  const englishFontControls = (
    <>
      {showCandidateFontControls && showCandidateEnglishFont && (
        <CandidateFontControls
          value={draft}
          onChange={appearanceActions.onPreferencesChange}
          readFonts={client.listFontFamilies}
          windows={host?.platform === "windows"}
          mainFont={false}
          englishFont
        />
      )}
    </>
  );
  const preeditFontControls = (
    <>
      {showCandidateFontControls && (
        <CandidateSizingSection
          preferences={draft}
          showFontControls={false}
          showPreeditFont={showCandidatePreeditFont}
          onChange={appearanceActions.onPreferencesChange}
        />
      )}
    </>
  );
  const scaleControl = (
    <>
      {showCandidateWindowScale && (
        <CandidateScaleRow
          preferences={draft}
          platform={host?.platform}
          onChange={appearanceActions.onPreferencesChange}
        />
      )}
    </>
  );
  const windowStyleControls = (
    <>
      <CandidateWindowStyleRows
        preferences={draft}
        platform={host?.platform}
        showOpacity={showCandidateWindowOpacity}
        showCornerRadius={showCandidateCornerRadius}
        showPadding={showCandidatePadding}
        showItemGeometry={showCandidateItemGeometry}
        showFitContent={host?.platform === "macos"}
        onChange={appearanceActions.onPreferencesChange}
      />
      {/* 候选徽标只有 macOS 宿主消费；其它平台画出来会是个不起作用的开关。 */}
      {host?.platform === "macos" && (
        <CandidateBadgeSettingsSection
          fixedBadge={draft.candidate_fixed_badge ?? true}
          fixedBadgeStyle={draft.candidate_fixed_badge_style ?? "paperclip"}
          aiBadgeStyle={draft.candidate_ai_badge_style ?? "robot"}
          onFixedBadgeChange={(candidate_fixed_badge) =>
            appearanceActions.onPreferencesChange({ candidate_fixed_badge })
          }
          onFixedBadgeStyleChange={(candidate_fixed_badge_style) =>
            appearanceActions.onPreferencesChange({ candidate_fixed_badge_style })
          }
          onAiBadgeStyleChange={(candidate_ai_badge_style) =>
            appearanceActions.onPreferencesChange({ candidate_ai_badge_style })
          }
        />
      )}
    </>
  );
  const preeditControls = (
    <>
      {/* 触屏键盘只剩「双拼预编辑」一行，不显示双拼时这里什么也不画。没有触屏方案的宿主，双拼预编辑在「输入 › 方案」的双拼详情里，只在双拼方案下出现，这里不再画；有触屏方案的宿主（含鸿蒙二合一）的方案组不画双拼各行，仍在这里设置。判断与输入页用同一个条件 `client.touchKeyboardSchemes`，保证这一行恰好出现在一个页面上。 */}
      <PreeditSettingsSection
        preferences={draft}
        mobile={mobilePlatform}
        showShuangpinPreedit={shuangpinPreeditHere}
        onChange={appearanceActions.onPreferencesChange}
      />
    </>
  );
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "appearance"} ariaLabel="候选窗口">
      {/* 桌面页首的插画引导已经说明本页管什么、皮肤在哪里改，这里不再重复一段。 */}
      {tabs.tabList}
      {/* Linux 上由桌面环境接管候选面板时，限制说明紧跟在预览下面。 */}
      <GroupList>
        <AppearanceCandidatePreview
          preferences={appearanceSettingsPreferences(
            { ...draft, custom_theme: previewCustomTheme },
            host?.platform === "windows",
          )}
          scan={client.scanSkinCatalog}
          readImage={client.readSkinImage}
          resolveTheme={client.resolveTheme}
          resolveFonts={client.resolveFontFamilies}
          active={page === "appearance"}
          revision={snapshot?.revision ?? 0}
          mobile={mobilePlatform}
          reserve={previewReserve}
          brand={!showAppLogo || draft.show_app_logo === true}
        />
        {host?.candidate_panel_limit && (
          <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
        )}
      </GroupList>
      {mobilePlatform ? (
        <GroupList title="布局与字体">
          {layoutControls}
          {fontControls}
          <MoreOptions>
            {extraLayoutControls}
            {englishFontControls}
            {preeditFontControls}
            {scaleControl}
            {windowStyleControls}
            {preeditControls}
          </MoreOptions>
        </GroupList>
      ) : (
        <>
          {tabs.panel(
            "layout",
            <GroupList title="布局">
              {layoutControls}
              {extraLayoutControls}
            </GroupList>,
          )}
          {tabs.panel(
            "font",
            <GroupList title="字体与大小">
              {fontControls}
              {englishFontControls}
              {scaleControl}
            </GroupList>,
          )}
          {tabs.panel("style", <GroupList title="窗口样式">{windowStyleControls}</GroupList>)}
          {tabs.panel(
            "preedit",
            <GroupList title="预编辑">
              {preeditFontControls}
              {preeditControls}
            </GroupList>,
          )}
        </>
      )}
    </SettingsPageFieldset>
  );
}

/**
 * HarmonyOS 手机的「候选栏」页，按设计布局：没有预览（键盘本身就是预览），没有通往主题页的链接；「候选栏」组以「候选字号」滑块开头。外接键盘的翻页键不在这里，在「外接键盘快捷键」页：它们只对接了实体键盘的人有用，放在候选栏页像是把电脑的设置搬上了手机。字体、预设、预编辑字号和预编辑这几行仍可在「更多选项」里找到。不提供每页候选数这一行：手机候选栏按宽度翻页，从不读取 `candidate_page_size`。
 */
function HarmonyPhoneCandidateGroups() {
  const {
    client,
    host,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showShuangpinPreedit,
    showCandidateFollowCursor,
    showCandidateWindowScale,
    showCandidateWindowOpacity,
    showCandidateCornerRadius,
    showCandidatePadding,
    showCandidateItemGeometry,
    draft,
    setDraft,
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
  } = useSettingsForm();
  const appearanceActions = createAppearanceSettingsActions({
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
  });
  return (
    <>
      {host?.candidate_panel_limit && (
        <GroupList>
          <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
        </GroupList>
      )}
      <GroupList title="候选栏">
        {showCandidateFontControls ? (
          <CandidateFontSizeSliderRow
            preferences={draft}
            onChange={appearanceActions.onPreferencesChange}
          />
        ) : (
          <CandidateFontUnsupportedNotice />
        )}
        <CandidateLayoutSection
          value={draft.candidate_layout}
          fixed={host?.fixed_candidate_layout !== undefined}
          onChange={(candidate_layout) =>
            appearanceActions.onPreferencesChange({ candidate_layout })
          }
        />
        {host?.candidate_page_number === true && (
          <CandidatePageNumberSection
            value={draft.show_candidate_page_number}
            platform={host?.platform}
            onChange={(show_candidate_page_number) =>
              appearanceActions.onPreferencesChange({ show_candidate_page_number })
            }
          />
        )}
        {showCandidateFollowCursor && (
          <CandidateFollowCursorSection
            value={draft.candidate_follow_cursor}
            onChange={(candidate_follow_cursor) =>
              appearanceActions.onPreferencesChange({ candidate_follow_cursor })
            }
          />
        )}
        <MoreOptions>
          {showCandidateFontControls && (
            <>
              <CandidateFontSettingsRows
                preferences={draft}
                platform={host?.platform}
                readFonts={client.listFontFamilies}
                showFontSize={false}
                showPreeditFont={showCandidatePreeditFont}
                englishFont={showCandidateEnglishFont}
                onChange={appearanceActions.onPreferencesChange}
              />
            </>
          )}
          {showCandidateWindowScale && (
            <CandidateScaleRow
              preferences={draft}
              platform={host?.platform}
              onChange={appearanceActions.onPreferencesChange}
            />
          )}
          <PreeditSettingsSection
            preferences={draft}
            mobile
            showShuangpinPreedit={showShuangpinPreedit}
            onChange={appearanceActions.onPreferencesChange}
          />
        </MoreOptions>
      </GroupList>
      {/* 手机候选栏有自己的底色和圆角，所以这些只在某个宿主在手机上声明支持时才出现；设计里这一页不放通往主题页的链接。 */}
      {(showCandidateWindowOpacity || showCandidateCornerRadius || showCandidatePadding) && (
        <CandidateWindowStyleSection
          preferences={draft}
          platform={host?.platform}
          showOpacity={showCandidateWindowOpacity}
          showCornerRadius={showCandidateCornerRadius}
          showPadding={showCandidatePadding}
          showItemGeometry={showCandidateItemGeometry}
          onChange={appearanceActions.onPreferencesChange}
        />
      )}
    </>
  );
}
