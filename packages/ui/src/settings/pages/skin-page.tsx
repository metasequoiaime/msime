import type { ThemeMode } from "../../index";
import * as settings from "../settings-style";
import { globalThemeDescription } from "../settings-options";
import {
  themeCatalog,
  themeEntry,
  customCandidateStyle,
  themeCandidateStyle,
  keyboardThemeId,
} from "../../theme/global-theme";
import { SkinCandidatePreview } from "../../skin/skin-candidate-preview";
import { SkinToolbarPreview } from "../../skin/skin-toolbar-preview";
import { ExternalSkins } from "../../skin/external-skins";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import { TouchKeyboardSkinEditor } from "../../keyboard/touch-keyboard-skin-editor";
import { defaultTouchKeyboardSkinDesign } from "../../keyboard/touch-keyboard-skin-design";
import { GroupList, Row, Segmented, Switch } from "../../core/platform-controls";
import { CandidateColorsSection } from "../candidate-colors-section";
import { ThemeSettingsSection } from "../theme-settings-section";
import { ScreenKeyboardSkinsSection } from "../screen-keyboard-skins-section";
import { useSettingsForm } from "../settings-form-context";
import { CandidatePanelLimitSection } from "../candidate-panel-limit-section";
import { CandidatePaletteFallbackNotice } from "../candidate-palette-fallback-notice";
import { SkinPlatformNotice } from "../skin-platform-notice";
import { ThemeCarousel } from "../theme-carousel";
import { createSettingsDraftActions } from "../settings-draft-actions";

const themeModeOptions = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
] as const satisfies readonly { value: ThemeMode; label: string }[];

/** The 主题 page of the settings form (route id `skin`): the global theme, what a custom theme is made of, and the per-surface overrides. */
export function SkinSettingsPage() {
  const {
    client,
    linuxPlatform,
    mobilePlatform,
    host,
    snapshot,
    draft,
    setDraft,
    busy,
    page,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    skinPreviewThemes,
    setSkinPreviewThemes,
    saveMobileKeyboardFeedback,
    candidatePreviewTheme,
    globalTheme,
    themeMode,
    customColors,
    setCandidateColor,
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showFloatingToolbar,
    desktopPanels,
    keyboardPreviewTheme,
    customTouchKeyboardSkin,
    withCustomKeyboard,
    customKeyboardSelected,
    showTouchSkinEditor,
    setShowTouchSkinEditor,
    openCommunity,
  } = useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return (
    <fieldset disabled={busy} hidden={page !== "skin"} aria-label="主题">
      <SkinPlatformNotice mobile={mobilePlatform} linux={linuxPlatform} />
      {host?.candidate_panel_limit && (
        <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
      )}
      <ThemeCarousel
        labels={themeCatalog.map((entry) => entry.title)}
        selectedIndex={Math.max(
          themeCatalog.findIndex((entry) => entry.id === globalTheme),
          0,
        )}
      >
        {themeCatalog.map((entry) => {
          const id = entry.id;
          const selected = globalTheme === id;
          // The custom card draws what the custom theme is assembled from: its base and pickers here, and its package in the external skin list below.
          const customPackage = id === "custom" ? draft.custom_theme?.candidate_skin : null;
          const fixedAppearance =
            id === "custom"
              ? themeEntry(draft.custom_theme?.base ?? "system").appearance
              : entry.appearance;
          // Only `system`, and a custom theme over it, follow the light/dark mode; a built-in theme is one fixed palette.
          const previewTheme = fixedAppearance ?? skinPreviewThemes[id] ?? candidatePreviewTheme;
          return (
            <article aria-label={entry.title} className={settings.skinCard(selected)} key={id}>
              <div className={settings.skinCardHeader} data-skin-card-header="">
                <div className={settings.skinCardBody}>
                  <span className={settings.skinCardTitle}>
                    {entry.title} ({previewTheme === "dark" ? "Dark" : "Light"})
                    {selected && <span className={settings.skinCardInUse}>使用中</span>}
                  </span>
                  <span className={settings.skinCardDescription}>
                    {customPackage
                      ? `外部皮肤 ${customPackage}`
                      : globalThemeDescription(entry, linuxPlatform)}
                  </span>
                </div>
                <div className={settings.skinCardActions}>
                  <button
                    type="button"
                    role="switch"
                    aria-label={entry.title}
                    aria-checked={selected}
                    className={settings.skinSwitch(selected)}
                    onClick={() => onPreferencesChange({ global_theme: id })}
                  >
                    <span className={settings.skinSwitchKnob(selected)} />
                  </button>
                  {fixedAppearance === null && (
                    <button
                      type="button"
                      className={settings.skinPreviewSwitch}
                      onClick={() =>
                        setSkinPreviewThemes((current) => ({
                          ...current,
                          [id]:
                            (current[id] ?? candidatePreviewTheme) === "dark" ? "light" : "dark",
                        }))
                      }
                    >
                      {previewTheme === "dark" ? "预览浅色" : "预览深色"}
                    </button>
                  )}
                </div>
              </div>
              <div
                className={settings.skinCardPreview}
                data-skin-preview=""
                data-global-theme={id}
                data-preview-theme={previewTheme}
                style={
                  id === "custom"
                    ? customCandidateStyle(
                        draft.custom_theme?.base,
                        draft.custom_theme?.candidate_colors,
                      )
                    : themeCandidateStyle(id)
                }
                aria-hidden="true"
              >
                <div className={settings.skinPreviewStage} data-skin-stage="">
                  <SkinCandidatePreview orientation="horizontal" />
                </div>
                <div className={settings.skinPreviewStage} data-skin-stage="">
                  <SkinCandidatePreview orientation="vertical" />
                </div>
                {/* A touch host has no floating toolbar; the theme's other surface there is the keyboard, which the 键盘 page used to show a second set of these cards for. */}
                {mobilePlatform ? (
                  <div className={settings.skinPreviewStage} data-skin-stage="">
                    <ScreenKeyboardPreview
                      theme={previewTheme}
                      skin={id === "custom" ? keyboardThemeId("custom", draft.custom_theme) : id}
                      customDesign={id === "custom" ? customTouchKeyboardSkin : undefined}
                      compact
                    />
                  </div>
                ) : (
                  !linuxPlatform && (
                    <div className={settings.skinPreviewStage} data-skin-stage="">
                      <SkinToolbarPreview />
                    </div>
                  )
                )}
              </div>
            </article>
          );
        })}
      </ThemeCarousel>
      <div className={settings.groups}>
        <GroupList title="外观">
          <Row title="颜色模式" description="设置窗口和各界面的默认明暗模式">
            <Segmented
              aria-label="颜色模式"
              options={themeModeOptions}
              value={themeMode}
              onChange={(theme) => onPreferencesChange({ theme })}
            />
          </Row>
        </GroupList>
        <GroupList title="自定义主题">
          {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop !== undefined && (
            <Row
              title="使用桌面候选皮肤"
              description="关闭时候选栏和按键一起使用键盘皮肤的颜色；打开后使用这里的主题和候选颜色。"
            >
              <Switch
                aria-label="使用桌面候选皮肤"
                disabled={mobileKeyboardFeedbackBusy}
                checked={mobileKeyboardFeedback.candidatePaletteFollowsDesktop}
                onChange={(checked) =>
                  void saveMobileKeyboardFeedback({
                    ...mobileKeyboardFeedback,
                    candidatePaletteFollowsDesktop: checked,
                  })
                }
              />
            </Row>
          )}
          {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop === false && (
            <CandidatePaletteFallbackNotice />
          )}
          {/* Choosing a colour makes the theme custom, over whatever theme was on screen (see `setCandidateColor`). */}
          <CandidateColorsSection
            preferences={customColors}
            previewTheme={candidatePreviewTheme}
            showRowColors={showCandidateRowColors}
            showSelectionAppearance={showCandidateSelectionAppearance}
            showBorderColor={showCandidateBorderColor}
            linux={linuxPlatform}
            onChange={setCandidateColor}
          />
          {client.customTouchKeyboardSkins && (
            <ScreenKeyboardSkinsSection
              theme={keyboardPreviewTheme}
              selected={customKeyboardSelected}
              customDesign={customTouchKeyboardSkin}
              editorOpen={showTouchSkinEditor}
              // The card previews the editor's design, so choosing it stores that design; without one the custom theme would draw its base's keyboard.
              onSelect={() => setDraft(withCustomKeyboard(draft, customTouchKeyboardSkin))}
              onToggleEditor={() => setShowTouchSkinEditor((value) => !value)}
            />
          )}
          {client.customTouchKeyboardSkins && showTouchSkinEditor && (
            <div className={settings.groupBlock}>
              <TouchKeyboardSkinEditor
                design={customTouchKeyboardSkin}
                selected={customKeyboardSelected}
                theme={keyboardPreviewTheme}
                disabled={busy}
                library={client.customSkinLibrary}
                aiSkins={client.aiSkins}
                communitySkins={client.communitySkins}
                onChange={(design) =>
                  setDraft((current) =>
                    current
                      ? {
                          ...current,
                          custom_theme: {
                            ...current.custom_theme,
                            keyboard: design,
                          },
                        }
                      : current,
                  )
                }
                onUse={() =>
                  setDraft((current) =>
                    current
                      ? withCustomKeyboard(
                          current,
                          current.custom_theme?.keyboard ?? defaultTouchKeyboardSkinDesign,
                        )
                      : current,
                  )
                }
                onClose={() => setShowTouchSkinEditor(false)}
              />
            </div>
          )}
          {/* The desktop sidebar already lists 社区; a second route to it is noise there. */}
          {mobilePlatform && client.communitySkins && (
            <Row title="社区皮肤" description="看看别人做的键盘皮肤，可以直接试用或保存">
              <button type="button" className="secondary" onClick={() => openCommunity("all")}>
                去社区发现皮肤
              </button>
            </Row>
          )}
        </GroupList>
      </div>
      {snapshot?.candidate_skin_catalog && (
        <div className={settings.externalMeta} role="status">
          外部皮肤目录：
          {snapshot.candidate_skin_catalog.scanned
            ? `已扫描（${snapshot.candidate_skin_catalog.packages.length} 个）`
            : "尚未扫描"}
          {snapshot.candidate_skin_catalog.issues?.length
            ? `，${snapshot.candidate_skin_catalog.issues.length} 个问题`
            : ""}
        </div>
      )}
      <ExternalSkins
        activeTheme={candidatePreviewTheme}
        scan={client.scanSkinCatalog}
        openDirectory={client.openSkinDirectory}
        importsSkin={host?.skin_directory_import === true}
        readImage={client.readSkinImage}
        readFont={client.readSkinFont}
        readToolbarCss={client.readSkinToolbarCss}
        selected={globalTheme === "custom" ? (draft.custom_theme?.candidate_skin ?? "") : ""}
        // A host that draws one layout judges a skin by that layout, not by a setting it ignores.
        layout={host?.fixed_candidate_layout ?? draft.candidate_layout ?? "vertical"}
        // An external package is part of the custom theme, so choosing one selects that theme.
        // The package's manifest base becomes the custom theme's base, which is what `resolve()` draws under the package, so the previews match and removing the package keeps that base.
        onSelect={(id, base) =>
          onPreferencesChange({
            global_theme: "custom",
            custom_theme: { ...draft.custom_theme, base, candidate_skin: id },
          })
        }
        toolbarPreview={!linuxPlatform}
      />
      {draft.custom_theme?.candidate_skin && (
        <div className={settings.externalMeta}>
          <button
            type="button"
            className="secondary"
            // Removing the package keeps the rest of the custom theme and the selection: it is then drawn over its own base.
            onClick={() =>
              onPreferencesChange({
                custom_theme: { ...draft.custom_theme, candidate_skin: null },
              })
            }
          >
            自定义主题不使用外部皮肤
          </button>
        </div>
      )}
      <div className={settings.groups}>
        {/* Each surface can still hold its own light or dark over the colour mode; the design folds these under 高级 on the theme page. */}
        <ThemeSettingsSection
          preferences={draft}
          mobile={mobilePlatform}
          linux={linuxPlatform}
          floatingToolbar={showFloatingToolbar}
          desktopPanels={desktopPanels}
          onChange={(key, value) => onPreferencesChange({ [key]: value })}
        />
      </div>
    </fieldset>
  );
}
