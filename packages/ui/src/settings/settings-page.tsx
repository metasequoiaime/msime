import { SettingsPreferencesForm } from "./settings-preferences-form";
import { useSettingsPageModel, type SettingsPageProps } from "./use-settings-page-model";
import { SettingsSidebar } from "./settings-sidebar";
import { appThemeStyle, seasonAttr } from "../core/app-theme-style";
import { SetupWarningStrip } from "../keyboard/setup-status-card";
import { KeyboardSwitchGuideHost } from "../keyboard/keyboard-switch-guide";
import { TryKeyboardPage } from "../keyboard/try-keyboard-page";
import { desktopDownloadUrl } from "./app-resources";
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
import { communityGalleryPage } from "./use-community-gallery-subview";
import { CommunityGallerySubview } from "./community-gallery-subview";
import { TypingStatisticsPage } from "./typing-statistics";
import { VocabularyReviewPage } from "./vocabulary-review";
import * as settings from "./settings-style";
import { FluentIcon } from "../core/fluent-icons";
import { AccountPage } from "../account/account-page";
import { ChatPage } from "../chat/chat-page";
import { HomePage } from "../keyboard/home-page";
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
import { CommunitySkinsPage } from "../community/community-skins";
import { createCommunityLocalDictionary } from "../community/community-resource-actions";
import { CommunityResourcesPage } from "../community/community-resources";
import { CommunityPluginsPage } from "../community/community-plugins";
import * as communityStyle from "../community/community-style";
import { CommunityPage } from "../community/community-page";

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
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    macosPlatform,
    settingsPlatform,
    dark,
    resolvedAppTheme,
    draft,
    setDraft,
    busy,
    setError,
    error,
    notice,
    recoveredBackup,
    inputSourceStartup,
    dismissInputSourceStartup,
    page,
    dictionaryTab,
    setDictionaryTab,
    accountLoginReturnPage,
    settingsContentRef,
    communityDestination,
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
    keyboardPreviewTheme,
    surfaceThemesFollow,
    availablePages,
    sidebarGroups,
    mobileActiveTab,
    untitledOnPhone,
    mobilePrimaryPages,
    mobileSecondaryGroups,
    navigationPage,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
    openLocalDesigns,
    openCommunity,
    initialCommunityCategory,
    initialCommunityScope,
    initialCommunityMine,
    communityGallery,
    closeCommunityGallery,
    communityTabs,
    currentAppVersion,
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
  const reloadSettings = createSettingsReloadAction({ dirty, reload, confirm });
  // 社区图库「添加」词库的去处：宿主有命名词库时装成一个独立的词库，否则导入本机词库。「社区」页和功能页里的图库共用。宿主能核对第三方词库时，发布词库前用它拦下来自搜狗细胞词库的词。
  const communityLocalDictionary = createCommunityLocalDictionary(
    client.dictionary,
    client.dictionaryCollections,
  );
  const recordCommunitySkin = (id: string) => {
    void client.typingStatistics?.recordSkin?.(id);
  };
  const {
    skinTabs,
    desktopTabs: desktopSkinTabs,
    phoneSkinCommunityShown,
    skinCommunityShown,
    pluginCommunityShown,
  } = useSettingsCommunityViews({
    page,
    mobile: mobilePlatform,
    harmony: harmonyPlatform,
    skinView,
    onSkinViewChange: setSkinView,
    pluginView,
    hasCommunitySkins: Boolean(client.communitySkins),
    hasCommunityCandidateSkins: Boolean(client.communityCandidateSkins),
    hasCommunityPlugins: Boolean(client.communityPlugins),
  });
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
  const {
    parentPage,
    goBack,
    shownSidebarGroups,
    navSearchField,
    isSidebarGroupExpanded,
    toggleSidebarGroup,
  } = useSettingsShellNavigation({
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
            mobilePlatform={mobilePlatform}
            preview={!client.host}
            shownSidebarGroups={shownSidebarGroups}
            navigationPage={navigationPage}
            navSearchField={navSearchField}
            windowDragHandlers={windowDragHandlers}
            selectPage={selectPage}
            isSidebarGroupExpanded={isSidebarGroupExpanded}
            toggleSidebarGroup={toggleSidebarGroup}
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
              {!macShell && (
                <SettingsPageHeader
                  title={pageTitle}
                  hiddenOnPhone={mobilePlatform && untitledOnPhone.includes(page)}
                  onBack={pushedPage ? goBack : undefined}
                  home={page === "home"}
                />
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
              {client.home && draft && page === "home" && (
                <HomePage
                  preferences={draft}
                  actions={client.home}
                  onOpenPage={onOpenPage}
                  onOpenChat={
                    onOpenChat &&
                    (() => {
                      setChatFocusRequest((count) => count + 1);
                      onOpenChat();
                    })
                  }
                  touchLayout={mobilePlatform}
                  ios={iosPlatform}
                  keyboardSurfaceTheme={
                    surfaceThemesFollow ? undefined : draft.screen_keyboard_theme
                  }
                  rootPages={mobileSecondaryGroups
                    .flatMap((group) => group.pages)
                    .map((item) => ({ id: item.id, title: item.title }))}
                />
              )}
              {client.home && page === "try-keyboard" && (
                <TryKeyboardPage
                  actions={client.home}
                  switchAway={!harmonyPlatform}
                  chat={client.chat}
                  onLogin={openAccountLogin}
                  onOpenUrl={
                    client.openExternalUrl
                      ? (url: string) => {
                          void openExternalUrl(url);
                        }
                      : undefined
                  }
                />
              )}
              {(client.account || client.appIcon) && page === "account" && (
                <AccountPage
                  client={client.account}
                  appIcon={client.appIcon}
                  platform={
                    androidPlatform
                      ? "android"
                      : iosPlatform
                        ? "ios"
                        : harmonyPlatform
                          ? "harmony"
                          : undefined
                  }
                  mobile={mobilePlatform}
                  onCancelLogin={accountLoginReturnPage ? finishAccountLogin : undefined}
                  onLoginComplete={accountLoginReturnPage ? finishAccountLogin : undefined}
                  onOpenLocalDesigns={
                    client.customTouchKeyboardSkins ? openLocalDesigns : undefined
                  }
                  // Windows 的社区页只有词包与回复模板，没有皮肤画廊；它的候选窗口皮肤社区在主题页。
                  onOpenCommunity={client.communityResources ? openCommunity : undefined}
                  communityHasSkins={Boolean(client.communitySkins)}
                  onOpenCloudDictionary={
                    !mobilePlatform && client.cloudDictionary
                      ? () => {
                          selectPage("dictionary");
                          setDictionaryTab("cloud");
                        }
                      : client.openCloudDictionary && !macosPlatform
                        ? () => {
                            void client.openCloudDictionary!().catch(() =>
                              setError("无法打开云词库，请重试。"),
                            );
                          }
                        : undefined
                  }
                  // macOS 只从输入法菜单打开云剪贴板，由菜单提供粘贴目标的输入会话。
                  onOpenCloudClipboard={
                    client.openCloudClipboard && !macosPlatform
                      ? () => {
                          void client.openCloudClipboard!().catch(() =>
                            setError("无法打开云剪贴板，请重试。"),
                          );
                        }
                      : undefined
                  }
                  onOpenAbout={mobilePlatform ? () => selectPage("about") : undefined}
                  onOpenFeedback={
                    mobilePlatform && availablePages.some((item) => item.id === "feedback")
                      ? () => selectPage("feedback")
                      : undefined
                  }
                  onOpenDesktopDownload={
                    mobilePlatform && client.openExternalUrl
                      ? () => {
                          void openExternalUrl(desktopDownloadUrl);
                        }
                      : undefined
                  }
                  onReplayOnboarding={mobilePlatform ? onReplayOnboarding : undefined}
                  preferences={draft}
                  onOpenPage={selectPage}
                  appTheme={client.appTheme}
                  onOpenUrl={
                    client.openExternalUrl
                      ? (url: string) => {
                          void openExternalUrl(url);
                        }
                      : undefined
                  }
                  copyText={client.copyText}
                  desktopDownloadUrl={desktopDownloadUrl}
                  appVersion={currentAppVersion}
                />
              )}
              {/* 触屏上 AI 对话从「AI 辅助」二级页进入：第一次打开后一直挂着，切换标签只是隐藏，对话记录和进行中的回复都不丢。 */}
              {client.chat && (page === "chat" || (mobilePlatform && chatMounted)) && (
                <div className={page === "chat" ? "contents" : undefined} hidden={page !== "chat"}>
                  <ChatPage
                    client={client.chat}
                    touch={mobilePlatform}
                    active={page === "chat"}
                    focusRequest={chatFocusRequest}
                    setup={client.home}
                    onLogin={openAccountLogin}
                  />
                </div>
              )}
              {page === "community" && !mobilePlatform && (
                <CommunityPage
                  destinationKey={communityDestination}
                  skins={client.communitySkins}
                  resources={client.communityResources}
                  theme={keyboardPreviewTheme}
                  initialMine={initialCommunityMine}
                  initialCategory={initialCommunityCategory}
                  initialScope={initialCommunityScope}
                  localDictionary={communityLocalDictionary}
                  localSkinLibrary={client.customSkinLibrary}
                  mobile={mobilePlatform}
                  onLogin={openAccountLogin}
                  preferences={draft}
                  // 从皮肤库应用的皮肤和其他偏好修改一样：先进入草稿，再由表单的自动保存写入。
                  onApplyPreferences={setDraft}
                  onSkinApplied={recordCommunitySkin}
                  look={harmonyPlatform && mobilePlatform ? "harmony" : undefined}
                />
              )}
              {/* 触屏的「社区」标签根页：顶部「皮肤 / 词库」分类，每类是完整的社区图库（搜索、分类、详情、获取或添加、举报、发布都在里面），只挂载选中的那一类。和皮肤页的「社区」分类一样画在设置表单之外：图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交。详情的历史记录留在这一页，返回先关详情。 */}
              {page === "community" && mobilePlatform && communityTabs.tabList}
              {page === "community" && mobilePlatform && (
                <div className={communityTabs.tabList ? "mt-6" : undefined}>
                  {communityTabs.selected === "skin" &&
                    client.communitySkins &&
                    communityTabs.panel(
                      "skin",
                      <CommunitySkinsPage
                        client={client.communitySkins}
                        theme={keyboardPreviewTheme}
                        localSkinLibrary={client.customSkinLibrary}
                        mobile
                        onLogin={openAccountLogin}
                        look={harmonyPlatform ? "harmony" : undefined}
                        preferences={draft}
                        // 「使用」和皮肤页的网格一样先进入草稿，再由表单的自动保存写入。
                        onApplyPreferences={setDraft}
                        onSkinApplied={recordCommunitySkin}
                        historyPage="community"
                      />,
                    )}
                  {communityTabs.selected === "dictionary" &&
                    client.communityResources &&
                    communityTabs.panel(
                      "dictionary",
                      <CommunityResourcesPage
                        client={client.communityResources}
                        kind="dictionary"
                        localDictionary={communityLocalDictionary}
                        mobile
                        look={harmonyPlatform ? "harmony" : undefined}
                        historyPage="community"
                      />,
                    )}
                </div>
              )}
              {/* 触屏宿主在功能页里打开的完整社区图库。画在设置表单之外：图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交。 */}
              {communityGallery && page === communityGalleryPage(communityGallery.category) && (
                <CommunityGallerySubview
                  key={`${communityGallery.category}:${communityGallery.scope}:${communityGallery.initialMine}`}
                  view={communityGallery}
                  skins={client.communitySkins}
                  resources={client.communityResources}
                  theme={keyboardPreviewTheme}
                  localDictionary={communityLocalDictionary}
                  localSkinLibrary={client.customSkinLibrary}
                  onLogin={openAccountLogin}
                  preferences={draft}
                  onApplyPreferences={setDraft}
                  onSkinApplied={recordCommunitySkin}
                  look={harmonyPlatform && mobilePlatform ? "harmony" : undefined}
                  onClose={closeCommunityGallery}
                />
              )}
              {client.typingStatistics && page === "typing-statistics" && (
                <TypingStatisticsPage
                  client={client.typingStatistics}
                  mobile={mobilePlatform}
                  platform={client.host?.platform}
                  openSystemSettings={client.openSystemKeyboardSettings}
                  look={
                    settingsPlatform === "harmony"
                      ? "harmony"
                      : settingsPlatform === "hm2"
                        ? "hm2"
                        : undefined
                  }
                />
              )}
              {client.vocabularyReview && page === "vocabulary" && (
                <VocabularyReviewPage
                  client={client.vocabularyReview}
                  mobile={mobilePlatform}
                  openPanel={client.openVocabulary}
                />
              )}
              {page === "skin" && desktopSkinTabs.tabList}
              {page === "skin" && skinTabs.tabList}
              {phoneSkinCommunityShown && client.communitySkins && (
                <div className="mt-6">
                  {skinTabs.panel(
                    "community",
                    <CommunitySkinsPage
                      client={client.communitySkins}
                      theme={keyboardPreviewTheme}
                      localSkinLibrary={client.customSkinLibrary}
                      mobile
                      onLogin={openAccountLogin}
                      look="harmony"
                      preferences={draft}
                      // 「使用」和网格里的卡片一样先进入草稿，再由表单的自动保存写入。
                      onApplyPreferences={setDraft}
                      onSkinApplied={recordCommunitySkin}
                      // 详情的历史记录留在皮肤页上，返回先关详情、再离开这一页。
                      historyPage="skin"
                    />,
                  )}
                </div>
              )}
              {skinCommunityShown &&
                client.communityCandidateSkins &&
                desktopSkinTabs.panel(
                  "community",
                  <div className="mt-4">
                    <CommunityPage
                      candidateSkins={client.communityCandidateSkins}
                      localSkins={client.scanSkinCatalog}
                      openSkinDirectory={client.openSkinDirectory}
                      readSkinImage={client.readSkinImage}
                      onOpenSkinPage={() => setSkinView("mine")}
                      theme={keyboardPreviewTheme}
                      onLogin={openAccountLogin}
                    />
                  </div>,
                )}
              {page === "plugins" && client.communityPlugins && (
                <div
                  className={communityStyle.categoryTabsPair}
                  role="tablist"
                  aria-label="插件来源"
                >
                  <button
                    type="button"
                    role="tab"
                    aria-selected={pluginView === "mine"}
                    onClick={() => setPluginView("mine")}
                  >
                    我的插件
                  </button>
                  <button
                    type="button"
                    role="tab"
                    aria-selected={pluginView === "community"}
                    onClick={() => setPluginView("community")}
                  >
                    社区插件
                  </button>
                </div>
              )}
              {client.communityPlugins && pluginCommunityShown && (
                <div className="mt-4">
                  <CommunityPluginsPage
                    client={client.communityPlugins}
                    localPlugins={client.plugins?.catalog}
                    onLogin={openAccountLogin}
                  />
                </div>
              )}
              {draft && isSettingsFormPage(page) && (
                <SettingsPreferencesForm
                  model={{ ...model, draft }}
                  skinTabs={skinTabs}
                  desktopSkinTabs={desktopSkinTabs}
                  skinCommunityShown={skinCommunityShown}
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
