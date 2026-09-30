import { VoiceSettingsContent, type VoiceSettingsContentProps } from "./voice-settings-content";

export type VoiceSettingsPanelProps = Omit<
  VoiceSettingsContentProps,
  "grouped" | "showVoiceHotkeys" | "showCredentialTests"
>;

/** Embedded voice settings adapter; the shared content owns the controls and provider logic. */
export function VoiceSettingsPanel(props: VoiceSettingsPanelProps) {
  return (
    <VoiceSettingsContent
      {...props}
      grouped={false}
      showVoiceHotkeys={props.desktopPanels}
      showCredentialTests
    />
  );
}
