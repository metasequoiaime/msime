import { expect, test } from "vitest";
import { aiSettingsPreferences } from "@msime/ui";

test("uses the shared AI defaults when no draft exists", () => {
  const values = aiSettingsPreferences();

  expect(values.ai.provider).toBe("deepseek");
  expect(values.ai.model).toBe("deepseek-v4-flash");
  expect(values.storedAiCredential).toBeUndefined();
});

test("finds the stored credential for the active provider", () => {
  const values = aiSettingsPreferences(
    {
      enabled: true,
      provider: "openai",
      model: "gpt",
      endpoint: "https://api.example.test",
      candidate_limit: 3,
      prompt_custom_1: "",
      prompt_custom_2: "",
      prompt_custom_3: "",
    },
    {
      ai: [
        { provider: "deepseek", endpoint: "https://api.deepseek.com", model: "deepseek-v4-flash" },
        { provider: "openai", endpoint: "https://api.example.test", model: "gpt" },
      ],
      aiInvalid: false,
      tencent: null,
      tencentInvalid: false,
      voiceAsr: [],
      voicePolish: [],
      voiceInvalid: false,
    },
  );

  expect(values.storedAiCredential).toEqual({
    provider: "openai",
    endpoint: "https://api.example.test",
    model: "gpt",
  });
});
