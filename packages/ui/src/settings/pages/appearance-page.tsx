import { AppearanceCandidatePreview } from "../../candidate/appearance-candidate-preview";
import { CandidateFontControls } from "../../candidate/candidate-font-controls";
import { useSettingsForm } from "../settings-form-context";
import * as settings from "../settings-style";
import { GroupList, LinkRow } from "../../core/platform-controls";
import { CandidateFollowCursorSection } from "../candidate-follow-cursor-section";
import { CandidatePageNumberSection } from "../candidate-page-number-section";
import { CandidateSizingSection } from "../candidate-sizing-section";
import { CandidatePageSizeSection } from "../candidate-page-size-section";
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

/** The 候选窗口 page of the settings form (route id `appearance`). */
export function AppearanceSettingsPage() {
  const {
    client,
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
  const showLayoutGroup =
    host?.fixed_candidate_page_size === undefined ||
    host?.fixed_candidate_layout === undefined ||
    showPageNumber ||
    showCandidateFollowCursor;
  const surfaceName = mobilePlatform ? "候选栏" : "候选窗口";
  return (
    <fieldset disabled={busy} hidden={page !== "appearance"} aria-label="候选窗口">
      {/* Basic to advanced: how the candidates are laid out, how big they are drawn, the window around them, and last the preedit. The colours and light/dark are edited only on 主题, which 窗口样式 links to; 翻页方式 is on the 输入 page next to 以词定字. */}
      <div className={settings.groups}>
        {/* 预览放在页首，它画的是下面所有组的设置；Linux 上由桌面环境接管候选面板时，限制说明紧跟在预览下面。 */}
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
          />
          {host?.candidate_panel_limit && (
            <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
          )}
        </GroupList>
        {showLayoutGroup && (
          <GroupList title="布局">
            <CandidateLayoutSection
              value={draft.candidate_layout}
              fixed={host?.fixed_candidate_layout !== undefined}
              onChange={(candidate_layout) =>
                appearanceActions.onPreferencesChange({ candidate_layout })
              }
            />
            {/* The iOS strip pages in nines whatever this says, so a selector there would change nothing. */}
            <CandidatePageSizeSection
              value={draft.candidate_page_size}
              fixed={host?.fixed_candidate_page_size !== undefined}
              onChange={(candidate_page_size) =>
                appearanceActions.onPreferencesChange({ candidate_page_size })
              }
            />
            {showPageNumber && (
              <CandidatePageNumberSection
                value={draft.show_candidate_page_number}
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
          </GroupList>
        )}
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
        </GroupList>
        <CandidateWindowStyleSection
          preferences={draft}
          showOpacity={showCandidateWindowOpacity}
          showCornerRadius={showCandidateCornerRadius}
          onChange={appearanceActions.onPreferencesChange}
        >
          <LinkRow
            title="颜色与明暗"
            description={`${surfaceName}的颜色和明暗在「主题」页设置`}
            onClick={() => selectPage("skin")}
          />
        </CandidateWindowStyleSection>
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
        </GroupList>
      </div>
    </fieldset>
  );
}
