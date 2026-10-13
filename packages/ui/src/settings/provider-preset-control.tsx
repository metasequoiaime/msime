import type { ReactNode } from "react";
import { ProviderPresetSection, type ProviderPreset } from "./provider-preset-section";

export interface ProviderPresetControlFactory {
  (label: string, preset: ProviderPreset | undefined, className?: string): ReactNode;
}

/** Binds the host's external-link command to the shared provider preset section. */
export function createProviderPresetControl(
  openExternalUrl?: (url: string) => void | Promise<void>,
): ProviderPresetControlFactory {
  return (label, preset, className = "section provider-preset-section") => (
    <ProviderPresetSection
      label={label}
      preset={preset}
      openExternalUrl={openExternalUrl}
      className={className}
    />
  );
}
