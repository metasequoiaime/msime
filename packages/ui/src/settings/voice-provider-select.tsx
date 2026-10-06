import type { ReactNode } from "react";
import { Select } from "../core/platform-controls";
import { VoiceProviderOptions, type VoiceProviderOption } from "../voice/voice-provider-options";

export interface VoiceProviderSelectProps {
  options: readonly VoiceProviderOption[];
  ariaLabel: string;
  value: string;
  disabled?: boolean;
  children?: ReactNode;
  onChange: (provider: string) => void;
}

/** Shared provider selector used by voice recognition and text-polish settings. */
export function VoiceProviderSelect({
  options,
  ariaLabel,
  value,
  disabled,
  children,
  onChange,
}: VoiceProviderSelectProps) {
  return (
    <Select
      aria-label={ariaLabel}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
    >
      <VoiceProviderOptions options={options} />
      {children}
    </Select>
  );
}
