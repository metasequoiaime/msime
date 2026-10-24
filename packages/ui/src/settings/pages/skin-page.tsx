import { candidateSkinSelection } from "../../skin/candidate-skin-selection";
import { BuiltinThemeCard } from "../../skin/builtin-theme-card";
import type { DesktopSkinTab } from "../desktop-skin-tabs";
import { useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import { themeModeOptions } from "../theme-mode-options";
import { recordSkinSafely } from "../typing-statistics-model";
import { SettingsGroupBlock } from "../settings-group-block";
import {
  offeredThemeCatalog,
  applyCandidateSkin,
  removeCandidateSkin,
} from "../../theme/global-theme";
import {
  ExternalSkinCard,
  ExternalSkinDirectoryRow,
  useSkinCatalog,
} from "../../skin/external-skins";
import { CandidateSkinPublishDialog } from "../../community/candidate-skin-publish-dialog";
import type { CandidateSkinVisibility } from "../../community/community-candidate-skin-model";
import { AiSkinGeneration } from "../../keyboard/ai-skin-generation";
import { TouchKeyboardSkinEditor } from "../../keyboard/touch-keyboard-skin-editor";
import type { SavedTouchKeyboardSkin } from "../../keyboard/touch-keyboard-skin-design";
import { GroupList, MoreOptions } from "../../core/platform-controls";
import { CandidateColorsSection } from "../candidate-colors-section";
import { CandidateThemeRow, ThemeSettingsSection } from "../theme-settings-section";
import { ScreenKeyboardSkinsSection } from "../screen-keyboard-skins-section";
import { useSettingsForm } from "../settings-form-context";
import { CandidatePanelLimitSection } from "../candidate-panel-limit-section";
import { SkinPlatformNotice } from "../skin-platform-notice";
import { ThemeCarousel } from "../theme-carousel";
import { SwitchRow } from "../switch-row";
import { SegmentedRow } from "../segmented-row";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { ActionRow } from "../action-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SettingsExternalMeta } from "../settings-external-meta";
import { SkinGrid, type SkinGridTab } from "../skin-grid";
import { mobilePageTitle } from "../mobile-tab-helpers";
import { pushMobileSettingsState } from "../mobile-navigation";
import { useMobilePopState } from "../use-mobile-pop-state";
import { useMobileSettingsSubview } from "../use-mobile-settings-subview";
import { useAsyncGeneration } from "../use-async-generation";

/** 设置表单的「主题」页（路由 id `skin`，手机上叫「皮肤」）：全局主题、自定义主题的组成，以及各界面的单独设置。`hidden` 让它在桌面外壳以「社区皮肤」标签页顶替时保持挂载。鸿蒙手机上主题按顶部的「内置 / 我的 / 社区」分成键盘缩略图网格，点按即应用；标签栏和「社区」的完整图库由设置外壳画在表单之外，这里只画前两个分类的网格（`tabs`）。 */
export function SkinSettingsPage({
  hidden = false,
  tabs,
  desktopTabs,
}: {
  hidden?: boolean;
  /** 设置外壳里的皮肤分类标签（只在鸿蒙手机上启用）：把一个分类的内容包成对应的标签面板，没选中的分类随之隐藏。 */
  tabs: { panel: (tab: SkinGridTab, children: ReactNode) => ReactNode };
  desktopTabs: {
    selected: DesktopSkinTab;
    panel: (tab: DesktopSkinTab, children: ReactNode) => ReactNode;
  };
}) {
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
    settingsContentRef,
    mobileKeyboardFeedback,
    mobileKeyboardFeedbackBusy,
    skinPreviewThemes,
    setSkinPreviewThemes,
    saveMobileKeyboardFeedback,
    candidatePreviewTheme,
    globalTheme,
    themeMode,
    customColors,
    previewCustomTheme,
    onCandidateColorChange,
    showCandidateRowColors,
    showCandidateSelectionAppearance,
    showCandidateBorderColor,
    showCandidateNumberColor,
    surfaceThemesFollow,
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
  } = useSettingsForm();
  useLayoutEffect(() => {
    if (page === "skin" && !mobilePlatform && settingsContentRef.current) {
      settingsContentRef.current.scrollTop = 0;
    }
  }, [desktopTabs.selected, page, mobilePlatform, settingsContentRef]);
  const { onCustomThemeChange, onPreferencesChange } = createSettingsDraftActions({ setDraft });
  // 发布对话框放在 fieldset 外，避免保存设置时禁用发布操作。
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
  const { skinsInUse, packageInUse, slotOf, slotSummary } = candidateSkinSelection({
    globalTheme,
    customTheme: draft.custom_theme,
    packages,
    previewTheme: candidatePreviewTheme,
  });
  // 鸿蒙手机画设计里的缩略图网格；其他宿主保留轮播。
  const phoneGrid = harmonyPlatform && mobilePlatform;
  // 触屏外壳把这页的标题叫作「皮肤」，所以页面也这样称呼自己。
  const pageName = mobilePlatform ? mobilePageTitle("skin", "主题") : "主题";
  // 从网格的 AI 图块打开的「AI 皮肤抽卡」是 WebView 返回栈上单独的一个视图，所以系统返回手势能关掉它。
  const { aiSkins, customSkinLibrary } = client;
  const {
    value: aiSubview,
    open: openAiSubview,
    close: closeAi,
  } = useMobileSettingsSubview<"ai">({
    mobile: phoneGrid,
    stateKey: "skinSubpage",
    decode: (state, current) =>
      state?.skinSubpage === "ai" && (current === null || state.skinSubpage === current)
        ? "ai"
        : null,
    encode: (value) => ({ page: "skin", skinSubpage: value }),
  });
  const aiOpen = aiSubview === "ai";
  // 「我的」里列出皮肤库的设计（在社区获取的皮肤也存在这里）。每次回到网格都重读：从「社区」分类或别的标签页回来，或从编辑器、AI 抽卡、社区图库这几个子视图返回，刚获取或刚保存的皮肤就能出现。
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
  // 鸿蒙手机上「设计我的皮肤」同样是返回栈上的一个视图，不在这页里就地展开。从「我的 → 我的设计」进来时没有这一条记录，编辑器直接打开；返回离开这页时记录里没有它，编辑器随之关上，下次进来不会再自己打开。
  useMobilePopState(phoneGrid, (event) => {
    setShowTouchSkinEditor(event.state?.skinSubpage === "editor");
  });
  const openAi = () => openAiSubview("ai");
  const openEditor = () => {
    pushMobileSettingsState({ page: "skin", skinSubpage: "editor" });
    setShowTouchSkinEditor(true);
  };
  const closeEditor = () => {
    if (window.history.state?.skinSubpage === "editor") window.history.back();
    else setShowTouchSkinEditor(false);
  };
  // 「内置」和「我的」两个分类的网格共用同一组卡片数据，选中状态按全部卡片判断。
  const gridProps = {
    themes: themeCatalog,
    globalTheme,
    customTheme: draft.custom_theme,
    packages,
    customDesign: client.customTouchKeyboardSkins
      ? {
          design: customTouchKeyboardSkin,
          selected: customKeyboardSelected,
          // 卡片画的是编辑器里的设计，所以选中它就保存那个设计，与「自定义主题」下的「我的皮肤」卡片一致。
          onSelect: () => onCustomKeyboardChange(customTouchKeyboardSkin),
          // 这页没有「更多选项」，选中后再点一次就是「设计我的皮肤」的入口。
          onEdit: openEditor,
        }
      : undefined,
    savedDesigns,
    onSelectSavedDesign: client.customTouchKeyboardSkins ? onCustomKeyboardChange : undefined,
    keyboardTheme: keyboardPreviewTheme,
    onApply: onPreferencesChange,
    onApplied: (id: string) => recordSkinSafely(client.typingStatistics, id),
    onOpenAi: aiSkins && customSkinLibrary ? openAi : undefined,
  };
  const colourMode = (
    <GroupList title="明暗">
      <SegmentedRow
        title="颜色模式"
        // 各界面一律跟随颜色模式的宿主上它不再是可被各界面覆盖的默认值，而是除候选窗口以外所有界面的明暗。
        description={
          surfaceThemesFollow
            ? "设置窗口和各界面的明暗模式；候选窗口可在下面单独设置"
            : "设置窗口和各界面的默认明暗模式"
        }
        aria-label="颜色模式"
        options={themeModeOptions}
        value={themeMode}
        onChange={(theme) => onPreferencesChange({ theme })}
      />
      {/* 各界面一律跟随颜色模式的宿主（macOS、Windows、Linux 和 HarmonyOS 2in1）只剩候选窗口还能单独设明暗，放在颜色模式下面，主题页因此没有「高级」组。 */}
      {surfaceThemesFollow && (
        <CandidateThemeRow
          mobile={mobilePlatform}
          linux={linuxPlatform}
          value={draft.candidate_theme}
          onChange={(candidate_theme) => onPreferencesChange({ candidate_theme })}
        />
      )}
    </GroupList>
  );
  const moreSkins = (
    <GroupList title={mobilePlatform ? "更多皮肤" : "本地皮肤"}>
      {/* 外部皮肤是候选窗口的皮肤文件夹（skin.toml 加图片），要在电脑上整理好再导入；手机上没有人会备好这样的文件夹，皮肤从下面的社区获取，所以手机不显示这一行。HarmonyOS 2in1 是桌面形态，照旧显示。 */}
      {!mobilePlatform && (
        <ExternalSkinDirectoryRow
          skins={skins}
          scannable={!!client.scanSkinCatalog}
          openable={!!client.openSkinDirectory}
          importsSkin={importsSkin}
        />
      )}
      {/* 桌面宿主在本页顶部的「社区皮肤」标签里浏览候选窗口皮肤，并从上面各自的卡片发布；这一行在手机上切到底部「社区」标签的「皮肤」分类，那里是完整的键盘皮肤图库。 */}
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
      {/* 取色器收在「自定义配色」折叠区里：内置主题和皮肤包已经定好配色，多数人不需要逐个调。悬停色也在里面，系统底色上不会自动推出悬停色，深色自定义表面只能靠它看清悬停。 */}
      {(!mobilePlatform || mobileKeyboardFeedback?.candidatePaletteFollowsDesktop === true) && (
        <MoreOptions title="自定义配色">
          <CandidateColorsSection
            preferences={customColors}
            previewTheme={candidatePreviewTheme}
            showRowColors={showCandidateRowColors}
            showSelectionAppearance={showCandidateSelectionAppearance}
            showBorderColor={showCandidateBorderColor}
            showNumberColor={showCandidateNumberColor}
            linux={linuxPlatform}
            onChange={onCandidateColorChange}
          />
        </MoreOptions>
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
  // 每个界面仍可在颜色模式之上单独指定浅色或深色；设计把这些收在主题页的「高级」下。各界面一律跟随颜色模式的宿主不给这一组，候选窗口那一行在「明暗」组里。
  const advanced = !surfaceThemesFollow && (
    <ThemeSettingsSection
      preferences={draft}
      mobile={mobilePlatform}
      linux={linuxPlatform}
      floatingToolbar={showFloatingToolbar}
      desktopPanels={desktopPanels}
      onChange={(key, value) => onPreferencesChange({ [key]: value })}
    />
  );
  const builtinCards = themeCatalog.map((entry) => (
    <BuiltinThemeCard
      key={entry.id}
      entry={entry}
      globalTheme={globalTheme}
      packageSelected={packageInUse >= 0}
      preferences={draft}
      previewCustomTheme={previewCustomTheme}
      candidatePreviewTheme={candidatePreviewTheme}
      previewTheme={skinPreviewThemes[entry.id]}
      keyboardDesign={customTouchKeyboardSkin}
      mobile={mobilePlatform}
      linux={linuxPlatform}
      onPreferencesChange={onPreferencesChange}
      onTogglePreview={() =>
        setSkinPreviewThemes((current) => ({
          ...current,
          [entry.id]: (current[entry.id] ?? candidatePreviewTheme) === "dark" ? "light" : "dark",
        }))
      }
    />
  ));
  const localCards = packages.map((skin) => (
    <ExternalSkinCard
      key={`package:${skin.id}`}
      skin={skin}
      selected={skinsInUse.includes(skin.id)}
      windowStyle={draft}
      // 固定候选布局的宿主按实际布局判断皮肤支持情况。
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
  ));
  return (
    <>
      <SettingsPageFieldset disabled={busy} hidden={!shown} ariaLabel={pageName}>
        {/* 桌面页首的插画引导已经说明本页管什么、布局在哪里改，这里不再重复一段。 */}
        {/* 手机网格的卡片没有分浅色/深色的预览，用不着这段说明。 */}
        {!phoneGrid && <SkinPlatformNotice mobile={mobilePlatform} linux={linuxPlatform} />}
        {/* 这条说明指出面板会忽略本页的皮肤和颜色，而它们在别处都无法编辑，所以除了「候选窗口」预览下方，这里也保留一份。 */}
        {host?.candidate_panel_limit && (
          <CandidatePanelLimitSection limit={host.candidate_panel_limit} />
        )}
        {phoneGrid ? (
          <>
            {/* 设计里这页没有设置行，只有分类网格。颜色模式和各界面的明暗覆盖不提供；我的皮肤是「我的」里的卡片，选中后再点一次打开编辑器（`onEdit`），「我的 → 我的设计」也能进入。 */}
            {tabs.panel("builtin", <SkinGrid section="builtin" {...gridProps} />)}
            {tabs.panel(
              "mine",
              <>
                <SkinGrid section="mine" {...gridProps} />
                {/* 皮肤库里还没有设计时说明去哪里获取，与 Android 皮肤页「我的」的空状态一致。 */}
                {savedDesigns.length === 0 && (
                  <SettingsExternalMeta>
                    还没有保存的皮肤。可到社区获取喜欢的皮肤。
                  </SettingsExternalMeta>
                )}
              </>,
            )}
          </>
        ) : (
          desktopTabs.panel(
            "mine",
            <>
              {/* 内置主题与已安装皮肤统一选择，颜色模式决定它们的默认预览明暗。 */}
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
                {builtinCards}
                {localCards}
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
            </>,
          )
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
            recordSkinSafely(client.typingStatistics, "custom");
          }}
          onClose={closeAi}
        />
      )}
    </>
  );
}
