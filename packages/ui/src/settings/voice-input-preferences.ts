/** 语音输入页和各宿主共同使用的语音偏好模型。 */
export type VoiceInputPreferences = {
  enabled: boolean;
  language: string;
  capture_backend?: "" | "auto" | "pulse" | "pipewire" | "alsa" | "windows" | "macos" | "harmony";
  capture_device?: string;
  asr_provider?: string;
  asr_endpoint?: string;
  asr_token?: string;
  asr_tokens?: Record<string, string>;
  asr_app_key?: string;
  doubao_auth_mode?: "api_key" | "legacy";
  hotkey_ralt?: boolean;
  hotkey_ctrl_f9?: boolean;
  hotkey_ctrl_win?: boolean;
  hotkey_rctrl_ralt?: boolean;
  hotkey_hold_space_lock?: boolean;
  sound_enabled?: boolean;
  start_sound?: boolean;
  end_sound?: boolean;
  mute_system_audio?: boolean;
  polish_enabled?: boolean;
  polish_text?: boolean;
  asr_model?: string;
  /** `local` 服务商加载的绝对路径，目录中应包含 `msime-model.json`。 */
  asr_model_path?: string;
  /** 加在每个模型下载地址前的可选 `https://` 镜像前缀；填了时先试它；之后不论填没填，都先走本项目镜像 `https://dl.msime.app/gh/`，最后才用目录中的原地址。 */
  asr_model_mirror?: string;
  asr_resource_id?: string;
  commit_mode?: "tsf" | "sendinput" | "ctrl_v";
  polish_provider?: string;
  polish_endpoint?: string;
  polish_token?: string;
  polish_tokens?: Record<string, string>;
  polish_model?: string;
  /** DeepSeek 润色的思考强度：disabled | low | high | max，仅 DeepSeek 生效。 */
  polish_thinking?: string;
  polish_prompt_id?: string;
  polish_prompt_custom_1?: string;
  polish_prompt_custom_2?: string;
  polish_prompt_custom_3?: string;
  stream_inline_preedit?: boolean;
  doubao_enable_itn?: boolean;
  doubao_enable_punc?: boolean;
  doubao_enable_ddc?: boolean;
  doubao_boosting_table_id?: string;
  /** 点按开始的本地识别出字后停顿多久结束录音（毫秒），500–5000、步进 100；缺键为 1500。只留在本机。 */
  pause_millis?: number;
  /** 判停灵敏度 10–90，越高越容易判定说完；缺键为 50。只留在本机。 */
  stop_sensitivity?: number;
  /** 语音标点：识别结果上屏前怎么处理标点，缺键为 `auto`（原样）；不认识的值宿主按 `auto` 处理。目前只有 Android 读它并在设置页给出（#10558），共享设置页和其他宿主见 #10595、#10596、#10597。 */
  punctuation_mode?: "auto" | "omit_trailing" | "omit_all" | "space";
  [key: string]: unknown;
};

/** `client-core` 的语音输入缺省偏好。 */
export const defaultVoiceInput: VoiceInputPreferences = {
  enabled: true,
  language: "zh-CN",
  asr_provider: "local",
  doubao_auth_mode: "api_key",
  asr_resource_id: "volc.seedasr.sauc.duration",
};
