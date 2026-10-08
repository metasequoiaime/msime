import { expect, test } from "vitest";
import {
  asrProviderCredentialTestDisabled,
  asrServiceCredentialTestDisabled,
  polishServiceCredentialTestDisabled,
} from "../../../../packages/ui/src/settings/voice-credential-test-config";

const baseVoice = {
  enabled: true,
  language: "zh-CN",
  asr_provider: "openai",
  asr_token: "synthetic-token",
};

test("requires an ASR token before testing a remote service", () => {
  expect(asrServiceCredentialTestDisabled({ ...baseVoice, asr_token: "" }, "api_key")).toBe(true);
  expect(asrServiceCredentialTestDisabled(baseVoice, "api_key")).toBe(false);
});

test("requires a legacy Doubao app key in addition to its token", () => {
  expect(
    asrServiceCredentialTestDisabled(
      { ...baseVoice, asr_provider: "doubao", asr_app_key: "" },
      "legacy",
    ),
  ).toBe(true);
  expect(
    asrServiceCredentialTestDisabled(
      { ...baseVoice, asr_provider: "doubao", asr_app_key: "synthetic-app" },
      "legacy",
    ),
  ).toBe(false);
  expect(
    asrServiceCredentialTestDisabled({ ...baseVoice, asr_provider: "doubao" }, "api_key"),
  ).toBe(false);
});

test("requires a polish token before testing the remote service", () => {
  expect(polishServiceCredentialTestDisabled({ ...baseVoice, polish_token: "" })).toBe(true);
  expect(
    polishServiceCredentialTestDisabled({ ...baseVoice, polish_token: "synthetic-token" }),
  ).toBe(false);
});

test("requires a chosen model before testing on-device recognition", () => {
  const local = { ...baseVoice, asr_provider: "local" };
  expect(asrProviderCredentialTestDisabled(local)).toBe(true);
  expect(asrProviderCredentialTestDisabled({ ...local, asr_model_path: "" })).toBe(true);
  expect(
    asrProviderCredentialTestDisabled({
      ...local,
      asr_model_path: "/synthetic/voice-models/synthetic-model",
    }),
  ).toBe(false);
  expect(asrProviderCredentialTestDisabled(baseVoice)).toBe(false);
});
