import { VoiceProviderCatalog } from "./VoiceProviderCatalog";

/** The non-secret shape of the voice settings read from the Engine's prepared preferences. */
export interface VoicePolishTokens {
  siliconflow?: string;
  openai?: string;
  deepseek?: string;
  groq?: string;
}

export interface VoiceAsrTokens {
  doubao?: string;
  openai?: string;
  siliconflow?: string;
  groq?: string;
  everyapi?: string;
  mistral?: string;
  bailian?: string;
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
  /** 宿主注入的临时目录（扩展上下文的 `tempDir`）；离线 transducer 的热词文件在这里写下、加载完即删。 */
  local_model_scratch?: string;
  /** 点按开始的本地识别出字后停顿多久结束录音（毫秒），只留在本机；缺键为 1500，读法见 `VoiceRecognitionPolicy.pauseMillis`。 */
  pause_millis?: number;
  /** 判停灵敏度 10–90，越高越容易判定说完；缺键为 50，只留在本机。 */
  stop_sensitivity?: number;
  /** 键盘在开始聆听时注入：这次会话按停顿结束时为存下的判停灵敏度，否则为 0，本地识别据此选 VAD 阈值（`LocalAsrPolicy.vadThreshold`）。 */
  local_stop_sensitivity?: number;
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
  asr_provider: "local",
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
  polish_endpoint: VoiceProviderCatalog.polishEndpoint("siliconflow"),
  polish_model: VoiceProviderCatalog.polishModel("siliconflow"),
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
    if (config.asr_provider === "local") {
      // 出厂识别服务就是本地模型，新装还没有模型时录音会被 `HarmonyVoiceRecognizer.start` 以同一句话拒绝；这里先告诉用户去哪下载，不落到下面的「不可用，请检查设置」。
      return "使用本机模型识别，请先在设置中下载并选择本地语音模型；原始音频不会离开设备。";
    }
    if (DoubaoCredentialPolicy.attempted(config)) {
      return "使用豆包实时语音识别；原始音频会发送到豆包。";
    }
    if (config.asr_provider === "doubao") {
      return "豆包未填写密钥，暂用 HarmonyOS 系统语音识别；输入法不会保存原始音频。";
    }
    const cloudProviders: Record<string, string> = {
      openai: "OpenAI",
      siliconflow: "SiliconFlow",
      groq: "Groq",
      everyapi: "EveryAPI",
      mistral: "Mistral",
      bailian: "阿里云百炼",
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

/**
 * 豆包要不要用、缺什么。旧版出厂的 `asr_provider` 是 `doubao` 且不带任何凭据（现在出厂是 `local`，已保存的旧值不改），按旧默认装好、从没配置过的用户拿到的一直是系统识别，所以「一个凭据都没填」读作没有配置，照旧交给系统识别，不报错；只要填过其中一项（密钥或 App ID），就按豆包走，缺的那一项明确报出来，不再悄悄换成系统识别。
 */
export class DoubaoCredentialPolicy {
  /** 豆包密钥：当前框优先，空时取 `asr_tokens.doubao`，与共享解析 `resolve_voice_fields` 一致。 */
  static token(config: VoiceInputConfiguration): string {
    const flat: string = config.asr_token.trim();
    return flat.length > 0 ? flat : (config.asr_tokens?.doubao ?? "").trim();
  }

  /** 豆包接口地址：留空时用默认的双向流式地址。设置页把空地址显示成「双向流式」预设，Windows 的 `resolved_asr_endpoint` 和 HTTP 服务商的 `HttpAsrConfigurationPolicy` 也这样回退，这里不能把空地址当成没配置。 */
  static endpoint(config: VoiceInputConfiguration): string {
    const value: string = config.asr_endpoint.trim();
    return value.length > 0 ? value : DEFAULT_VOICE_INPUT_CONFIGURATION.asr_endpoint;
  }

  /** 豆包资源 ID：留空时用 `volc.seedasr.sauc.duration`，与 client-providers 的录音解析和 macOS、Windows 宿主一致；设置页「检查」按钮（共享的 `asrServiceCredentialTestConfig`）也按这个值回退，两边对空资源 ID 的结论相同。 */
  static resourceId(config: VoiceInputConfiguration): string {
    const value: string = config.asr_resource_id.trim();
    return value.length > 0 ? value : DEFAULT_VOICE_INPUT_CONFIGURATION.asr_resource_id;
  }

  /** 选了豆包，并且至少填过一项凭据。 */
  static attempted(config: VoiceInputConfiguration): boolean {
    return (
      config.asr_provider === "doubao" &&
      (DoubaoCredentialPolicy.token(config).length > 0 || config.asr_app_key.trim().length > 0)
    );
  }

  /** 填了一部分时缺的那一项，给语音面板显示；没选豆包、没碰过或凭据齐全时为 null。格式问题（非法字符、接口地址）仍由握手前的校验报出。 */
  static missing(config: VoiceInputConfiguration): string | null {
    if (!DoubaoCredentialPolicy.attempted(config)) return null;
    const legacy: boolean = config.doubao_auth_mode === "legacy";
    if (DoubaoCredentialPolicy.token(config).length === 0) {
      return legacy ? "豆包缺少 Access Token，请在设置中填写" : "豆包缺少 API Key，请在设置中填写";
    }
    if (legacy && config.asr_app_key.trim().length === 0)
      return "豆包旧版鉴权缺少 App ID，请在设置中填写";
    return null;
  }
}
