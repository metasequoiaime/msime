import { SettingsFeaturePages } from "./settings-feature-pages";
import { SettingsPreferencesForm } from "./settings-preferences-form";
import { useSettingsPageModel, type SettingsPageProps } from "./use-settings-page-model";
import { SettingsMascotIntro, settingsMascotIntro } from "./settings-mascot-intro";
import { SettingsSidebar } from "./settings-sidebar";
import { appThemeStyle, seasonAttr } from "../core/app-theme-style";
import { SetupWarningStrip } from "../keyboard/setup-status-card";
import { KeyboardSwitchGuideHost } from "../keyboard/keyboard-switch-guide";
import { useSettingsWindowInteractions } from "./use-settings-window-interactions";
import { useSettingsShellState } from "./use-settings-shell-state";
import { useSettingsShellNavigation } from "./use-settings-shell-navigation";
import { useSettingsCommunityViews } from "./use-settings-community-views";
import { MobileSettingsTabs } from "./mobile-settings-tabs";
import { SettingsPageHeader } from "./settings-page-header";
import { settingsShellPlatformPresentation } from "./settings-shell-platform-presentation";
import { SettingsShellToasts } from "./settings-shell-toasts";
import { canReloadSettingsPage, isSettingsFormPage } from "./settings-page-visibility";
import { useLayoutEffect } from "react";
import { SettingsPageStatus } from "./settings-page-status";
import { NoticeBanner } from "./notice-banner";
import { WindowTitlebar } from "./window-titlebar";
import * as settings from "./settings-style";
import { FluentIcon } from "../core/fluent-icons";
import { SettingsPluginCommunitySection } from "./settings-plugin-community-section";
import { SettingsSkinEditorSection } from "./settings-skin-editor-section";
import { SettingsSkinCommunitySection } from "./settings-skin-community-section";
import {
  ScreenKeyboardLayoutContext,
  screenKeyboardLayoutFor,
} from "../keyboard/screen-keyboard-preview";
import { createSettingsReloadAction } from "./settings-reload-action";
import { createSettingsPageSelection } from "./settings-page-selection";
import { createSettingsStatusActions } from "./settings-status-actions";
import { createSettingsExternalActions } from "./settings-external-actions";
import { createSettingsNavigationActions } from "./settings-navigation-actions";
import { logo } from "./settings-options";
import { createCommunityLocalDictionary } from "../community/community-resource-actions";
import { recordSkinSafely } from "./typing-statistics-model";

export function SettingsPage(props: SettingsPageProps) {
  // 宿主自己的屏幕键盘布局：没写 `layout` 的键盘预览按它画，手机上不画桌面键盘。
  return (
    <ScreenKeyboardLayoutContext.Provider
      value={screenKeyboardLayoutFor(props.client.host?.platform)}
    >
      <SettingsShell {...props} />
    </ScreenKeyboardLayoutContext.Provider>
  );
}

function SettingsShell(props: SettingsPageProps) {
  const { onReplayOnboarding } = props;
  const model = useSettingsPageModel(props);
  const {
    client,
    confirmation,
    confirm,
    harmonyPlatform,
    mobilePlatform,
    macosPlatform,
    settingsPlatform,
    dark,
    resolvedAppTheme,
    draft,
    busy,
    setError,
    error,
    notice,
    dismissNotice,
    recoveredBackup,
    inputSourceStartup,
    dismissInputSourceStartup,
    page,
    settingsContentRef,
    windowMaximized,
    reload,
    retrySave,
    saveState,
    saveError,
    openExternalUrl,
    platformIssuesUrl,
    openPanel,
    restoreDefaults,
    recoverPreferences,
    dirty,
    availablePages,
    sidebarGroups,
    mobileActiveTab,
    untitledOnPhone,
    mobilePrimaryPages,
    navigationPage,
    selectPage,
    selectMobileTab,
  } = model;
  const {
    pluginView,
    setPluginView,
    skinView,
    setSkinView,
    navQuery,
    setNavQuery,
    titleCollapsedOn,
    setTitleCollapsedOn,
    chatMounted,
    chatFocusRequest,
    setChatFocusRequest,
    pageColumnRef,
    launchPage,
  } = useSettingsShellState(page);
  const shellPresentation = settingsShellPlatformPresentation({
    settingsPlatform,
    mobilePlatform,
    mobileActiveTab,
    page,
    hasWindowControl: Boolean(client.windowControl),
    hasWindowDrag: Boolean(client.beginWindowDrag),
    availablePages,
    untitledOnPhone,
  });
  const {
    harmonyLook,
    harmonyThemePlatform,
    harmonyPhone,
    tabRootPage,
    pushedPage,
    contentFills,
    macShell,
    linuxShell,
    ipadSidebarShown,
    titlebarShown,
    pageTitle,
    collapsingTitle,
    searchInTitlebar,
  } = shellPresentation;
  // 从一个推入页面再推入的页面（反馈 到 帮助）会保留内容列的动画类，所以要重启动画，而不是让它停在已结束的状态。用重启动画而不是按页面给内容列设 key，能让表单页面像在其他宿主上一样保持挂载、保留状态。
  useLayoutEffect(() => {
    const column = pageColumnRef.current;
    if (!pushedPage || !column || typeof column.getAnimations !== "function") return;
    for (const animation of column.getAnimations()) {
      animation.cancel();
      animation.play();
    }
  }, [page, pushedPage]);
  const mascotIntro = mobilePlatform
    ? undefined
    : settingsMascotIntro(page, {
        ...model.introTabs,
        "screen-keyboard": model.handwritingOnScreenKeyboardPage
          ? model.introTabs["screen-keyboard"]
          : "keyboard",
        tools: client.clipboard?.list ? model.introTabs.tools : "clipboard",
      });
  const reloadSettings = createSettingsReloadAction({ dirty, reload, confirm });
  // 社区图库「添加」词库的去处：宿主有命名词库时装成一个独立的词库，否则导入本机词库。「社区」页和功能页里的图库共用。宿主能核对第三方词库时，发布词库前用它拦下来自搜狗细胞词库的词。
  const communityLocalDictionary = createCommunityLocalDictionary(
    client.dictionary,
    client.dictionaryCollections,
  );
  const recordCommunitySkin = (id: string) => {
    recordSkinSafely(client.typingStatistics, id);
  };
  const communityViews = useSettingsCommunityViews({
    page,
    mobile: mobilePlatform,
    harmony: harmonyPlatform,
    skinView,
    onSkinViewChange: setSkinView,
    pluginView,
    hasCommunitySkins: Boolean(client.communitySkins),
    hasCommunityCandidateSkins: Boolean(client.communityCandidateSkins),
    hasCommunityPlugins: Boolean(client.communityPlugins),
    hasCandidateSkinEditor: Boolean(
      model.host?.candidate_skin_editor && client.candidateSkinEditor && !mobilePlatform,
    ),
  });
  const {
    skinTabs,
    desktopTabs: desktopSkinTabs,
    skinCommunityShown,
    skinEditorShown,
    pluginCommunityShown,
  } = communityViews;
  const { onOpenPage } = createSettingsPageSelection({ selectPage });
  const statusActions = createSettingsStatusActions({
    recoverPreferences,
    inputSourceStartup: client.inputSourceStartup,
    dismissInputSourceStartup,
  });
  const externalActions = createSettingsExternalActions({
    mobile: mobilePlatform,
    canOpenExternalUrl: Boolean(client.openExternalUrl),
    openExternalUrl,
    issuesUrl: platformIssuesUrl,
    openSystemKeyboardSettings: client.openSystemKeyboardSettings,
  });
  const { onOpenChat, onRestoreDefaults } = createSettingsNavigationActions({
    selectPage,
    chatAvailable: Boolean(client.chat),
    openPanel,
    openHandwriting: client.openHandwriting,
    restoreDefaults,
  });
  const { onWindowPointerDownCapture, windowDragHandlers, keepPointer } =
    useSettingsWindowInteractions({
      windowControl: client.windowControl,
      beginWindowDrag: client.beginWindowDrag,
      resizeWindow: client.resizeWindow,
      windowMaximized,
      macShell,
      onError: setError,
    });
  const { parentPage, goBack, shownSidebarGroups, navSearchField, navSearchResults } =
    useSettingsShellNavigation({
      contentRef: pageColumnRef,
      onRevealPage: (target) => {
        if (target === "skin") setSkinView("mine");
        if (target === "plugins") setPluginView("mine");
      },
      page,
      navigationPage,
      launchPage,
      availablePages,
      sidebarGroups,
      navQuery,
      setNavQuery,
      selectPage,
    });
  return (
    <div
      className={settings.shell}
      data-settings-shell=""
      // 平台控件读取的强调色和 `--p-*` token 由这个属性决定。
      data-platform={settingsPlatform}
      // HarmonyOS 应用主题以内联方式在同一个元素上为 token 表重新着色，优先级高于 `[data-platform]` 规则；没有解析出主题时这里为空，沿用 token 表自己的颜色。
      style={
        harmonyThemePlatform
          ? appThemeStyle(resolvedAppTheme, dark, harmonyThemePlatform)
          : undefined
      }
      data-season={seasonAttr(resolvedAppTheme)}
      // 标记手机导航；配色由 `styles.css` 的 `[data-platform]` 规则提供，手机宿主也使用同一套规则。
      data-mobile={mobilePlatform ? "" : undefined}
      onPointerDownCapture={onWindowPointerDownCapture}
    >
      <SettingsShellToasts enabled={harmonyLook}>
        {confirmation}
        <KeyboardSwitchGuideHost setup={client.home?.setup} />
        {titlebarShown && (
          <WindowTitlebar
            linux={linuxShell}
            logo={logo}
            pageTitle={pageTitle}
            search={searchInTitlebar ? navSearchField : undefined}
            maximized={windowMaximized}
            windowControl={client.windowControl}
            dragHandlers={windowDragHandlers}
            keepPointer={keepPointer}
          />
        )}
        <div className={settings.body} data-settings-body="">
          {/* 底部标签栏通过 `order-2` 排在内容下方，DOM 中仍放在内容之前，让辅助技术和键盘焦点先到导航；底部留白避开手势区。超过手机宽度后隐藏，由侧栏承担导航。 */}
          {mobilePlatform && !contentFills && (
            <MobileSettingsTabs
              tabs={mobilePrimaryPages}
              activeTab={mobileActiveTab}
              onSelect={selectMobileTab}
              platform={settingsPlatform}
            />
          )}
          <SettingsSidebar
            presentation={shellPresentation}
            preview={!client.host}
            shownSidebarGroups={shownSidebarGroups}
            navigationPage={navigationPage}
            navSearchField={navSearchField}
            navSearchResults={navSearchResults}
            windowDragHandlers={windowDragHandlers}
            selectPage={selectPage}
          />
          <main
            ref={settingsContentRef}
            id="settings-content"
            className={`${settings.content} ${ipadSidebarShown ? "" : "ipad:col-span-2"} ${contentFills ? "flex flex-col" : ""}`}
            aria-labelledby="page-title"
            onScroll={
              collapsingTitle
                ? (event) => setTitleCollapsedOn(event.currentTarget.scrollTop > 28 ? page : null)
                : undefined
            }
          >
            {collapsingTitle && (
              // 大标题滚出视野后显示紧凑标题。标题文字只作装饰，页面名称仍由下方的 `h1` 提供；返回按钮则承担实际导航，因为原按钮会随大标题滚走，推入页滚动后仍需保留系统手势以外的返回出口。
              <div className={settings.collapsedTitle(titleCollapsedOn === page)}>
                {pushedPage && titleCollapsedOn === page && (
                  <button
                    type="button"
                    className={settings.collapsedBackButton}
                    aria-label="返回"
                    onClick={goBack}
                  >
                    <FluentIcon name="arrow_left" size={22} />
                  </button>
                )}
                <span aria-hidden="true">{pageTitle}</span>
              </div>
            )}
            {macShell && (
              <header className={settings.macosToolbar} data-window-drag="" {...windowDragHandlers}>
                <h1 id="page-title">{pageTitle}</h1>
              </header>
            )}
            <div
              ref={pageColumnRef}
              className={`${settings.contentColumn} ${pushedPage ? "animate-ms-push-in motion-reduce:animate-none" : ""} ${contentFills ? "flex min-h-0 flex-1 flex-col" : ""}`}
              // 统计 和 我的 使用更紧凑的标签页列表，样式表依据这个属性区分。社区虽是标签页的根页，内容是和皮肤页「社区」分类同样的图库而不是设置列表，保持图库原来的版式。
              data-tab-page={
                harmonyPhone && tabRootPage && page !== "home" && page !== "community"
                  ? ""
                  : undefined
              }
            >
              {/* 三个标签页一打开都是已经表明自身的内容（标题行、资料卡、一排数字），源应用不在它们上面印页面标题。标题是隐藏而不是去掉：它为 `main` 提供名称。 */}
              {parentPage && !harmonyPhone && (
                <button
                  type="button"
                  className={settings.backLink}
                  // 名称指明返回目标，避免与侧栏出现两个同名的「反馈」按钮。
                  aria-label={`返回${parentPage.title}`}
                  onClick={() => selectPage(parentPage.id)}
                >
                  <span aria-hidden="true">‹ </span>
                  {parentPage.title}
                </button>
              )}
              {mascotIntro ? (
                <SettingsMascotIntro {...mascotIntro}>
                  {!macShell && (
                    <SettingsPageHeader
                      title={pageTitle}
                      onBack={pushedPage ? goBack : undefined}
                    />
                  )}
                </SettingsMascotIntro>
              ) : (
                !macShell && (
                  <SettingsPageHeader
                    title={pageTitle}
                    hiddenOnPhone={mobilePlatform && untitledOnPhone.includes(page)}
                    onBack={pushedPage ? goBack : undefined}
                    home={page === "home"}
                  />
                )
              )}
              {/* 2-in-1 的根页面上没有设置引导卡片，所以输入法还需要添加或设为默认时，由 输入 和 关于 提示。 */}
              {settingsPlatform === "hm2" &&
                client.home &&
                (page === "input" || page === "about") && (
                  <SetupWarningStrip actions={client.home} />
                )}
              <SettingsPageStatus
                error={error}
                notice={notice}
                onDismissNotice={dismissNotice}
                busy={busy}
                recoveredBackup={recoveredBackup}
                canRecover={Boolean(client.recoverPreferences)}
                onRecover={statusActions.onRecover}
                openPreferencesDirectory={client.openPreferencesDirectory}
                macos={macosPlatform}
                onError={setError}
                draft={draft}
                inputSourceStartup={inputSourceStartup}
                onOpenSettings={statusActions.onOpenSettings}
                onDismiss={statusActions.onDismiss}
                saveError={
                  saveState === "failed" && draft && isSettingsFormPage(page) ? saveError : ""
                }
                onRetrySave={() => void retrySave()}
              />
              {client.notices && (
                <NoticeBanner client={client.notices} openExternalUrl={openExternalUrl} />
              )}
              <SettingsFeaturePages
                model={model}
                chatState={{ chatMounted, chatFocusRequest, setChatFocusRequest }}
                onOpenPage={onOpenPage}
                onOpenChat={onOpenChat}
                onReplayOnboarding={onReplayOnboarding}
                localDictionary={communityLocalDictionary}
                onSkinApplied={recordCommunitySkin}
              />
              <SettingsSkinCommunitySection
                model={model}
                views={communityViews}
                onSkinViewChange={setSkinView}
                onSkinApplied={recordCommunitySkin}
              />
              <SettingsSkinEditorSection
                model={model}
                views={communityViews}
                onSkinViewChange={setSkinView}
              />
              <SettingsPluginCommunitySection
                model={model}
                views={communityViews}
                pluginView={pluginView}
                onPluginViewChange={setPluginView}
              />
              {draft && isSettingsFormPage(page) && (
                <SettingsPreferencesForm
                  model={{ ...model, draft }}
                  skinTabs={skinTabs}
                  desktopSkinTabs={desktopSkinTabs}
                  // 社区图库和「制作皮肤」都画在表单之外，选中其一时表单里的皮肤页隐藏。
                  skinCommunityShown={skinCommunityShown || skinEditorShown}
                  pluginCommunityShown={pluginCommunityShown}
                  externalActions={externalActions}
                  onRestoreDefaults={onRestoreDefaults}
                  reloadSettings={reloadSettings}
                />
              )}
              {/* 表单可见时，加载或保存失败后的「重新读取」位于表单操作行；没有表单或设置加载失败时，这里是唯一的恢复入口。 */}
              {canReloadSettingsPage(page) && !(draft && isSettingsFormPage(page)) && (
                <button className="secondary" disabled={busy} onClick={() => void reloadSettings()}>
                  重新读取
                </button>
              )}
            </div>
          </main>
        </div>
      </SettingsShellToasts>
    </div>
  );
}
