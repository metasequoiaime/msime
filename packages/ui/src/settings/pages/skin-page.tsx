import { useState } from "react";
import type { ThemeMode } from "../../index";
import * as settings from "../settings-style";
import { SettingsGroupBlock } from "../settings-group-block";
import { globalThemeDescription } from "../settings-options";
import {
  offeredThemeCatalog,
  themeEntry,
  customCandidateStyle,
  themeCandidateStyle,
  keyboardThemeId,
} from "../../theme/global-theme";
import { SkinCandidatePreview } from "../../skin/skin-candidate-preview";
import { SkinToolbarPreview } from "../../skin/skin-toolbar-preview";
import { SkinCardHeader } from "../../skin/skin-card-header";
import { SkinPreviewStage } from "../../skin/skin-preview-stage";
import { SkinPreviewSurface } from "../../skin/skin-preview-surface";
import {
  ExternalSkinCard,
  ExternalSkinDirectoryRow,
  useSkinCatalog,
} from "../../skin/external-skins";
import { CandidateSkinPublishDialog } from "../../community/candidate-skin-publish-dialog";
import type { CandidateSkinVisibility } from "../../community/community-candidate-skins";
import { ScreenKeyboardPreview } from "../../keyboard/screen-keyboard-preview";
import {
  AiSkinGeneration,
  TouchKeyboardSkinEditor,
} from "../../keyboard/touch-keyboard-skin-editor";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { CandidateColorsSection } from "../candidate-colors-section";
import { ThemeSettingsSection } from "../theme-settings-section";
import { ScreenKeyboardSkinsSection } from "../screen-keyboard-skins-section";
import { useSettingsForm } from "../settings-form-context";
import { CandidatePanelLimitSection } from "../candidate-panel-limit-section";
import { SkinPlatformNotice } from "../skin-platform-notice";
import { ActionButton } from "../action-button";
import { ThemeCarousel } from "../theme-carousel";
import { SwitchRow } from "../switch-row";
import { SegmentedRow } from "../segmented-row";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { ActionRow } from "../action-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsExternalMeta } from "../settings-external-meta";
import { SkinGrid } from "../skin-grid";
import { mobilePageTitle } from "../mobile-tab-helpers";
import { pushMobileSettingsState } from "../mobile-navigation";
import { useMobilePopState } from "../use-mobile-pop-state";

const themeModeOptions = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
] as const satisfies readonly { value: ThemeMode; label: string }[];

/** 设置表单的「主题」页（路由 id `skin`，手机上叫「皮肤」）：全局主题、自定义主题的组成，以及各界面的单独设置。`hidden` 让它在桌面外壳以「社区皮肤」标签页顶替时保持挂载。鸿蒙手机上主题是一组键盘缩略图网格，点按即应用，设置项收在下方的「更多选项」里。 */
export function SkinSettingsPage({ hidden = false }: { hidden?: boolean }) {
  const {
    client,
    linuxPlatform,
    mobilePlatform,
    harmonyPlatform,
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
  const shown = page === "skin" && !hidden;
  const skins = useSkinCatalog(
    client.scanSkinCatalog,
    client.openSkinDirectory,
    importsSkin,
    shown,
  );
  const packages = skins.catalog?.packages ?? [];
  // 只列本宿主提供的主题：「原生」只在 iOS 上有。
  const themeCatalog = offeredThemeCatalog(host?.platform);
  // A package is part of the custom theme, so its card is the one in use while the custom theme draws it; the 自定义 card is then the custom theme without a package.
  const skinInUse = globalTheme === "custom" ? (draft.custom_theme?.candidate_skin ?? null) : null;
  const packageInUse = packages.findIndex((skin) => skin.id === skinInUse);
  // 鸿蒙手机画设计里的缩略图网格；其他宿主保留轮播。
  const phoneGrid = harmonyPlatform && mobilePlatform;
  // 触屏外壳把这页的标题叫作「皮肤」，所以页面也这样称呼自己。
  const pageName = mobilePlatform ? mobilePageTitle("skin", "主题") : "主题";
  // 从网格的 AI 图块打开的「AI 皮肤抽卡」是 WebView 返回栈上单独的一个视图，所以系统返回手势能关掉它。
  const { aiSkins, customSkinLibrary } = client;
  const [aiOpen, setAiOpen] = useState(false);
  useMobilePopState(phoneGrid, (event) => setAiOpen(event.state?.skinSubpage === "ai"));
  const openAi = () => {
    pushMobileSettingsState({ page: "skin", skinSubpage: "ai" });
    setAiOpen(true);
  };
  const closeAi = () => window.history.back();
  const colourMode = (
    <GroupList title="明暗">
      <SegmentedRow
        title="颜色模式"
        description="设置窗口和各界面的默认明暗模式"
        aria-label="颜色模式"
        options={themeModeOptions}
        value={themeMode}
        onChange={(theme) => onPreferencesChange({ theme })}
      />
    </GroupList>
  );
  const moreSkins = (
    <GroupList title="更多皮肤">
      {/* 外部皮肤是候选窗口的皮肤文件夹（skin.toml 加图片），要在电脑上整理好再导入；手机上没有人会备好这样的文件夹，皮肤从下面的社区获取，所以手机不显示这一行。HarmonyOS 2in1 是桌面形态，照旧显示。 */}
      {!mobilePlatform && (
        <ExternalSkinDirectoryRow
          skins={skins}
          scannable={!!client.scanSkinCatalog}
          openable={!!client.openSkinDirectory}
          importsSkin={importsSkin}
        />
      )}
      {/* 桌面宿主在本页顶部的「社区皮肤」标签里浏览候选窗口皮肤，并从上面各自的卡片发布；这一行是手机进入「社区」标签里键盘皮肤图库的入口。 */}
      {mobilePlatform && client.communitySkins && (
        <ActionRow
          title="在线皮肤"
          description="看看别人做的键盘皮肤，可以直接试用或保存"
          action={() => openCommunity("all")}
          label="去社区找皮肤"
        />
      )}
    </GroupList>
  );
  const customThemeGroup = (
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
      {/* 候选颜色取色器作用于候选窗的配色。手机的触屏候选栏画的是键盘皮肤的颜色：Android 不读解析出的候选配色，HarmonyOS 手机的候选栏也取键盘调色板（只有 2in1 的候选窗用候选配色，见 KeyboardView.ets `candidateColors`），只有 iOS 在打开上面的「候选栏使用主题配色」后才读它们。所以手机上只在那个开关打开时显示取色器，其余时候它们改了也看不到效果；桌面和 HarmonyOS 2in1 照旧显示。选颜色会让主题变成自定义，叠在当时屏幕上的主题之上（见 `onCandidateColorChange`）。 */}
      {(!mobilePlatform || mobileKeyboardFeedback?.candidatePaletteFollowsDesktop === true) && (
        <CandidateColorsSection
          preferences={customColors}
          previewTheme={candidatePreviewTheme}
          showRowColors={showCandidateRowColors}
          showSelectionAppearance={showCandidateSelectionAppearance}
          showBorderColor={showCandidateBorderColor}
          linux={linuxPlatform}
          onChange={onCandidateColorChange}
        />
      )}
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
        <SettingsGroupBlock>
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
        </SettingsGroupBlock>
      )}
    </GroupList>
  );
  // 每个界面仍可在颜色模式之上单独指定浅色或深色；设计把这些收在主题页的「高级」下。
  const advanced = (
    <ThemeSettingsSection
      preferences={draft}
      mobile={mobilePlatform}
      linux={linuxPlatform}
      floatingToolbar={showFloatingToolbar}
      desktopPanels={desktopPanels}
      onChange={(key, value) => onPreferencesChange({ [key]: value })}
    />
  );
  return (
    <>
      <SettingsPageFieldset disabled={busy} hidden={!shown} ariaLabel={pageName}>
        {/* 手机网格的卡片没有分浅色/深色的预览，用不着这段说明。 */}
        {!phoneGrid && <SkinPlatformNotice mobile={mobilePlatform} linux={linuxPlatform} />}
        {/* 这条说明指出面板会忽略本页的皮肤和颜色，而它们在别处都无法编辑，所以除了「候选窗口」预览下方，这里也保留一份。 */}
        {host?.candidate_panel_limit && (
          <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
        )}
        {phoneGrid ? (
          <>
            <SkinGrid
              themes={themeCatalog}
              globalTheme={globalTheme}
              customTheme={draft.custom_theme}
              packages={packages}
              customDesign={
                client.customTouchKeyboardSkins
                  ? {
                      design: customTouchKeyboardSkin,
                      selected: customKeyboardSelected,
                      // 卡片画的是编辑器里的设计，所以选中它就保存那个设计，与「自定义主题」下的「我的皮肤」卡片一致。
                      onSelect: () => onCustomKeyboardChange(customTouchKeyboardSkin),
                    }
                  : undefined
              }
              keyboardTheme={keyboardPreviewTheme}
              onApply={onPreferencesChange}
              onApplied={(id) => void client.typingStatistics?.recordSkin?.(id)}
              onOpenAi={aiSkins && customSkinLibrary ? openAi : undefined}
            />
            {/* 设计里这页没有设置行；颜色模式、皮肤目录和社区入口、自定义主题和各界面的单独设置仍可在「更多选项」下找到。从「我的 → 社区作品 → 我的设计」进来时编辑器已经打开，它在这个折叠区里，所以折叠区一开始就展开，否则用户只看到网格。 */}
            <GroupList>
              <MoreOptions defaultOpen={showTouchSkinEditor}>
                {colourMode}
                {moreSkins}
                {customThemeGroup}
                {advanced}
              </MoreOptions>
            </GroupList>
          </>
        ) : (
          <>
            {/* 颜色模式排在卡片之前，因为每张卡片的明暗预览都以它为起点。 */}
            {colourMode}
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
                  <article
                    aria-label={entry.title}
                    className={settings.skinCard(selected)}
                    key={id}
                  >
                    <SkinCardHeader
                      title={entry.title}
                      theme={previewTheme}
                      selected={selected}
                      description={globalThemeDescription(entry, linuxPlatform)}
                      actions={
                        <>
                          <ActionButton
                            action={() =>
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
                            ariaChecked={selected}
                            ariaLabel={entry.title}
                            className={settings.skinSwitch(selected)}
                            label={<span className={settings.skinSwitchKnob(selected)} />}
                            role="switch"
                          />
                          {fixedAppearance === null && (
                            <ActionButton
                              action={() =>
                                setSkinPreviewThemes((current) => ({
                                  ...current,
                                  [id]:
                                    (current[id] ?? candidatePreviewTheme) === "dark"
                                      ? "light"
                                      : "dark",
                                }))
                              }
                              className={settings.skinPreviewSwitch}
                              label={previewTheme === "dark" ? "预览浅色" : "预览深色"}
                            />
                          )}
                        </>
                      }
                    />
                    <SkinPreviewSurface
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
                        <SkinPreviewStage>
                          <SkinCandidatePreview orientation="horizontal" />
                        </SkinPreviewStage>
                        <SkinPreviewStage>
                          <SkinCandidatePreview orientation="vertical" />
                        </SkinPreviewStage>
                      </div>
                      {/* A touch host has no floating toolbar; the theme's other surface there is the keyboard, which the 键盘 page used to show a second set of these cards for. */}
                      {mobilePlatform ? (
                        <SkinPreviewStage>
                          <ScreenKeyboardPreview
                            theme={previewTheme}
                            skin={
                              id === "custom" ? keyboardThemeId("custom", draft.custom_theme) : id
                            }
                            customDesign={id === "custom" ? customTouchKeyboardSkin : undefined}
                            compact
                          />
                        </SkinPreviewStage>
                      ) : (
                        !linuxPlatform && (
                          <SkinPreviewStage>
                            <SkinToolbarPreview />
                          </SkinPreviewStage>
                        )
                      )}
                    </SkinPreviewSurface>
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
              <SettingsExternalMeta role="status">
                {published === "private" ? "已保存到你的皮肤库，仅自己可见。" : "已发布到社区。"}
              </SettingsExternalMeta>
            )}
            {moreSkins}
            {customThemeGroup}
            {advanced}
          </>
        )}
      </SettingsPageFieldset>
      {candidateSkins && publishSkinId && shown && (
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
      {phoneGrid && aiOpen && shown && aiSkins && customSkinLibrary && (
        <AiSkinGeneration
          client={aiSkins}
          library={customSkinLibrary}
          communitySkins={client.communitySkins}
          fullScreen
          useLabel="使用这款皮肤"
          // 提案成为自定义主题的键盘设计，方式与「我的皮肤」卡片应用编辑器设计相同；流程交出提案后自行关闭。
          onUse={(design) => {
            onCustomKeyboardChange(design);
            // 提案成为用户自己的设计，「换装达人」徽章把它计为 `custom`，即 Android 键盘皮肤面板为设计记录的 id。
            void client.typingStatistics?.recordSkin?.("custom");
          }}
          onClose={closeAi}
        />
      )}
    </>
  );
}
