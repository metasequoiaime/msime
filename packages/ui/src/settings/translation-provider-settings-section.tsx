import {
  CustomTranslationSettingsSection,
  type CustomTranslationSettingsSectionProps,
} from "./custom-translation-settings-section";
import {
  NiuTransSettingsSection,
  type NiuTransSettingsSectionProps,
} from "./niutrans-settings-section";
import {
  TencentTranslationSettingsSection,
  type TencentTranslationSettingsSectionProps,
} from "./tencent-translation-settings-section";
import type { TranslationProvider } from "./translation-service-selector-section";

export type TranslationNiuTransSettings = Omit<NiuTransSettingsSectionProps, "grouped">;
export type TranslationTencentSettings = Omit<TencentTranslationSettingsSectionProps, "grouped">;
export type TranslationCustomSettings = Omit<CustomTranslationSettingsSectionProps, "grouped">;

export interface TranslationProviderSettingsSectionProps {
  grouped?: boolean;
  /** The service chosen in 翻译服务; only its settings are shown, and none for 关闭 or the MSIME account. */
  provider: TranslationProvider;
  niutrans: TranslationNiuTransSettings;
  tencent: TranslationTencentSettings;
  custom: TranslationCustomSettings;
}

/** Shared binding for the settings of the chosen translation service: Tencent, NiuTrans, or a custom one. The 翻译服务 select is the only place a service is turned on, so the other services' settings stay out of the way. */
export function TranslationProviderSettingsSection({
  grouped = false,
  provider,
  niutrans,
  tencent,
  custom,
}: TranslationProviderSettingsSectionProps) {
  switch (provider) {
    case "tencent":
      return <TencentTranslationSettingsSection grouped={grouped} {...tencent} />;
    case "niutrans":
      return <NiuTransSettingsSection grouped={grouped} {...niutrans} />;
    case "custom":
      return <CustomTranslationSettingsSection grouped={grouped} {...custom} />;
    case "none":
    case "account":
      return null;
  }
}
