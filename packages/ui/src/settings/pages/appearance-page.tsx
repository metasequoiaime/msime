import { AppearanceCandidatePreview } from "../../candidate/appearance-candidate-preview";
import { CandidateFontControls } from "../../candidate/candidate-font-controls";
import { defaultNavigation } from "../navigation-section";
import { useSettingsForm } from "../settings-form-context";
import * as settings from "../settings-style";
import { GroupList } from "../../core/platform-controls";
import { CandidateFollowCursorSection } from "../candidate-follow-cursor-section";
import { CandidateSizingSection } from "../candidate-sizing-section";
import { CandidatePageSizeSection } from "../candidate-page-size-section";
import { CandidateLayoutSection } from "../candidate-layout-section";
import { PreeditSettingsSection } from "../preedit-settings-section";
import { NavigationSection } from "../navigation-section";
import { appearanceSettingsPreferences } from "../appearance-settings-preferences";
import { CandidatePanelLimitSection } from "../candidate-panel-limit-section";
import { CandidateFontUnsupportedNotice } from "../candidate-font-unsupported-notice";
import { createAppearanceSettingsActions } from "../appearance-settings-actions";

/** The 候选窗口 page of the settings form (route id `appearance`). */
export function AppearanceSettingsPage() {
  const {
    client,
    linuxPlatform,
    mobilePlatform,
    host,
    showCandidateFontControls,
    showCandidatePreeditFont,
    showCandidateEnglishFont,
    showShuangpinPreedit,
    showCandidateFollowCursor,
    snapshot,
    draft,
    setDraft,
    busy,
    page,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    saveMobileKeyboardFeedback,
    wordCharacter,
  } = useSettingsForm();
  const navigation = draft.navigation ?? defaultNavigation;
  const appearanceActions = createAppearanceSettingsActions({
    draft,
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
  });
  const showWindowGroup =
    host?.fixed_candidate_page_size === undefined || host?.fixed_candidate_layout === undefined;
  return (
    <fieldset disabled={busy} hidden={page !== "appearance"} aria-label="候选窗口">
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
      {/* The groups keep the reference window's order of these settings (following, fonts, page size and layout, preedit, paging); the design's 窗口布局 group leads with the layout instead. */}
      <div className={settings.groups}>
        {showCandidateFollowCursor && (
          <GroupList title="位置">
            <CandidateFollowCursorSection
              value={draft.candidate_follow_cursor}
              onChange={(candidate_follow_cursor) =>
                appearanceActions.onPreferencesChange({ candidate_follow_cursor })
              }
            />
          </GroupList>
        )}
        <GroupList title="字体">
          {showCandidateFontControls ? (
            <>
              <CandidateFontControls
                value={draft}
                onChange={appearanceActions.onPreferencesChange}
                readFonts={client.listFontFamilies}
                windows={host?.platform === "windows"}
                englishFont={showCandidateEnglishFont}
                mobile={mobilePlatform}
              />
              <CandidateSizingSection
                preferences={draft}
                mobile={mobilePlatform}
                showFontControls
                showPreeditFont={showCandidatePreeditFont}
                onChange={appearanceActions.onPreferencesChange}
              />
            </>
          ) : (
            <CandidateFontUnsupportedNotice />
          )}
        </GroupList>
        {showWindowGroup && (
          <GroupList title={mobilePlatform ? "布局" : "窗口布局"}>
            {/* The iOS strip pages in nines whatever this says, so a selector there would change nothing. */}
            <CandidatePageSizeSection
              value={draft.candidate_page_size}
              fixed={host?.fixed_candidate_page_size !== undefined}
              onChange={(candidate_page_size) =>
                appearanceActions.onPreferencesChange({ candidate_page_size })
              }
            />
            <CandidateLayoutSection
              value={draft.candidate_layout}
              fixed={host?.fixed_candidate_layout !== undefined}
              onChange={(candidate_layout) =>
                appearanceActions.onPreferencesChange({ candidate_layout })
              }
            />
          </GroupList>
        )}
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
        <GroupList title="翻页">
          <NavigationSection
            navigation={navigation}
            wordCharacter={wordCharacter}
            linux={linuxPlatform}
            onChange={(next) =>
              appearanceActions.onPreferencesChange({
                // Only a paging key that takes 以词定字's key touches it; otherwise an unset value stays unset.
                ...(next.wordCharacter !== wordCharacter
                  ? { word_character: next.wordCharacter }
                  : {}),
                navigation: next.navigation,
              })
            }
          />
        </GroupList>
      </div>
    </fieldset>
  );
}
