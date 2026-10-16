export type AiAssistantPreferences = {
  enabled: boolean;
  provider: string;
  model: string;
  endpoint: string;
  candidate_limit: number;
  token?: string;
  tokens?: Record<string, string>;
  prompt_id?: string;
  prompt_custom_1: string;
  prompt_custom_2: string;
  prompt_custom_3: string;
};
