import type { VoiceInputPreferences } from "../index";
import { VoiceRecordingBehaviorSection } from "./voice-recording-behavior-section";

export interface VoiceRecordingBehaviorSettingsSectionProps {
  android: boolean;
  linux: boolean;
  voiceInput: VoiceInputPreferences;
  updateVoice: (patch: Partial<VoiceInputPreferences>) => void;
}

/** Binds recording behavior controls to the shared voice preferences. */
export function VoiceRecordingBehaviorSettingsSection({
  android,
  linux,
  voiceInput,
  updateVoice,
}: VoiceRecordingBehaviorSettingsSectionProps) {
  if (android) return null;

  return (
    <VoiceRecordingBehaviorSection
      linux={linux}
      soundEnabled={voiceInput.sound_enabled !== false}
      startSound={voiceInput.start_sound !== false}
      endSound={voiceInput.end_sound !== false}
      muteSystemAudio={voiceInput.mute_system_audio === true}
      onSoundEnabledChange={(sound_enabled) => updateVoice({ sound_enabled })}
      onStartSoundChange={(start_sound) => updateVoice({ start_sound })}
      onEndSoundChange={(end_sound) => updateVoice({ end_sound })}
      onMuteSystemAudioChange={(mute_system_audio) => updateVoice({ mute_system_audio })}
    />
  );
}
