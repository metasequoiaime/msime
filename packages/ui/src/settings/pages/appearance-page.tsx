import { AppearanceCandidatePreview } from "../../candidate/appearance-candidate-preview";
import { CandidateFontControls } from "../../candidate/candidate-font-controls";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, LinkRow, MoreOptions } from "../../core/platform-controls";
import { CandidateFollowCursorSection } from "../candidate-follow-cursor-section";
import { CandidatePageNumberSection } from "../candidate-page-number-section";
import { CandidateAppLogoSection } from "../candidate-app-logo-section";
import { CandidateFontSizeSliderRow, CandidateSizingSection } from "../candidate-sizing-section";
import { NavigationSection, defaultNavigation } from "../navigation-section";
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

/** The 候选窗口 page of the settings form (route id `appearance`). */
export function AppearanceSettingsPage() {
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
  const showLayoutGroup =
    host?.fixed_candidate_page_size === undefined ||
    host?.fixed_candidate_layout === undefined ||
    showPageNumber ||
    showAppLogo ||
    showCandidateFollowCursor;
  const surfaceName = mobilePlatform ? "候选栏" : "候选窗口";
  if (harmonyPlatform && mobilePlatform) {
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "appearance"} ariaLabel={surfaceName}>
        <HarmonyPhoneCandidateGroups />
      </SettingsPageFieldset>
    );
  }
  // 预览按下面「布局」组能选到的最高排布预留高度：排列方式能改时按纵向，每页数量能改时按滑块的最大值，两者都固定（iOS）时不预留。
  const layoutFixed = host?.fixed_candidate_layout !== undefined;
  const pageSizeFixed = host?.fixed_candidate_page_size !== undefined;
  const previewReserve =
    layoutFixed && pageSizeFixed
      ? undefined
      : {
          orientation: layoutFixed ? (draft.candidate_layout ?? "vertical") : ("vertical" as const),
          count: pageSizeFixed
            ? draft.candidate_page_size
            : Math.max(...offeredCandidatePageSizes(draft.candidate_page_size)),
        };
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "appearance"} ariaLabel="候选窗口">
      {/* 从基础到进阶：候选怎么排列、绘制多大、外围的窗口，最后是预编辑。颜色和明暗只在「主题」编辑，「窗口样式」链接过去；「翻页方式」在「输入」页，挨着「以词定字」。 */}

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
          reserve={previewReserve}
          brand={!showAppLogo || draft.show_app_logo === true}
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
          <CandidateScaleRow preferences={draft} onChange={appearanceActions.onPreferencesChange} />
        )}
      </GroupList>
      <CandidateWindowStyleSection
        preferences={draft}
        showOpacity={showCandidateWindowOpacity}
        showCornerRadius={showCandidateCornerRadius}
        onChange={appearanceActions.onPreferencesChange}
      >
        <LinkRow
          title="皮肤、颜色与明暗"
          description={`${surfaceName}的皮肤、颜色和明暗在「主题」页设置`}
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
    </SettingsPageFieldset>
  );
}

/**
 * HarmonyOS 手机的「候选栏」页，按设计布局：没有预览（键盘本身就是预览），没有通往主题页的链接；「候选栏」组以「候选字号」滑块开头；「翻页」组放硬件翻页键，在手机上它们放在这里而不在「输入」页。字体、预设、预编辑字号和预编辑这几行仍可在「更多选项」里找到。不提供每页候选数这一行：手机候选栏按宽度翻页，从不读取 `candidate_page_size`。
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
    draft,
    setDraft,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    saveMobileKeyboardFeedback,
    wordCharacter,
  } = useSettingsForm();
  const appearanceActions = createAppearanceSettingsActions({
    mobileKeyboardFeedback,
    saveMobileKeyboardFeedback,
    setDraft,
  });
  const navigation = draft.navigation ?? defaultNavigation;
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
      <GroupList title="翻页">
        <NavigationSection
          navigation={navigation}
          wordCharacter={wordCharacter}
          linux={false}
          harmonyPhone
          onChange={(next) =>
            appearanceActions.onPreferencesChange({
              // 只有占用了「以词定字」按键的翻页键才会改动它；否则未设置的值保持未设置。
              ...(next.wordCharacter !== wordCharacter
                ? { word_character: next.wordCharacter }
                : {}),
              navigation: next.navigation,
            })
          }
        />
      </GroupList>
      {/* 手机候选栏有自己的底色和圆角，所以这些只在某个宿主在手机上声明支持时才出现；设计里这一页不放通往主题页的链接。 */}
      {(showCandidateWindowOpacity || showCandidateCornerRadius) && (
        <CandidateWindowStyleSection
          preferences={draft}
          showOpacity={showCandidateWindowOpacity}
          showCornerRadius={showCandidateCornerRadius}
          onChange={appearanceActions.onPreferencesChange}
        />
      )}
    </>
  );
}
