import { ActionButton } from "./action-button";

export interface ProviderPreset {
  models?: readonly string[];
  documentation?: string;
}

export interface ProviderPresetSectionProps {
  label: string;
  preset?: ProviderPreset;
  openExternalUrl?: (url: string) => void | Promise<void>;
  className?: string;
}

/** The provider's API key documentation link. Its known models are offered by the model picker of the same group (`ModelPickerRows`), not here. */
export function ProviderPresetSection({
  label,
  preset,
  openExternalUrl,
  className = "section provider-preset-section",
}: ProviderPresetSectionProps) {
  const documentation = preset?.documentation;
  if (!documentation || !openExternalUrl) return null;
  return (
    <div className={className}>
      <ActionButton
        action={() => openExternalUrl(documentation)}
        label={`${label}接入说明与 API Key`}
      />
    </div>
  );
}
