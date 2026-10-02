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
  /** 在「翻译服务」中选择的服务；只显示它的设置，选「关闭」或 MSIME 账户时不显示任何设置。 */
  provider: TranslationProvider;
  niutrans: TranslationNiuTransSettings;
  tencent: TranslationTencentSettings;
  custom: TranslationCustomSettings;
}

/** 所选翻译服务设置的共享绑定：腾讯、NiuTrans 或自定义服务。「翻译服务」下拉框是开启服务的唯一位置，所以其他服务的设置不会挡道。 */
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
