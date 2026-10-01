import type { SettingsClient } from "../index";
import type { CustomTranslationsSectionProps } from "./custom-translations-section";
import type { TranslationSettingsContentProps } from "./translation-settings-content";
import {
  createCandidateTranslationSettings,
  type CandidateTranslationSettingsOptions,
} from "./translation-candidate-settings";
import {
  createTranslationProviderSettings,
  type TranslationProviderSettingsOptions,
} from "./translation-provider-settings";
import type { SettingsSaveState } from "./use-settings-persistence";

/** 自定义候选释义编辑器所绑定的内容；它属于「显示英文释义」，而不属于任何一个翻译服务。 */
export interface CustomGlossesSettingsOptions {
  client: Pick<SettingsClient, "customTranslations">;
  mobile: boolean;
  customTranslationsText: string;
  customTranslationsPlaceholder: string;
  customTranslationsNotice: string;
  customTranslationsSummary: string;
  customTranslationsSaveState: SettingsSaveState;
  customTranslationsSaveError: string;
  onCustomTranslationsChange: (value: string) => void;
  onFlushCustomTranslations: () => void;
}

export type TranslationSettingsBindingsOptions = CandidateTranslationSettingsOptions &
  Omit<TranslationProviderSettingsOptions, "grouped" | "client"> &
  Omit<CustomGlossesSettingsOptions, "client"> & {
    client: TranslationProviderSettingsOptions["client"] & CustomGlossesSettingsOptions["client"];
  };

export interface TranslationSettingsBindings extends TranslationSettingsContentProps {
  /** 自定义候选释义编辑器，宿主没有访问释义覆盖文件的途径时不提供。由宿主自行摆放在「显示英文释义」旁边。 */
  customGlosses?: CustomTranslationsSectionProps;
}

/** 组合两个设置宿主共用的候选、翻译服务和自定义释义绑定。 */
export function createTranslationSettingsBindings({
  android,
  client,
  mobile,
  customTranslationsText,
  customTranslationsPlaceholder,
  customTranslationsNotice,
  customTranslationsSummary,
  customTranslationsSaveState,
  customTranslationsSaveError,
  onCustomTranslationsChange,
  onFlushCustomTranslations,
  ...options
}: TranslationSettingsBindingsOptions): TranslationSettingsBindings {
  return {
    candidate: createCandidateTranslationSettings({ android, ...options }),
    providers: android ? undefined : createTranslationProviderSettings({ client, ...options }),
    // 在确认 Android 宿主会读取释义覆盖文件之前不提供给它；它原来所在的翻译服务组在 Android 上被隐藏时，它也随之被隐藏了。
    customGlosses:
      !android && client.customTranslations
        ? {
            mobile,
            value: customTranslationsText,
            placeholder: customTranslationsPlaceholder,
            notice: customTranslationsNotice,
            summary: customTranslationsSummary,
            saveState: customTranslationsSaveState,
            saveError: customTranslationsSaveError,
            onChange: onCustomTranslationsChange,
            onFlush: onFlushCustomTranslations,
          }
        : undefined,
  };
}
