/** The non-secret shape of the voice settings read from the Engine's prepared preferences. */
export interface VoicePolishTokens {
  siliconflow?: string;
  openai?: string;
  deepseek?: string;
  groq?: string;
}

export interface VoiceAsrTokens {
  openai?: string;
  siliconflow?: string;
  groq?: string;
  everyapi?: string;
  mistral?: string;
}

export interface VoiceInputConfiguration {
  /** Shared privacy/availability switch; omitted by older prepared preference documents. */
  enabled: boolean;
  asr_provider: string;
  language: string;
  /** Which host's enumeration the device id belongs to; empty or `auto` means the system default. */
  capture_backend: string;
  capture_device: string;
  /** The five Windows voice shortcuts; absent in older documents, where the defaults stand. */
  hotkey_ralt?: boolean;
  hotkey_ctrl_win?: boolean;
  hotkey_rctrl_ralt?: boolean;
  hotkey_hold_space_lock?: boolean;
  hotkey_ctrl_f9?: boolean;
  /** What happens around a recording rather than in it; absent means the shared defaults. */
  sound_enabled?: boolean;
  start_sound?: boolean;
  end_sound?: boolean;
  mute_system_audio?: boolean;
  /** Show partial recognizer output while recording; the shared default is off. */
  stream_inline_preedit?: boolean;
  asr_endpoint: string;
  asr_token: string;
  asr_tokens?: VoiceAsrTokens;
  asr_app_key: string;
  doubao_auth_mode: string;
  asr_resource_id: string;
  asr_model: string;
  /** Directory of the installed on-device model the `local` provider loads; absent or empty means none has been picked. */
  asr_model_path?: string;
  /** 宿主注入的根目录；本地识别只接受 `<filesDir>/voice-models` 下的模型。 */
  local_model_root?: string;
  /** Download mirror the settings page uses for model installs; the keyboard only carries it through. */
  asr_model_mirror?: string;
  polish_enabled: boolean;
  polish_text: boolean;
  polish_provider: string;
  polish_token: string;
  polish_tokens: VoicePolishTokens;
  polish_endpoint: string;
  polish_model: string;
  polish_prompt_id: string;
  polish_prompt_custom_1: string;
  polish_prompt_custom_2: string;
  polish_prompt_custom_3: string;
  doubao_enable_itn: boolean;
  doubao_enable_punc: boolean;
  doubao_enable_ddc: boolean;
  doubao_boosting_table_id: string;
}

export const DEFAULT_VOICE_INPUT_CONFIGURATION: VoiceInputConfiguration = {
  enabled: true,
  asr_provider: "doubao",
  language: "zh-cn",
  capture_backend: "",
  capture_device: "",
  hotkey_ralt: true,
  hotkey_ctrl_win: false,
  hotkey_rctrl_ralt: false,
  hotkey_hold_space_lock: true,
  hotkey_ctrl_f9: true,
  sound_enabled: true,
  start_sound: true,
  end_sound: true,
  mute_system_audio: false,
  stream_inline_preedit: false,
  asr_endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
  asr_token: "",
  asr_tokens: {},
  asr_app_key: "",
  doubao_auth_mode: "api_key",
  asr_resource_id: "volc.seedasr.sauc.duration",
  asr_model: "",
  asr_model_path: "",
  asr_model_mirror: "",
  polish_enabled: false,
  polish_text: false,
  polish_provider: "siliconflow",
  polish_token: "",
  polish_tokens: {},
  polish_endpoint: "https://api.siliconflow.cn/v1/chat/completions",
  polish_model: "Qwen/Qwen3-8B",
  polish_prompt_id: "cleanup",
  polish_prompt_custom_1: "",
  polish_prompt_custom_2: "",
  polish_prompt_custom_3: "",
  doubao_enable_itn: true,
  doubao_enable_punc: true,
  doubao_enable_ddc: false,
  doubao_boosting_table_id: "",
};

/** Voice input is on unless the prepared document explicitly turns it off. */
export class VoiceInputConfigurationPolicy {
  static enabled(value: boolean | undefined): boolean {
    return value !== false;
  }

  /** The short explanation shown in the keyboard's voice panel. Keep the privacy statement tied
   * to the selected recognizer: the local model never uploads audio, while every cloud provider
   * does. */
  static description(config: VoiceInputConfiguration): string {
    if (config.asr_provider === "local" && (config.asr_model_path ?? "").trim().length > 0) {
      return "使用本机模型识别；原始音频不会离开设备。";
    }
    if (config.asr_provider === "doubao" && config.asr_token.trim().length > 0) {
      return "使用豆包实时语音识别；原始音频会发送到豆包。";
    }
    const cloudProviders: Record<string, string> = {
      openai: "OpenAI",
      siliconflow: "SiliconFlow",
      groq: "Groq",
      everyapi: "EveryAPI",
      mistral: "Mistral",
    };
    const provider: string | undefined = cloudProviders[config.asr_provider];
    if (provider !== undefined) {
      return `使用${provider}云端语音识别；原始音频会发送到配置的服务。`;
    }
    if (config.asr_provider === "system" || config.asr_provider.length === 0) {
      return "使用 HarmonyOS 系统语音识别；输入法不会保存原始音频。";
    }
    return "当前语音服务不可用，请检查设置。";
  }
}
