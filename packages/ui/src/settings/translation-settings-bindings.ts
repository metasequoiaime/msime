import type { TranslationSettingsContentProps } from "./translation-settings-content";
import {
  createCandidateTranslationSettings,
  type CandidateTranslationSettingsOptions,
} from "./translation-candidate-settings";
import {
  createTranslationProviderSettings,
  type TranslationProviderSettingsOptions,
} from "./translation-provider-settings";

export type TranslationSettingsBindingsOptions = CandidateTranslationSettingsOptions &
  Omit<TranslationProviderSettingsOptions, "grouped">;

export type TranslationSettingsBindings = TranslationSettingsContentProps;

/** 组合两个设置宿主共用的候选和翻译服务绑定。 */
export function createTranslationSettingsBindings({
  android,
  client,
  ...options
}: TranslationSettingsBindingsOptions): TranslationSettingsBindings {
  return {
    candidate: createCandidateTranslationSettings({ android, ...options }),
    providers: android ? undefined : createTranslationProviderSettings({ client, ...options }),
  };
}
