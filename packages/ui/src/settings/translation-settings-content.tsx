import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { TranslationServiceSelectorSection } from "./translation-service-selector-section";
import {
  CandidateTranslationSettingsSection,
  type CandidateTranslationSettingsSectionProps,
} from "./candidate-translation-settings-section";
import {
  TranslationProviderSettingsSection,
  type TranslationProviderSettingsSectionProps,
} from "./translation-provider-settings-section";

export interface TranslationSettingsContentProps {
  candidate: CandidateTranslationSettingsSectionProps;
  providers?: TranslationProviderSettingsSectionProps;
  /** 服务选择与凭据放入同一个桌面分区。 */
  serviceInOwnGroup?: boolean;
  /** 桌面页用顶部标签包装分区，手机保持原顺序。 */
  renderGroup?: (tab: "translation" | "providers", children: ReactNode) => ReactNode;
}

/** 候选翻译与可选服务配置的共用组合。 */
export function TranslationSettingsContent({
  candidate,
  providers,
  serviceInOwnGroup = false,
  renderGroup = (_tab, children) => children,
}: TranslationSettingsContentProps) {
  return (
    <>
      {renderGroup(
        "translation",
        <CandidateTranslationSettingsSection
          {...candidate}
          showTranslationService={serviceInOwnGroup ? false : candidate.showTranslationService}
        />,
      )}
      {providers &&
        renderGroup(
          "providers",
          <>
            {serviceInOwnGroup &&
              candidate.showTranslationService &&
              candidate.onTranslationProviderChange && (
                <GroupList title="翻译服务">
                  <TranslationServiceSelectorSection
                    grouped
                    available={candidate.enabled}
                    provider={candidate.translationProvider ?? "none"}
                    showAccountProvider={candidate.showAccountProvider ?? false}
                    onChange={candidate.onTranslationProviderChange}
                  />
                </GroupList>
              )}
            <TranslationProviderSettingsSection {...providers} />
          </>,
        )}
    </>
  );
}
