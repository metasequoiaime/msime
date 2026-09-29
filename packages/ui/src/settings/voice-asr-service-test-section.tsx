import type { ReactNode } from "react";
import type { VoiceInputPreferences } from "../index";
import { isAsrServiceProvider } from "../voice/voice-providers";
import {
  asrServiceCredentialTestConfig,
  asrServiceCredentialTestDisabled,
} from "./voice-credential-test-config";
import type { DoubaoAuthMode } from "./doubao-auth-mode-section";
import { VoiceSyntheticSilenceNotice } from "./voice-synthetic-silence-notice";

export interface VoiceAsrServiceTestSectionProps {
  available: boolean;
  voiceInput: VoiceInputPreferences;
  doubaoAuthMode: DoubaoAuthMode;
  credentialTestControl: (
    service: "voice.asr",
    label: string,
    config: Record<string, unknown>,
    disabled?: boolean,
  ) => ReactNode;
}

/** Shared remote ASR test notice and credential test control. */
export function VoiceAsrServiceTestSection({
  available,
  voiceInput,
  doubaoAuthMode,
  credentialTestControl,
}: VoiceAsrServiceTestSectionProps) {
  const provider = voiceInput.asr_provider ?? "";
  if (!available || !isAsrServiceProvider(provider)) return null;

  return (
    <>
      <VoiceSyntheticSilenceNotice />
      {credentialTestControl(
        "voice.asr",
        provider === "doubao" ? "测试豆包识别配置" : "测试语音识别配置",
        asrServiceCredentialTestConfig(voiceInput, doubaoAuthMode),
        asrServiceCredentialTestDisabled(voiceInput, doubaoAuthMode),
      )}
    </>
  );
}
