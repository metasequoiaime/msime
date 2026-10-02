// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ASR_PROVIDER_DEFAULTS, SettingsPage, asrProviderUpdate, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// EveryAPI and Mistral are the transcription services the Apple client offers and the shared
// client did not carry. Both speak the same OpenAI-compatible multipart API as the providers
// already here, so the risk is not the transport: it is a default that points somewhere else.
const added = ["everyapi", "mistral"] as const;

test("the new providers ship a batch HTTPS endpoint and a model", () => {
  for (const provider of added) {
    const defaults = ASR_PROVIDER_DEFAULTS[provider];
    expect(defaults.endpoint.startsWith("https://")).toBe(true);
    expect(defaults.model).not.toBe("");
    expect(defaults.models?.includes(defaults.model)).toBe(true);
    expect(defaults.documentation?.startsWith("https://")).toBe(true);
  }
});

// Leaving the Doubao websocket in asr_endpoint is what once routed an OpenAI token to
// ByteDance, so each newly reachable provider has to rewrite it in the same way.
test("switching to a new provider replaces the Doubao websocket endpoint", () => {
  for (const provider of added) {
    const update = asrProviderUpdate(provider, {
      asr_provider: "doubao",
      asr_endpoint: ASR_PROVIDER_DEFAULTS.doubao.endpoint,
      asr_model: "",
      asr_token: "synthetic-doubao",
    });
    expect(update.asr_provider).toBe(provider);
    expect(update.asr_endpoint).toBe(ASR_PROVIDER_DEFAULTS[provider].endpoint);
    expect(update.asr_model).toBe(ASR_PROVIDER_DEFAULTS[provider].model);
    // The key stays in the slot of the provider being left rather than following the user.
    expect(update.asr_token).toBe("");
    expect(update.asr_tokens).toEqual({ doubao: "synthetic-doubao" });
  }
});

test("a hand-written endpoint survives the switch", () => {
  const update = asrProviderUpdate("mistral", {
    asr_provider: "openai",
    asr_endpoint: "https://gateway.invalid/v1/audio/transcriptions",
    asr_model: "private-model",
  });
  expect(update.asr_endpoint).toBeUndefined();
  expect(update.asr_model).toBeUndefined();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: { enabled: true, language: "zh-CN", asr_provider: "mistral" },
  },
};

test("iOS voice settings list the new services and their preset models", async () => {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), host: testHost({ platform: "ios" }) }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));

  const select = screen.getByLabelText("识别服务") as HTMLSelectElement;
  const values = [...select.options].map((option) => option.value);
  expect(values).toContain("everyapi");
  expect(values).toContain("mistral");
  expect(select.value).toBe("mistral");
  expect(
    [...(screen.getByLabelText("识别服务预置模型") as HTMLSelectElement).options].map(
      (option) => option.value,
    ),
  ).toEqual(["", "voxtral-mini-latest"]);
});
