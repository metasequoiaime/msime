import { useState } from "react";
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
import {
  ExternalSkinCard,
  ExternalSkinDirectoryRow,
  useSkinCatalog,
} from "../../skin/external-skins";
import { CandidateSkinPublishDialog } from "../../community/candidate-skin-publish-dialog";
import type { CandidateSkinVisibility } from "../../community/community-candidate-skins";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import { TouchKeyboardSkinEditor } from "../../keyboard/touch-keyboard-skin-editor";
import { GroupList, Row, Segmented } from "../../core/platform-controls";
import { CandidateColorsSection } from "../candidate-colors-section";
import { ThemeSettingsSection } from "../theme-settings-section";
import { ScreenKeyboardSkinsSection } from "../screen-keyboard-skins-section";
import { useSettingsForm } from "../settings-form-context";
import { CandidatePanelLimitSection } from "../candidate-panel-limit-section";
import { CandidatePaletteFallbackNotice } from "../candidate-palette-fallback-notice";
import { SkinPlatformNotice } from "../skin-platform-notice";
import { ThemeCarousel } from "../theme-carousel";
import { SwitchRow } from "../switch-row";
import { createSettingsDraftActions } from "../settings-draft-actions";

const themeModeOptions = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
] as const satisfies readonly { value: ThemeMode; label: string }[];

/** The 主题 page of the settings form (route id `skin`): the global theme, what a custom theme is made of, and the per-surface overrides. `hidden` keeps it mounted while the desktop shell shows the 社区皮肤 tab in its place. */
export function SkinSettingsPage({ hidden = false }: { hidden?: boolean }) {
  const {
    client,
    linuxPlatform,
    mobilePlatform,
    host,
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
    onCandidateColorChange,
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showFloatingToolbar,
    desktopPanels,
    keyboardPreviewTheme,
    customTouchKeyboardSkin,
    onCustomKeyboardChange,
    onUseCustomKeyboard,
    customKeyboardSelected,
    showTouchSkinEditor,
    setShowTouchSkinEditor,
    openCommunity,
    openAccountLogin,
  } = useSettingsForm();
  const { onCustomThemeChange, onPreferencesChange } = createSettingsDraftActions({ setDraft });
  // The package offered to the candidate-skin community from its card; the dialog is drawn outside the fieldset so a settings save in flight does not disable it.
  const [publishSkinId, setPublishSkinId] = useState<string | null>(null);
  const [published, setPublished] = useState<CandidateSkinVisibility | null>(null);
  const candidateSkins = client.communityCandidateSkins;
  const importsSkin = host?.skin_directory_import === true;
  const skins = useSkinCatalog(client.scanSkinCatalog, client.openSkinDirectory, importsSkin);
  const packages = skins.catalog?.packages ?? [];
  // A package is part of the custom theme, so its card is the one in use while the custom theme draws it; the 自定义 card is then the custom theme without a package.
  const skinInUse = globalTheme === "custom" ? (draft.custom_theme?.candidate_skin ?? null) : null;
  const packageInUse = packages.findIndex((skin) => skin.id === skinInUse);
  return (
    <>
      <fieldset disabled={busy} hidden={hidden || page !== "skin"} aria-label="主题">
        <div className={settings.groups}>
          <SkinPlatformNotice mobile={mobilePlatform} linux={linuxPlatform} />
          {/* 这条说明指出面板会忽略本页的皮肤和颜色，而它们在别处都无法编辑，所以除了「候选窗口」预览下方，这里也保留一份。 */}
          {host?.candidate_panel_limit && (
            <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
          )}
          {/* 颜色模式排在卡片之前，因为每张卡片的明暗预览都以它为起点。 */}
          <GroupList title="明暗">
            <Row title="颜色模式" description="设置窗口和各界面的默认明暗模式">
              <Segmented
                aria-label="颜色模式"
                options={themeModeOptions}
                value={themeMode}
                onChange={(theme) => onPreferencesChange({ theme })}
              />
            </Row>
          </GroupList>
          <ThemeCarousel
            labels={[
              ...themeCatalog.map((entry) => entry.title),
              ...packages.map((skin) => skin.name),
            ]}
            selectedIndex={
              packageInUse >= 0
                ? themeCatalog.length + packageInUse
                : Math.max(
                    themeCatalog.findIndex((entry) => entry.id === globalTheme),
                    0,
                  )
            }
          >
            {themeCatalog.map((entry) => {
              const id = entry.id;
              // The custom card draws what the custom theme is assembled from without a package: its base and pickers. Each package has its own card after the built-in ones.
              // A package the custom theme names but the scan did not list (removed, not scanned yet, or a host without a scanner) is drawn as the custom theme over its base, so the 自定义 card stays the one in use.
              const selected = globalTheme === id && (id !== "custom" || packageInUse < 0);
              const fixedAppearance =
                id === "custom"
                  ? themeEntry(draft.custom_theme?.base ?? "system").appearance
                  : entry.appearance;
              // Only `system`, and a custom theme over it, follow the light/dark mode; a built-in theme is one fixed palette.
              const previewTheme =
                fixedAppearance ?? skinPreviewThemes[id] ?? candidatePreviewTheme;
              return (
                <article aria-label={entry.title} className={settings.skinCard(selected)} key={id}>
                  <div className={settings.skinCardHeader} data-skin-card-header="">
                    <div className={settings.skinCardBody}>
                      <span className={settings.skinCardTitle}>
                        {entry.title}（{previewTheme === "dark" ? "深色" : "浅色"}）
                        {selected && <span className={settings.skinCardInUse}>使用中</span>}
                      </span>
                      <span className={settings.skinCardDescription}>
                        {globalThemeDescription(entry, linuxPlatform)}
                      </span>
                    </div>
                    <div className={settings.skinCardActions}>
                      <button
                        type="button"
                        role="switch"
                        aria-label={entry.title}
                        aria-checked={selected}
                        className={settings.skinSwitch(selected)}
                        onClick={() =>
                          onPreferencesChange(
                            id === "custom"
                              ? // Choosing the custom card itself drops the package and keeps the rest of the custom theme, drawn over its own base.
                                {
                                  global_theme: "custom",
                                  custom_theme: { ...draft.custom_theme, candidate_skin: null },
                                }
                              : { global_theme: id },
                          )
                        }
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
                                (current[id] ?? candidatePreviewTheme) === "dark"
                                  ? "light"
                                  : "dark",
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
                    <div className={settings.skinCandidateStages}>
                      <div className={settings.skinPreviewStage} data-skin-stage="">
                        <SkinCandidatePreview orientation="horizontal" />
                      </div>
                      <div className={settings.skinPreviewStage} data-skin-stage="">
                        <SkinCandidatePreview orientation="vertical" />
                      </div>
                    </div>
                    {/* A touch host has no floating toolbar; the theme's other surface there is the keyboard, which the 键盘 page used to show a second set of these cards for. */}
                    {mobilePlatform ? (
                      <div className={settings.skinPreviewStage} data-skin-stage="">
                        <ScreenKeyboardPreview
                          theme={previewTheme}
                          skin={
                            id === "custom" ? keyboardThemeId("custom", draft.custom_theme) : id
                          }
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
            {packages.map((skin) => (
              <ExternalSkinCard
                key={`package:${skin.id}`}
                skin={skin}
                selected={skin.id === skinInUse}
                // A host that draws one layout judges a skin by that layout, not by a setting it ignores.
                layout={host?.fixed_candidate_layout ?? draft.candidate_layout ?? "vertical"}
                // The package's manifest base becomes the custom theme's base, which is what `resolve()` draws under the package, so the previews match and removing the package keeps that base.
                onSelect={(id, base) =>
                  onPreferencesChange({
                    global_theme: "custom",
                    custom_theme: { ...draft.custom_theme, base, candidate_skin: id },
                  })
                }
                readImage={client.readSkinImage}
                readFont={client.readSkinFont}
                readToolbarCss={client.readSkinToolbarCss}
                revision={skins.revision}
                activeTheme={candidatePreviewTheme}
                toolbarPreview={!linuxPlatform}
                onPublish={
                  candidateSkins
                    ? (id) => {
                        setPublished(null);
                        setPublishSkinId(id);
                      }
                    : undefined
                }
              />
            ))}
          </ThemeCarousel>
          {published && (
            <p role="status" className={settings.externalMeta}>
              {published === "private" ? "已保存到你的皮肤库，仅自己可见。" : "已发布到社区。"}
            </p>
          )}
          <GroupList title="更多皮肤">
            <ExternalSkinDirectoryRow
              skins={skins}
              scannable={!!client.scanSkinCatalog}
              openable={!!client.openSkinDirectory}
              importsSkin={importsSkin}
            />
            {/* 桌面宿主在本页顶部的「社区皮肤」标签里浏览候选窗口皮肤，并从上面各自的卡片发布；这一行是手机进入「社区」标签里键盘皮肤图库的入口。 */}
            {mobilePlatform && client.communitySkins && (
              <Row title="在线皮肤" description="看看别人做的键盘皮肤，可以直接试用或保存">
                <button type="button" className="secondary" onClick={() => openCommunity("all")}>
                  去社区找皮肤
                </button>
              </Row>
            )}
          </GroupList>
          <GroupList title="自定义主题">
            {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop !== undefined && (
              <SwitchRow
                title="候选栏使用主题配色"
                description="关闭时候选栏和按键一起使用键盘皮肤的颜色；打开后使用本页的主题和候选颜色。"
                aria-label="候选栏使用主题配色"
                disabled={mobileKeyboardFeedbackBusy}
                checked={mobileKeyboardFeedback.candidatePaletteFollowsDesktop}
                onChange={(checked) =>
                  void saveMobileKeyboardFeedback({
                    ...mobileKeyboardFeedback,
                    candidatePaletteFollowsDesktop: checked,
                  })
                }
              />
            )}
            {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop === false && (
              <CandidatePaletteFallbackNotice />
            )}
            {/* Choosing a colour makes the theme custom, over whatever theme was on screen (see `onCandidateColorChange`). */}
            <CandidateColorsSection
              preferences={customColors}
              previewTheme={candidatePreviewTheme}
              showRowColors={showCandidateRowColors}
              showSelectionAppearance={showCandidateSelectionAppearance}
              showBorderColor={showCandidateBorderColor}
              linux={linuxPlatform}
              onChange={onCandidateColorChange}
            />
            {client.customTouchKeyboardSkins && (
              <ScreenKeyboardSkinsSection
                theme={keyboardPreviewTheme}
                selected={customKeyboardSelected}
                customDesign={customTouchKeyboardSkin}
                editorOpen={showTouchSkinEditor}
                // The card previews the editor's design, so choosing it stores that design; without one the custom theme would draw its base's keyboard.
                onSelect={() => onCustomKeyboardChange(customTouchKeyboardSkin)}
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
                  onChange={(design) => onCustomThemeChange({ keyboard: design })}
                  onUse={onUseCustomKeyboard}
                  onClose={() => setShowTouchSkinEditor(false)}
                />
              </div>
            )}
          </GroupList>
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
      {candidateSkins && publishSkinId && page === "skin" && !hidden && (
        <CandidateSkinPublishDialog
          client={candidateSkins}
          localSkins={client.scanSkinCatalog}
          initialSkinId={publishSkinId}
          openSkinDirectory={client.openSkinDirectory}
          readImage={client.readSkinImage}
          onClose={() => setPublishSkinId(null)}
          onPublished={(skin) => {
            setPublishSkinId(null);
            setPublished(skin.visibility);
          }}
          onLogin={() => {
            setPublishSkinId(null);
            openAccountLogin();
          }}
        />
      )}
    </>
  );
}
