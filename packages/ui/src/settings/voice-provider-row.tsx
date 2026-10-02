import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import { VoiceProviderSelect } from "./voice-provider-select";
import type { VoiceProviderOption } from "../voice/voice-provider-options";

export interface VoiceProviderRowProps {
  title: ReactNode;
  description?: ReactNode;
  options: readonly VoiceProviderOption[];
  ariaLabel: string;
  value: string;
  disabled?: boolean;
  children?: ReactNode;
  onChange: (provider: string) => void;
}

/** A settings row for a provider selector shared by voice features. */
export function VoiceProviderRow({
  title,
  description,
  options,
  ariaLabel,
  value,
  disabled,
  children,
  onChange,
}: VoiceProviderRowProps) {
  return (
    <Row title={title} description={description}>
      <VoiceProviderSelect
        options={options}
        ariaLabel={ariaLabel}
        value={value}
        disabled={disabled}
        onChange={onChange}
      >
        {children}
      </VoiceProviderSelect>
    </Row>
  );
}
