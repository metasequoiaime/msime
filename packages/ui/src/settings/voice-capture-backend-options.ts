import type { VoiceCaptureBackendOption } from "./voice-capture-devices-section";

export interface VoiceCaptureBackendOptionsContext {
  linux: boolean;
  macos: boolean;
  windows: boolean;
  harmony: boolean;
}

/** Builds the host-specific recording backend choices used by voice settings. */
export function voiceCaptureBackendOptions({
  linux,
  macos,
  windows,
  harmony,
}: VoiceCaptureBackendOptionsContext): readonly VoiceCaptureBackendOption[] {
  return [
    ["auto", "自动选择"],
    ...(linux
      ? ([
          ["pulse", "PulseAudio"],
          ["pipewire", "PipeWire"],
          ["alsa", "ALSA"],
        ] as const)
      : []),
    ...(macos ? ([["macos", "CoreAudio"]] as const) : []),
    ...(windows ? ([["windows", "Windows Audio"]] as const) : []),
    ...(harmony ? ([["harmony", "HarmonyOS 音频"]] as const) : []),
  ];
}
