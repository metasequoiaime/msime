import { expect, test } from "vitest";
import { aiCredentialSaveConfig } from "../../../../packages/ui/src/settings/ai-credential-save-config";

const ai = {
  provider: "openai",
  endpoint: "https://ai.example.test/v1/chat/completions",
  model: "test-model",
  enabled: true,
  candidate_limit: 3,
};

test("builds an AI credential save payload with a non-empty token", () => {
  expect(aiCredentialSaveConfig(ai, "synthetic-token")).toEqual({
    provider: ai.provider,
    endpoint: ai.endpoint,
    model: ai.model,
    token: "synthetic-token",
  });
});

test("omits an empty token so an existing credential is preserved", () => {
  expect(aiCredentialSaveConfig(ai, "  ")).toEqual({
    provider: ai.provider,
    endpoint: ai.endpoint,
    model: ai.model,
  });
});
