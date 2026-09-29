import { AiSettingsPanel } from "./ai-settings-panel";
import { VoiceSettingsPanel } from "./voice-settings-panel";

export interface SettingsVoiceAiPagesProps {
  voice: Parameters<typeof VoiceSettingsPanel>[0];
  ai: Parameters<typeof AiSettingsPanel>[0];
}

/** Groups the voice and AI settings pages while keeping their independent panels. */
export function SettingsVoiceAiPages({ voice, ai }: SettingsVoiceAiPagesProps) {
  return (
    <>
      <VoiceSettingsPanel {...voice} />
      <AiSettingsPanel {...ai} />
    </>
  );
}
