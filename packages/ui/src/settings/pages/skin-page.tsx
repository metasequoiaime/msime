import { useEffect, useRef, useState } from "react";
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
  applyCandidateSkin,
  candidateSkinFor,
  removeCandidateSkin,
  skinDrawsIn,
  skinSlot,
} from "../../theme/global-theme";
import { SkinCandidatePreview } from "../../skin/skin-candidate-preview";
import { candidateWindowStyle } from "../../candidate/candidate-window-style";
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
import { AiSkinGeneration } from "../../keyboard/ai-skin-generation";
import { TouchKeyboardSkinEditor } from "../../keyboard/touch-keyboard-skin-editor";
import type { SavedTouchKeyboardSkin } from "../../keyboard/touch-keyboard-skin-design";
import { communitySkinMessage } from "../../community/community-helpers";
import { GroupList, NavRow } from "../../core/platform-controls";
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
import { useAsyncGeneration } from "../use-async-generation";

const themeModeOptions = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
] as const satisfies readonly { value: ThemeMode; label: string }[];

/** 设置表单的「主题」页（路由 id `skin`，手机上叫「皮肤」）：全局主题、自定义主题的组成，以及各界面的单独设置。`hidden` 让它在桌面外壳以「社区皮肤」标签页顶替时保持挂载。鸿蒙手机上主题是一组键盘缩略图网格，点按即应用，网格下方「浏览更多皮肤」打开完整的社区皮肤图库。 */
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
    communityGallery,
    openCommunityGallery,
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
  // 皮肤包是自定义主题的一部分：放在浅色或深色槽位里的包都算正在使用，它们的卡片都打开；这时「自定义」卡片代表的是不带皮肤包的自定义主题。
  const customTheme = draft.custom_theme;
  const skinsInUse =
    globalTheme === "custom"
      ? [customTheme?.candidate_skin, customTheme?.candidate_skin_dark].filter((id): id is string =>
          Boolean(id),
        )
      : [];
  // 轮播停在当前预览明暗下用的那款皮肤上，没有时停在另一个槽位的那款上。
  const previewSkin =
    globalTheme === "custom"
      ? candidateSkinFor(customTheme, candidatePreviewTheme === "dark")
      : null;
  const shownSkin = [previewSkin, ...skinsInUse].find((id) =>
    packages.some((skin) => skin.id === id),
  );
  const packageInUse = packages.findIndex((skin) => skin.id === shownSkin);
  // 应用皮肤时用目录判断原来那款皮肤属于哪个槽位，见 `applyCandidateSkin`。
  const slotOf = (id: string) => {
    const skin = packages.find((entry) => entry.id === id);
    return skin ? skinSlot(skin.base) : undefined;
  };
  // 每种明暗下实际画的皮肤：取那个模式的槽位，包的 base 还得属于那种明暗。目录里还没有的包按 id 显示。
  const drawnSkinName = (dark: boolean) => {
    const id = candidateSkinFor(customTheme, dark);
    if (!id) return null;
    const skin = packages.find((entry) => entry.id === id);
    if (!skin) return id;
    return skinDrawsIn(skin.base, dark) ? skin.name : null;
  };
  const slotSummary =
    skinsInUse.length > 0
      ? ([false, true] as const)
          .map((dark) => {
            const name = drawnSkinName(dark);
            return `${dark ? "深色" : "浅色"}模式${name ? `用「${name}」` : "不用皮肤"}`;
          })
          .join("，") + "。"
      : null;
  // 鸿蒙手机画设计里的缩略图网格；其他宿主保留轮播。
  const phoneGrid = harmonyPlatform && mobilePlatform;
  // 触屏外壳把这页的标题叫作「皮肤」，所以页面也这样称呼自己。
  const pageName = mobilePlatform ? mobilePageTitle("skin", "主题") : "主题";
  // 从网格的 AI 图块打开的「AI 皮肤抽卡」是 WebView 返回栈上单独的一个视图，所以系统返回手势能关掉它。
  const { aiSkins, customSkinLibrary } = client;
  const [aiOpen, setAiOpen] = useState(false);
  // 网格里列出皮肤库的设计（在社区图库获取的皮肤也存在这里）。每次回到网格都重读：从别的标签页回来，或从编辑器、AI 抽卡、社区图库这几个子视图返回，刚获取或刚保存的皮肤就能出现。
  const [savedDesigns, setSavedDesigns] = useState<SavedTouchKeyboardSkin[]>([]);
  const galleryOpen = communityGallery?.category === "skin";
  const gridVisible = phoneGrid && shown && !showTouchSkinEditor && !aiOpen && !galleryOpen;
  const savedDesignsGeneration = useAsyncGeneration(gridVisible, customSkinLibrary);
  useEffect(() => {
    if (!gridVisible || !customSkinLibrary) return;
    const current = savedDesignsGeneration.current;
    customSkinLibrary.load().then(
      (items) => {
        if (savedDesignsGeneration.current === current) setSavedDesigns(items);
      },
      // 皮肤库读不出来时只是少了这几张卡片，内置主题照常显示，与 Android 皮肤页一致。
      () => {
        if (savedDesignsGeneration.current === current) setSavedDesigns([]);
      },
    );
  }, [gridVisible, customSkinLibrary, savedDesignsGeneration]);
  // 社区的第一页也列在网格里，点一下就获取并换上，与 Android 键盘皮肤面板一致；更多的在网格下方「浏览更多皮肤」打开的完整图库里。离线或服务不可用时只是没有这几张卡片。
  const { communitySkins } = client;
  const [communityList, setCommunityList] = useState<SavedTouchKeyboardSkin[]>([]);
  const [communityNotice, setCommunityNotice] = useState("");
  const taking = useRef(false);
  const communityListGeneration = useAsyncGeneration(gridVisible, communitySkins);
  useEffect(() => {
    if (!gridVisible || !communitySkins) return;
    const current = communityListGeneration.current;
    communitySkins.list(0, "", false, null).then(
      (page) => {
        if (communityListGeneration.current === current)
          setCommunityList(page.skins.map(({ id, name, design }) => ({ id, name, design })));
      },
      () => {
        if (communityListGeneration.current === current) setCommunityList([]);
      },
    );
  }, [gridVisible, communitySkins, communityListGeneration]);
  /** 点社区皮肤：和社区页的「获取」一样下载（服务器计入一次下载，设计以社区 id 存进皮肤库），结束试用但不保留，再经表单换上，与「使用」相同。 */
  const takeCommunitySkin = async (skin: SavedTouchKeyboardSkin): Promise<boolean> => {
    if (!communitySkins || taking.current) return false;
    taking.current = true;
    setCommunityNotice("");
    try {
      const result = await communitySkins.download(skin.id, skin.name);
      await communitySkins.finishTrial(result.trial.id, false);
      setSavedDesigns((items) => [
        ...items.filter((item) => item.id.toLowerCase() !== result.skin.id.toLowerCase()),
        result.skin,
      ]);
      onCustomKeyboardChange(result.skin.design);
      return true;
    } catch (failure) {
      // 例如未登录、皮肤库已满十二个设计，`communitySkinMessage` 会写明。
      setCommunityNotice(communitySkinMessage(failure));
      return false;
    } finally {
      taking.current = false;
    }
  };
  // 鸿蒙手机上「设计我的皮肤」同样是返回栈上的一个视图，不在这页里就地展开。从「我的 → 我的设计」进来时没有这一条记录，编辑器直接打开；返回离开这页时记录里没有它，编辑器随之关上，下次进来不会再自己打开。
  useMobilePopState(phoneGrid, (event) => {
    setAiOpen(event.state?.skinSubpage === "ai");
    setShowTouchSkinEditor(event.state?.skinSubpage === "editor");
  });
  const openAi = () => {
    pushMobileSettingsState({ page: "skin", skinSubpage: "ai" });
    setAiOpen(true);
  };
  const closeAi = () => window.history.back();
  const openEditor = () => {
    pushMobileSettingsState({ page: "skin", skinSubpage: "editor" });
    setShowTouchSkinEditor(true);
  };
  const closeEditor = () => {
    if (window.history.state?.skinSubpage === "editor") window.history.back();
    else setShowTouchSkinEditor(false);
  };
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
      {/* 桌面宿主在本页顶部的「社区皮肤」标签里浏览候选窗口皮肤，并从上面各自的卡片发布；这一行在手机上打开本页的完整键盘皮肤图库（触屏宿主的 `openCommunity` 不去「社区」页）。 */}
      {mobilePlatform && client.communitySkins && (
        <ActionRow
          title="在线皮肤"
          description="看看别人做的键盘皮肤，可以直接试用或保存"
          action={() => openCommunity("all")}
          label="浏览更多皮肤"
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
          // 卡片预览的是编辑器里的设计，所以选中它就保存那个设计；没有设计时自定义主题会画它基础主题的键盘。
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
                      // 这页没有「更多选项」，选中后再点一次就是「设计我的皮肤」的入口。
                      onEdit: openEditor,
                    }
                  : undefined
              }
              savedDesigns={savedDesigns}
              onSelectSavedDesign={
                client.customTouchKeyboardSkins ? onCustomKeyboardChange : undefined
              }
              communitySkins={communityList}
              onSelectCommunitySkin={
                client.customTouchKeyboardSkins && communitySkins ? takeCommunitySkin : undefined
              }
              keyboardTheme={keyboardPreviewTheme}
              onApply={onPreferencesChange}
              onApplied={(id) => void client.typingStatistics?.recordSkin?.(id)}
              onOpenAi={aiSkins && customSkinLibrary ? openAi : undefined}
            />
            {communityNotice && (
              <SettingsExternalMeta role="status">{communityNotice}</SettingsExternalMeta>
            )}
            {/* 设计里这页没有设置行，只有网格。颜色模式和各界面的明暗覆盖不提供；社区第一页的皮肤直接列在网格里，网格下方的「浏览更多皮肤」打开完整的社区皮肤图库（搜索、分类、详情、发布和我的作品都在里面）；我的皮肤是网格里的卡片，选中后再点一次打开编辑器（`onEdit`），「我的 → 我的设计」也能进入。 */}
            {client.communitySkins && (
              <GroupList>
                <NavRow
                  variant="me"
                  title="浏览更多皮肤"
                  description="搜索、分类浏览社区皮肤，也可以发布自己的设计"
                  onClick={() =>
                    openCommunityGallery({ category: "skin", scope: "", initialMine: false })
                  }
                />
              </GroupList>
            )}
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
                                  ? // 选「自定义」卡片本身会去掉两个槽位的皮肤包，保留自定义主题的其余部分，画在它自己的底上。
                                    {
                                      global_theme: "custom",
                                      custom_theme: {
                                        ...draft.custom_theme,
                                        candidate_skin: null,
                                        candidate_skin_dark: null,
                                      },
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
                      style={{
                        ...(id === "custom"
                          ? customCandidateStyle(
                              draft.custom_theme?.base,
                              draft.custom_theme?.candidate_colors,
                            )
                          : themeCandidateStyle(id)),
                        // 用户设的窗口圆角和不透明度对每个主题都生效，卡片照实际候选窗画，与 macOS 原生设置页的预览一致。
                        ...candidateWindowStyle(draft),
                      }}
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
                  selected={skinsInUse.includes(skin.id)}
                  windowStyle={draft}
                  // A host that draws one layout judges a skin by that layout, not by a setting it ignores.
                  layout={host?.fixed_candidate_layout ?? draft.candidate_layout ?? "vertical"}
                  // 皮肤包放进它 base 所属的槽位（浅色皮肤给浅色模式，深色皮肤给深色模式，`system` 底两个都放），另一个槽位不动。清单的 base 成为自定义主题的底，即 `resolve()` 画在包下面的那个，所以预览一致，取下皮肤后也保留这个底。
                  onSelect={(id, base) =>
                    onPreferencesChange({
                      global_theme: "custom",
                      custom_theme: applyCandidateSkin(draft.custom_theme, id, base, slotOf),
                    })
                  }
                  onDeselect={(id) =>
                    onPreferencesChange({
                      global_theme: "custom",
                      custom_theme: removeCandidateSkin(draft.custom_theme, id),
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
            {slotSummary && <SettingsExternalMeta>{slotSummary}</SettingsExternalMeta>}
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
      {phoneGrid && showTouchSkinEditor && shown && client.customTouchKeyboardSkins && (
        <TouchKeyboardSkinEditor
          fullScreen
          design={customTouchKeyboardSkin}
          selected={customKeyboardSelected}
          theme={keyboardPreviewTheme}
          disabled={busy}
          library={client.customSkinLibrary}
          aiSkins={client.aiSkins}
          communitySkins={client.communitySkins}
          onChange={(design) => onCustomThemeChange({ keyboard: design })}
          onUse={onUseCustomKeyboard}
          onClose={closeEditor}
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
