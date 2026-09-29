// @vitest-environment jsdom
import { expect, test } from "vitest";
import {
  ASR_PROVIDER_DEFAULTS,
  POLISH_PROVIDER_DEFAULTS,
  asrProviderUpdate,
  providerSettingValue,
  polishProviderUpdate,
} from "@msime/ui";

const doubaoEndpoint = ASR_PROVIDER_DEFAULTS.doubao.endpoint;

test("provider setting values prefer edits and fall back to known defaults", () => {
  expect(
    providerSettingValue(
      "  https://asr.internal.example/v1/audio/transcriptions  ",
      "openai",
      ASR_PROVIDER_DEFAULTS,
      "endpoint",
    ),
  ).toBe("https://asr.internal.example/v1/audio/transcriptions");
  expect(providerSettingValue("  ", "openai", ASR_PROVIDER_DEFAULTS, "model")).toBe("whisper-1");
  expect(providerSettingValue(undefined, "unknown", POLISH_PROVIDER_DEFAULTS, "model")).toBe("");
});

test("system recognition clears the active cloud credential and restores it on return", () => {
  const system = asrProviderUpdate("system", {
    asr_provider: "openai",
    asr_endpoint: ASR_PROVIDER_DEFAULTS.openai.endpoint,
    asr_model: "whisper-1",
    asr_token: "synthetic-openai",
    asr_tokens: { system: "unused-synthetic" },
  });
  expect(system.asr_endpoint).toBe("");
  expect(system.asr_model).toBe("");
  expect(system.asr_token).toBe("");
  expect(system.asr_tokens).toEqual({ openai: "synthetic-openai" });
  const restored = asrProviderUpdate("openai", system);
  expect(restored.asr_token).toBe("synthetic-openai");
  expect(restored.asr_endpoint).toBe(ASR_PROVIDER_DEFAULTS.openai.endpoint);
  expect(restored.asr_model).toBe("whisper-1");
  const custom = asrProviderUpdate("system", {
    asr_endpoint: "https://example.invalid/custom",
    asr_model: "synthetic-model",
  });
  expect(custom.asr_endpoint).toBeUndefined();
  expect(custom.asr_model).toBeUndefined();
});

test("picking a non-Doubao provider rewrites the shipped Doubao endpoint", () => {
  // The defect: asr_endpoint defaults to Doubao's websocket URL, and the
  // Windows host routes any websocket endpoint to DoubaoAsrClient. Leaving it
  // in place sent the user's OpenAI token to ByteDance.
  const update = asrProviderUpdate("openai", { asr_endpoint: doubaoEndpoint, asr_model: "" });
  expect(update.asr_provider).toBe("openai");
  expect(update.asr_endpoint).toBe(ASR_PROVIDER_DEFAULTS.openai.endpoint);
  expect(update.asr_endpoint?.startsWith("https://")).toBe(true);
  expect(update.asr_model).toBe("whisper-1");
});

test("every provider's default endpoint is rewritten on switch", () => {
  for (const provider of ["doubao", "openai", "siliconflow", "groq"]) {
    const update = asrProviderUpdate(provider, { asr_endpoint: doubaoEndpoint });
    expect(update.asr_endpoint).toBe(ASR_PROVIDER_DEFAULTS[provider].endpoint);
  }
  // Only Doubao keeps a websocket endpoint; the rest must be HTTPS, because
  // the native side reads the scheme to decide which client to build.
  for (const provider of ["openai", "siliconflow", "groq"]) {
    expect(ASR_PROVIDER_DEFAULTS[provider].endpoint.startsWith("wss://")).toBe(false);
  }
  expect(ASR_PROVIDER_DEFAULTS.doubao.endpoint.startsWith("wss://")).toBe(true);
});

test("a self-hosted endpoint the user typed is never overwritten", () => {
  const mine = "https://asr.internal.example/v1/audio/transcriptions";
  const update = asrProviderUpdate("groq", { asr_endpoint: mine, asr_model: "my-model" });
  expect(update.asr_endpoint).toBeUndefined();
  expect(update.asr_model).toBeUndefined();
  expect(update.asr_provider).toBe("groq");
});

test("an empty field is filled rather than left blank", () => {
  const update = asrProviderUpdate("siliconflow", { asr_endpoint: "", asr_model: "   " });
  expect(update.asr_endpoint).toBe(ASR_PROVIDER_DEFAULTS.siliconflow.endpoint);
  expect(update.asr_model).toBe(ASR_PROVIDER_DEFAULTS.siliconflow.model);
});

test("a superseded shipped default still counts as untouched", () => {
  // Someone who never edited the model but carries an older default should
  // still be moved forward rather than pinned to a stale value.
  const update = asrProviderUpdate("openai", { asr_model: "TeleAI/TeleSpeechASR" });
  expect(update.asr_model).toBe("whisper-1");
});

test("Doubao takes no model, so switching to it clears one", () => {
  const update = asrProviderUpdate("doubao", { asr_model: "whisper-1" });
  expect(update.asr_model).toBe("");
});

test("an unknown provider leaves endpoint and model alone", () => {
  const update = asrProviderUpdate("nonsense", {
    asr_endpoint: doubaoEndpoint,
    asr_model: "whisper-1",
  });
  // Nothing is known about it, so its endpoint and model are not invented.
  expect(update.asr_endpoint).toBeUndefined();
  expect(update.asr_model).toBeUndefined();
  expect(update.asr_provider).toBe("nonsense");
});

test("the polish provider follows the same rules", () => {
  const update = polishProviderUpdate("deepseek", {
    polish_endpoint: POLISH_PROVIDER_DEFAULTS.siliconflow.endpoint,
    polish_model: POLISH_PROVIDER_DEFAULTS.siliconflow.model,
  });
  expect(update.polish_endpoint).toBe(POLISH_PROVIDER_DEFAULTS.deepseek.endpoint);
  expect(update.polish_model).toBe(POLISH_PROVIDER_DEFAULTS.deepseek.model);

  const custom = polishProviderUpdate("openai", {
    polish_endpoint: "https://llm.internal.example/v1/chat",
  });
  expect(custom.polish_endpoint).toBeUndefined();
});

test("switching provider stashes the old key and restores the new one", () => {
  // A single flat token meant the previous provider's key stayed in the box and
  // was sent to the new endpoint until the user noticed.
  const first = asrProviderUpdate("openai", {
    asr_provider: "doubao",
    asr_token: "doubao-key",
    asr_tokens: {},
  });
  expect(first.asr_tokens).toEqual({ doubao: "doubao-key" });
  expect(first.asr_token).toBe("");

  // Coming back restores it rather than leaving the box empty.
  const back = asrProviderUpdate("doubao", {
    asr_provider: "openai",
    asr_token: "openai-key",
    asr_tokens: first.asr_tokens,
  });
  expect(back.asr_token).toBe("doubao-key");
  expect(back.asr_tokens).toEqual({ doubao: "doubao-key", openai: "openai-key" });
});

test("clearing the box forgets that provider's slot", () => {
  const update = asrProviderUpdate("groq", {
    asr_provider: "openai",
    asr_token: "  ".trim(),
    asr_tokens: { openai: "old", groq: "g" },
  });
  // An emptied box is a deliberate removal, not something to preserve.
  expect(update.asr_tokens.openai).toBeUndefined();
  expect(update.asr_token).toBe("g");
});

test("the polish provider keeps its own slots", () => {
  const update = polishProviderUpdate("deepseek", {
    polish_provider: "siliconflow",
    polish_token: "sf-key",
    polish_tokens: { deepseek: "ds-key" },
  });
  expect(update.polish_token).toBe("ds-key");
  expect(update.polish_tokens).toEqual({ siliconflow: "sf-key", deepseek: "ds-key" });
});

test("an unknown provider still swaps the token slot", () => {
  // Endpoint and model are left alone for an unknown id, but the credential
  // must not follow the user to it.
  const update = asrProviderUpdate("nonsense", {
    asr_provider: "openai",
    asr_token: "openai-key",
    asr_tokens: {},
  });
  expect(update.asr_token).toBe("");
  expect(update.asr_tokens).toEqual({ openai: "openai-key" });
});
