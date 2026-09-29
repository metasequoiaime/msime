import type { SettingsClient } from "../index";
import type { SettingsPageId } from "./mobile-navigation";
import type { SettingsDictionaryPageProps } from "./settings-dictionary-page";

type DictionaryPageState = Omit<
  SettingsDictionaryPageProps,
  | "disabled"
  | "hidden"
  | "dictionary"
  | "dictionaryManifest"
  | "platform"
  | "macos"
  | "resetLearnedData"
>;

export interface SettingsPageDictionaryModelOptions extends DictionaryPageState {
  page: SettingsPageId;
  busy: boolean;
  client: SettingsClient;
  macos: boolean;
}

/** Builds the dictionary page props from the page controller's state and host capabilities. */
export function settingsPageDictionaryModel({
  page,
  busy,
  client,
  macos,
  ...state
}: SettingsPageDictionaryModelOptions): SettingsDictionaryPageProps {
  return {
    ...state,
    disabled: busy,
    hidden: page !== "dictionary",
    dictionary: client.dictionary,
    dictionaryManifest: client.dictionaryManifest,
    platform: client.host?.platform,
    macos,
    resetLearnedData: client.resetLearnedData,
  };
}
