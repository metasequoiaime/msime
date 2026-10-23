import type { AiAssistantPreferences } from "../index";
import { deepseekChatEndpoint } from "../core/provider-resources";

/** Instruction used by the AI polish test, separate from the candidate prompt. */
export const aiPolishTestPrompt = "请润色以下文字，保持原意，只返回修改后的文字。";

/** Defaults shared by the settings page and the AI test controls. */
export const defaultAiAssistant: AiAssistantPreferences = {
  enabled: false,
  provider: "deepseek",
  model: "deepseek-v4-flash",
  endpoint: deepseekChatEndpoint,
  candidate_limit: 3,
  token: "",
  tokens: {},
  endpoints: {},
  models: {},
  thinking: "disabled",
  prompt_id: "custom_1",
  prompt_custom_1: "",
  prompt_custom_2: "",
  prompt_custom_3: "",
};
