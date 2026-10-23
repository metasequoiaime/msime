export type AiAssistantPreferences = {
  enabled: boolean;
  provider: string;
  model: string;
  endpoint: string;
  candidate_limit: number;
  token?: string;
  tokens?: Record<string, string>;
  /** 每个提供方记住的接口地址与模型，键为提供方 id；只存自定义值，缺键或空值表示用目录默认值。 */
  endpoints?: Record<string, string>;
  models?: Record<string, string>;
  /** DeepSeek 思考强度：disabled | low | high | max。 */
  thinking?: string;
  prompt_id?: string;
  prompt_custom_1: string;
  prompt_custom_2: string;
  prompt_custom_3: string;
};
