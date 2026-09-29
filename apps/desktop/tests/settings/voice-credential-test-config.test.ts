import { expect, test } from "vitest";
import { asrServiceCredentialTestDisabled } from "../../../../packages/ui/src/settings/voice-credential-test-config";

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
