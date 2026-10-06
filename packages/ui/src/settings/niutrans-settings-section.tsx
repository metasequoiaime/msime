import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { NiuTransSection } from "./niutrans-section";

export interface NiuTransSettingsSectionProps {
  grouped?: boolean;
  available: boolean;
  appId: string;
  apiKey: string;
  onAppIdChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  credentialTest?: ReactNode;
}

/** Shared NiuTrans settings binding; hosts choose whether the section owns a group wrapper. */
export function NiuTransSettingsSection({
  grouped = false,
  available,
  appId,
  apiKey,
  onAppIdChange,
  onApiKeyChange,
  credentialTest,
}: NiuTransSettingsSectionProps) {
  const content = (
    <NiuTransSection
      available={available}
      appId={appId}
      apiKey={apiKey}
      onAppIdChange={onAppIdChange}
      onApiKeyChange={onApiKeyChange}
    >
      {credentialTest}
    </NiuTransSection>
  );
  return grouped ? <GroupList title="小牛翻译">{content}</GroupList> : content;
}
