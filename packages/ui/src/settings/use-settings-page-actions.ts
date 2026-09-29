import {
  communityDestinationView,
  type CommunityDestination,
} from "../community/community-destination";
import {
  createAboutSettingsActions,
  type CreateAboutSettingsActionsOptions,
} from "./about-settings-actions";
import {
  useAccountPageActions,
  type UseAccountPageActionsOptions,
} from "./use-account-page-actions";
import {
  createHelpcodeSettingsActions,
  type CreateHelpcodeSettingsActionsOptions,
} from "./helpcode-settings-actions";
import {
  createScreenKeyboardActions,
  type CreateScreenKeyboardActionsOptions,
} from "./screen-keyboard-actions";
import {
  createSettingsExternalActions,
  type CreateSettingsExternalActionsOptions,
} from "./settings-external-actions";
import {
  createSettingsNavigationActions,
  type CreateSettingsNavigationActionsOptions,
} from "./settings-navigation-actions";
import {
  createSettingsPageSelection,
  type CreateSettingsPageSelectionOptions,
} from "./settings-page-selection";
import {
  createSettingsReloadAction,
  type CreateSettingsReloadActionOptions,
} from "./settings-reload-action";
import {
  createSettingsStatusActions,
  type CreateSettingsStatusActionsOptions,
} from "./settings-status-actions";
import {
  createShortcutsSettingsActions,
  type CreateShortcutsSettingsActionsOptions,
} from "./shortcuts-settings-actions";
import {
  createUtilitiesSettingsActions,
  type CreateUtilitiesSettingsActionsOptions,
} from "./utilities-settings-actions";
import {
  useSettingsDestinationActions,
  type UseSettingsDestinationActionsOptions,
} from "./use-settings-destination-actions";
import { useSettingsNavigation, type SettingsNavigationOptions } from "./use-settings-navigation";

export interface UseSettingsPageActionsOptions {
  navigation: SettingsNavigationOptions;
  destination: Omit<UseSettingsDestinationActionsOptions, "selectPage">;
  account: Omit<
    UseAccountPageActionsOptions,
    | "hasAccountLoginReturnPage"
    | "finishAccountLogin"
    | "openAbout"
    | "openLocalDesigns"
    | "openCommunity"
  > & {
    openLocalDesignsAvailable?: boolean;
    openCommunityAvailable?: boolean;
  };
  external: CreateSettingsExternalActionsOptions;
  about: Omit<CreateAboutSettingsActionsOptions, "selectPage">;
  screenKeyboard: Omit<CreateScreenKeyboardActionsOptions, "openCommunity">;
  shortcuts: CreateShortcutsSettingsActionsOptions;
  helpcode: CreateHelpcodeSettingsActionsOptions;
  utilities: CreateUtilitiesSettingsActionsOptions;
  navigationActions: Omit<CreateSettingsNavigationActionsOptions, "selectPage">;
  reload: CreateSettingsReloadActionOptions;
  status: CreateSettingsStatusActionsOptions;
  selection: Omit<CreateSettingsPageSelectionOptions, "selectPage">;
  communityDestination: CommunityDestination;
}

/** Composes settings navigation and page callbacks from the shared page state. */
export function useSettingsPageActions({
  navigation: navigationOptions,
  destination: destinationOptions,
  account: accountOptions,
  external: externalOptions,
  about: aboutOptions,
  screenKeyboard: screenKeyboardOptions,
  shortcuts: shortcutsOptions,
  helpcode: helpcodeOptions,
  utilities: utilitiesOptions,
  navigationActions: navigationActionsOptions,
  reload: reloadOptions,
  status: statusOptions,
  selection: selectionOptions,
  communityDestination,
}: UseSettingsPageActionsOptions) {
  const navigation = useSettingsNavigation(navigationOptions);
  const destination = useSettingsDestinationActions({
    ...destinationOptions,
    selectPage: navigation.selectPage,
  });
  const { openLocalDesignsAvailable, openCommunityAvailable, ...accountPageOptions } =
    accountOptions;
  const accountPageActions = useAccountPageActions({
    ...accountPageOptions,
    openLocalDesigns: openLocalDesignsAvailable ? destination.openLocalDesigns : undefined,
    openCommunity: openCommunityAvailable ? destination.openCommunity : undefined,
    hasAccountLoginReturnPage: Boolean(navigationOptions.accountLoginReturnPage),
    finishAccountLogin: navigation.finishAccountLogin,
    openAbout: destination.openAbout,
  });
  const settingsExternalActions = createSettingsExternalActions(externalOptions);
  const aboutSettingsActions = createAboutSettingsActions({
    ...aboutOptions,
    selectPage: navigation.selectPage,
  });
  const screenKeyboardActions = createScreenKeyboardActions({
    ...screenKeyboardOptions,
    openCommunity: destination.openCommunityAll,
  });
  const shortcutsSettingsActions = createShortcutsSettingsActions(shortcutsOptions);
  const helpcodeSettingsActions = createHelpcodeSettingsActions(helpcodeOptions);
  const utilitiesSettingsActions = createUtilitiesSettingsActions(utilitiesOptions);
  const settingsNavigationActions = createSettingsNavigationActions({
    ...navigationActionsOptions,
    selectPage: navigation.selectPage,
  });
  const reloadSettings = createSettingsReloadAction(reloadOptions);
  const settingsStatusActions = createSettingsStatusActions(statusOptions);
  const settingsPageSelection = createSettingsPageSelection({
    ...selectionOptions,
    selectPage: navigation.selectPage,
  });

  return {
    ...navigation,
    ...destination,
    accountPageActions,
    settingsExternalActions,
    aboutSettingsActions,
    screenKeyboardActions,
    shortcutsSettingsActions,
    helpcodeSettingsActions,
    utilitiesSettingsActions,
    settingsNavigationActions,
    reloadSettings,
    settingsStatusActions,
    settingsPageSelection,
    communityView: communityDestinationView(communityDestination),
  } as const;
}

export type UseSettingsPageActionsResult = ReturnType<typeof useSettingsPageActions>;
