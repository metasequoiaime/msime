import type { ReactNode } from "react";
import type { VoiceInputPreferences } from "../index";
import { isAsrServiceProvider } from "../voice/voice-providers";
import {
  asrServiceCredentialTestConfig,
  asrServiceCredentialTestDisabled,
} from "./voice-credential-test-config";
import type { DoubaoAuthMode } from "./doubao-auth-mode-section";
import { VoiceSyntheticSilenceNotice } from "./voice-synthetic-silence-notice";
import * as settings from "./settings-style";

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
  /** 放在语音页「识别服务配置」组末尾时为真，画成组内的一块。 */
  grouped?: boolean;
}

/** Shared remote ASR test notice and credential test control. */
export function VoiceAsrServiceTestSection({
  available,
  voiceInput,
  doubaoAuthMode,
  credentialTestControl,
  grouped = false,
}: VoiceAsrServiceTestSectionProps) {
  const provider = voiceInput.asr_provider ?? "";
  if (!available || !isAsrServiceProvider(provider)) return null;

  const content = (
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
  return grouped ? <div className={settings.groupBlock}>{content}</div> : content;
}
