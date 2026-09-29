import { expect, test } from "vitest";
import {
  aiCredentialTestDisabled,
  aiProviderCredentialTestConfig,
  aiServiceCredentialTestConfig,
  aiServiceCredentialTestDisabled,
} from "../../../../packages/ui/src/settings/ai-credential-test-config";

const ai = {
  enabled: true,
  provider: "openai",
  endpoint: "https://ai.example.test/v1/chat/completions",
  model: "test-model",
};

test("builds the provider and remote AI credential test payloads", () => {
  expect(aiProviderCredentialTestConfig(ai)).toEqual({
    provider: ai.provider,
    endpoint: ai.endpoint,
    model: ai.model,
  });
  expect(aiServiceCredentialTestConfig(ai, "synthetic-token")).toEqual({
    provider: ai.provider,
    endpoint: ai.endpoint,
    model: ai.model,
    token: "synthetic-token",
  });
});

test("disables provider tests when AI is not ready", () => {
  expect(aiCredentialTestDisabled(ai, "openai.example.test")).toBe(false);
  expect(aiCredentialTestDisabled({ ...ai, enabled: false }, "openai.example.test")).toBe(true);
  expect(aiCredentialTestDisabled(ai, null)).toBe(true);
  expect(aiCredentialTestDisabled({ ...ai, model: " " }, "openai.example.test")).toBe(true);
});

test("requires a token for remote AI credential tests", () => {
  expect(aiServiceCredentialTestDisabled(ai, "openai.example.test", "synthetic-token")).toBe(false);
  expect(aiServiceCredentialTestDisabled(ai, "openai.example.test", " ")).toBe(true);
});
