import type { Preferences, SettingsClient } from "../index";
import { HomePage, MoreSettingsPage, type HomePageActions } from "../keyboard/home-page";
import { AccountPage } from "../account/account-page";
import { ChatPage } from "../chat/chat-page";
import { CommunityHomePage, CommunityResourcesPage } from "../community/community-resources";
import { CommunitySkinsPage } from "../community/community-skins";
import type { CommunityDestinationView } from "../community/community-destination";
import { TypingStatisticsPage } from "./typing-statistics";
import { VocabularyReviewPage } from "./vocabulary-review";
import type { SettingsPageId } from "./mobile-navigation";
import type { useAccountPageActions } from "./use-account-page-actions";

export interface SettingsStandalonePagesProps {
  page: SettingsPageId;
  draft?: Preferences;
  client: SettingsClient;
  mobile: boolean;
  accountPlatform: "android" | "ios" | "harmony" | undefined;
  keyboardPreviewTheme: "light" | "dark";
  communityView: CommunityDestinationView;
  communityKey: string;
  mobileSecondaryPages: { id: string; title: string; icon: string }[];
  settingsPageSelection: { onOpenPage: (page: string) => void };
  selectHomeScheme: NonNullable<Parameters<typeof HomePage>[0]["onSelectScheme"]>;
  onOpenChat?: () => void;
  accountPageActions: ReturnType<typeof useAccountPageActions>;
  openAccountLogin: () => void;
}

/** Renders settings destinations that do not use the shared preferences form. */
export function SettingsStandalonePages({
  page,
  draft,
  client,
  mobile,
  accountPlatform,
  keyboardPreviewTheme,
  communityView,
  communityKey,
  mobileSecondaryPages,
  settingsPageSelection,
  selectHomeScheme,
  onOpenChat,
  accountPageActions,
  openAccountLogin,
}: SettingsStandalonePagesProps) {
  return (
    <>
      {client.home && draft && page === "home" && (
        <HomePage
          preferences={draft}
          actions={client.home as HomePageActions}
          {...settingsPageSelection}
          onSelectScheme={selectHomeScheme}
          onOpenChat={onOpenChat}
          touchLayout={mobile}
        />
      )}
      {page === "more" && (
        <MoreSettingsPage pages={mobileSecondaryPages} {...settingsPageSelection} />
      )}
      {(client.account || client.appIcon) && page === "account" && (
        <AccountPage
          client={client.account}
          appIcon={client.appIcon}
          platform={accountPlatform}
          mobile={mobile}
          {...accountPageActions}
        />
      )}
      {client.chat && page === "chat" && (
        <ChatPage
          client={client.chat}
          autoFocus={mobile}
          touch={mobile}
          onLogin={openAccountLogin}
        />
      )}
      {client.communitySkins && client.communityResources && page === "community" && (
        <CommunityHomePage
          key={communityKey}
          skins={client.communitySkins}
          resources={client.communityResources}
          theme={keyboardPreviewTheme}
          initialMine={communityView.initialMine}
          initialCategory={communityView.category}
          initialScope={communityView.scope}
          localDictionary={client.dictionary}
          localSkinLibrary={client.customSkinLibrary}
          mobile={mobile}
          onLogin={openAccountLogin}
        />
      )}
      {client.communitySkins && !client.communityResources && page === "community" && (
        <CommunitySkinsPage
          key={communityKey}
          client={client.communitySkins}
          theme={keyboardPreviewTheme}
          localSkinLibrary={client.customSkinLibrary}
          initialMine={communityView.initialMine}
          mobile={mobile}
          onLogin={openAccountLogin}
        />
      )}
      {!client.communitySkins && client.communityResources && page === "community" && (
        <CommunityResourcesPage
          client={client.communityResources}
          kind={communityView.category === "reply" ? "reply" : "dictionary"}
          initialScope={communityView.scope}
          mobile={mobile}
        />
      )}
      {client.typingStatistics && page === "typing-statistics" && (
        <TypingStatisticsPage
          client={client.typingStatistics}
          mobile={mobile}
          platform={client.host?.platform}
          openSystemSettings={client.openSystemKeyboardSettings}
        />
      )}
      {client.vocabularyReview && page === "vocabulary" && (
        <VocabularyReviewPage
          client={client.vocabularyReview}
          mobile={mobile}
          openPanel={client.openVocabulary}
        />
      )}
    </>
  );
}
