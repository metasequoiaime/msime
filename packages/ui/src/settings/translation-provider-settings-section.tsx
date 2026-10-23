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
import {
  AiTranslationSettingsSection,
  type AiTranslationSettingsSectionProps,
} from "./ai-translation-settings-section";
import type { TranslationProvider } from "./translation-service-selector-section";

export type TranslationNiuTransSettings = Omit<NiuTransSettingsSectionProps, "grouped">;
export type TranslationTencentSettings = Omit<
  TencentTranslationSettingsSectionProps,
  "grouped" | "foldRegion"
>;
export type TranslationCustomSettings = Omit<CustomTranslationSettingsSectionProps, "grouped">;

export interface TranslationProviderSettingsSectionProps {
  grouped?: boolean;
  /** 腾讯云的「地域」是否自带「更多选项」，见 `TencentTranslationSectionProps.foldRegion`。 */
  foldRegion?: boolean;
  /** 在「翻译服务」中选择的服务；只显示它的设置，选「关闭」或 MSIME 账号时不显示任何设置。 */
  provider: TranslationProvider;
  niutrans: TranslationNiuTransSettings;
  tencent: TranslationTencentSettings;
  custom: TranslationCustomSettings;
  /** 选「AI 辅助」时显示的区块；宿主没有 AI 偏好投影时可缺省，此时只显示未配置提示。 */
  ai?: Omit<AiTranslationSettingsSectionProps, "grouped">;
}

/** 所选翻译服务设置的共享绑定：腾讯、NiuTrans 或自定义服务。「翻译服务」下拉框是开启服务的唯一位置，所以其他服务的设置不会挡道。 */
export function TranslationProviderSettingsSection({
  grouped = false,
  foldRegion = true,
  provider,
  niutrans,
  tencent,
  custom,
  ai,
}: TranslationProviderSettingsSectionProps) {
  switch (provider) {
    case "ai":
      return (
        <AiTranslationSettingsSection
          grouped={grouped}
          summary={ai?.summary ?? ""}
          ready={ai?.ready ?? false}
          credentialTest={ai?.credentialTest}
        />
      );
    case "tencent":
      return (
        <TencentTranslationSettingsSection grouped={grouped} foldRegion={foldRegion} {...tencent} />
      );
    case "niutrans":
      return <NiuTransSettingsSection grouped={grouped} {...niutrans} />;
    case "custom":
      return <CustomTranslationSettingsSection grouped={grouped} {...custom} />;
    case "none":
    case "account":
      return null;
  }
}
