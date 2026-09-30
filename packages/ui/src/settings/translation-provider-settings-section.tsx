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

export type TranslationNiuTransSettings = Omit<NiuTransSettingsSectionProps, "grouped">;
export type TranslationTencentSettings = Omit<TencentTranslationSettingsSectionProps, "grouped">;
export type TranslationCustomSettings = Omit<CustomTranslationSettingsSectionProps, "grouped">;

export interface TranslationProviderSettingsSectionProps {
  grouped?: boolean;
  niutrans: TranslationNiuTransSettings;
  tencent: TranslationTencentSettings;
  custom: TranslationCustomSettings;
}

/** Shared binding for the NiuTrans, Tencent, and custom translation provider settings. */
export function TranslationProviderSettingsSection({
  grouped = false,
  niutrans,
  tencent,
  custom,
}: TranslationProviderSettingsSectionProps) {
  return (
    <>
      <NiuTransSettingsSection grouped={grouped} {...niutrans} />
      <TencentTranslationSettingsSection grouped={grouped} {...tencent} />
      <CustomTranslationSettingsSection grouped={grouped} {...custom} />
    </>
  );
}
