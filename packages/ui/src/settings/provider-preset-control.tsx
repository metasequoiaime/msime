import type { ReactNode } from "react";
import { ProviderPresetSection, type ProviderPreset } from "./provider-preset-section";

export interface ProviderPresetControlFactory {
  (
    label: string,
    preset: ProviderPreset | undefined,
    model: string,
    onSelectModel: (model: string) => void,
    className?: string,
  ): ReactNode;
}

/** Binds the host's external-link command to the shared provider preset section. */
export function createProviderPresetControl(
  openExternalUrl?: (url: string) => void | Promise<void>,
): ProviderPresetControlFactory {
  return (label, preset, model, onSelectModel, className = "section provider-preset-section") => (
    <ProviderPresetSection
      label={label}
      preset={preset}
      model={model}
      onSelectModel={onSelectModel}
      openExternalUrl={openExternalUrl}
      className={className}
    />
  );
}
