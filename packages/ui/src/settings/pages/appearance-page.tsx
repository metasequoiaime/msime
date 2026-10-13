import { useId, useState } from "react";
import type { ReactNode } from "react";
import * as settingsStyle from "../settings-style";
import { AppearanceCandidatePreview } from "../../candidate/appearance-candidate-preview";
import { CandidateFontControls } from "../../candidate/candidate-font-controls";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, MoreOptions } from "../../core/platform-controls";
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
  CandidateFontPresetRow,
  CandidateScaleRow,
  CandidateWindowStyleSection,
} from "../candidate-window-style-section";
import { SettingsPageFieldset } from "../settings-page-fieldset";

const candidateTabs = [
  { id: "layout", title: "布局" },
  { id: "font", title: "字体与大小" },
  { id: "style", title: "窗口样式" },
  { id: "preedit", title: "预编辑" },
] as const;
type CandidateTab = (typeof candidateTabs)[number]["id"];

/** 候选窗口设置页，路由标识为 `appearance`。 */
export function AppearanceSettingsPage() {
  const [selectedTab, setSelectedTab] = useState<CandidateTab>("layout");
  const tabsId = useId();
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
    snapshot,
    draft,
    setDraft,
    busy,
    page,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    saveMobileKeyboardFeedback,
    selectPage,
  } = useSettingsForm();
  const appearanceActions = createAppearanceSettingsActions({
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
  });
  const showPageNumber = host?.candidate_page_number === true;
  const showAppLogo = host?.app_logo === true;
  const showExpandCandidates = host?.expand_candidates === true;
  const showLayoutGroup =
    host?.fixed_candidate_page_size === undefined ||
    host?.fixed_candidate_layout === undefined ||
    showPageNumber ||
    showAppLogo ||
    showExpandCandidates ||
    showCandidateFollowCursor;
  const surfaceName = mobilePlatform ? "候选栏" : "候选窗口";
  if (harmonyPlatform && mobilePlatform) {
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "appearance"} ariaLabel={surfaceName}>
        <HarmonyPhoneCandidateGroups />
      </SettingsPageFieldset>
    );
  }
  // 桌面预览随当前样例伸缩，让下方标签紧邻候选窗；手机保留原有的最高排布占位。
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
  const panel = (tab: CandidateTab, children: ReactNode) =>
    mobilePlatform ? (
      children
    ) : (
      <div
        role="tabpanel"
        id={`${tabsId}-${tab}-panel`}
        aria-labelledby={`${tabsId}-${tab}-tab`}
        hidden={selectedTab !== tab}
        tabIndex={0}
      >
        {children}
      </div>
    );
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "appearance"} ariaLabel="候选窗口">
      {/* 预览在四组设置间共用，切换标签时保持挂载；Linux 上由桌面环境接管候选面板时，限制说明紧跟在预览下面。 */}
      <GroupList>
        <AppearanceCandidatePreview
          preferences={appearanceSettingsPreferences(draft, host?.platform === "windows")}
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
      {!mobilePlatform && (
        <div className={settingsStyle.candidateTabs} role="tablist" aria-label="候选窗口设置分类">
          {candidateTabs.map((tab, index) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`${tabsId}-${tab.id}-tab`}
              aria-controls={`${tabsId}-${tab.id}-panel`}
              aria-selected={selectedTab === tab.id}
              tabIndex={selectedTab === tab.id ? 0 : -1}
              onClick={() => setSelectedTab(tab.id)}
              onKeyDown={(event) => {
                let next: number;
                if (event.key === "ArrowRight") next = (index + 1) % candidateTabs.length;
                else if (event.key === "ArrowLeft")
                  next = (index + candidateTabs.length - 1) % candidateTabs.length;
                else if (event.key === "Home") next = 0;
                else if (event.key === "End") next = candidateTabs.length - 1;
                else return;
                event.preventDefault();
                setSelectedTab(candidateTabs[next].id);
                document.getElementById(`${tabsId}-${candidateTabs[next].id}-tab`)?.focus();
              }}
            >
              {tab.title}
            </button>
          ))}
        </div>
      )}
      {panel(
        "layout",
        showLayoutGroup && (
          <GroupList title="布局">
            <CandidateLayoutSection
              value={draft.candidate_layout}
              fixed={host?.fixed_candidate_layout !== undefined}
              onChange={(candidate_layout) =>
                appearanceActions.onPreferencesChange({ candidate_layout })
              }
            />
            {/* iOS 候选栏每页固定九项，不提供无效的数量选择器。 */}
            <CandidatePageSizeSection
              value={draft.candidate_page_size}
              fixed={host?.fixed_candidate_page_size !== undefined}
              max={host?.max_candidate_page_size}
              onChange={(candidate_page_size) =>
                appearanceActions.onPreferencesChange({ candidate_page_size })
              }
            />
            {showExpandCandidates && (
              <CandidateExpandSection
                value={draft.expand_candidates}
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
                onChange={(show_app_logo) =>
                  appearanceActions.onPreferencesChange({ show_app_logo })
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
          </GroupList>
        ),
      )}
      {panel(
        "font",
        <GroupList title="字体与大小">
          {showCandidateFontControls ? (
            <>
              <CandidateFontPresetRow
                preferences={draft}
                platform={host?.platform}
                onChange={appearanceActions.onPreferencesChange}
              />
              <CandidateFontControls
                value={draft}
                onChange={appearanceActions.onPreferencesChange}
                readFonts={client.listFontFamilies}
                windows={host?.platform === "windows"}
                englishFont={showCandidateEnglishFont}
              />
              <CandidateSizingSection
                preferences={draft}
                showFontControls
                showPreeditFont={showCandidatePreeditFont}
                onChange={appearanceActions.onPreferencesChange}
              />
            </>
          ) : (
            <CandidateFontUnsupportedNotice />
          )}
          {showCandidateWindowScale && (
            <CandidateScaleRow
              preferences={draft}
              onChange={appearanceActions.onPreferencesChange}
            />
          )}
        </GroupList>,
      )}
      {panel(
        "style",
        <CandidateWindowStyleSection
          preferences={draft}
          platform={host?.platform}
          showOpacity={showCandidateWindowOpacity}
          showCornerRadius={showCandidateCornerRadius}
          showPadding={showCandidatePadding}
          onChange={appearanceActions.onPreferencesChange}
        >
          <LinkRow
            title="皮肤、颜色与明暗"
            description={`${surfaceName}的皮肤、颜色和明暗在「主题」页设置`}
            onClick={() => selectPage("skin")}
          />
        </CandidateWindowStyleSection>,
      )}
      {panel(
        "preedit",
        <GroupList title="预编辑">
          <PreeditSettingsSection
            preferences={draft}
            mobile={mobilePlatform}
            showShuangpinPreedit={showShuangpinPreedit}
            inlinePreedit={mobileKeyboardFeedback?.inlinePreedit}
            inlinePreeditBusy={mobileKeyboardFeedbackBusy}
            onChange={appearanceActions.onPreferencesChange}
            onInlinePreeditChange={appearanceActions.onInlinePreeditChange}
          />
        </GroupList>,
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
    draft,
    setDraft,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
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
              <CandidateFontPresetRow
                preferences={draft}
                platform={host?.platform}
                onChange={appearanceActions.onPreferencesChange}
              />
              <CandidateFontControls
                value={draft}
                onChange={appearanceActions.onPreferencesChange}
                readFonts={client.listFontFamilies}
                windows={false}
                englishFont={showCandidateEnglishFont}
              />
              <CandidateSizingSection
                preferences={draft}
                showFontControls={false}
                showPreeditFont={showCandidatePreeditFont}
                onChange={appearanceActions.onPreferencesChange}
              />
            </>
          )}
          {showCandidateWindowScale && (
            <CandidateScaleRow
              preferences={draft}
              onChange={appearanceActions.onPreferencesChange}
            />
          )}
          <PreeditSettingsSection
            preferences={draft}
            mobile
            showShuangpinPreedit={showShuangpinPreedit}
            inlinePreedit={mobileKeyboardFeedback?.inlinePreedit}
            inlinePreeditBusy={mobileKeyboardFeedbackBusy}
            onChange={appearanceActions.onPreferencesChange}
            onInlinePreeditChange={appearanceActions.onInlinePreeditChange}
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
          onChange={appearanceActions.onPreferencesChange}
        />
      )}
    </>
  );
}
