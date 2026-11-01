// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn(async (..._args: unknown[]) => undefined) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { NO_LOCAL_VOICE, speakWithLocalVoice, withReadAloud } from "../../src/core/read-aloud";
import type { VocabularyReviewClient } from "@msime/ui";

class Utterance {
  voice: unknown = null;
  lang = "";
  constructor(readonly text: string) {}
}

afterEach(() => {
  invoke.mockClear();
  vi.unstubAllGlobals();
});

function voice(name: string, lang: string, localService: boolean, isDefault = false) {
  return { name, lang, localService, default: isDefault, voiceURI: name } as SpeechSynthesisVoice;
}

function synthesizer(voices: SpeechSynthesisVoice[]) {
  return {
    getVoices: vi.fn(() => voices),
    cancel: vi.fn(),
    speak: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
}

test("a local English voice reads the word and network voices are never chosen", async () => {
  vi.stubGlobal("SpeechSynthesisUtterance", Utterance);
  const online = voice("Online (Natural)", "en-US", false, true);
  const local = voice("Samantha", "en-US", true);
  const synth = synthesizer([online, voice("Ting-Ting", "zh-CN", true, true), local]);
  await speakWithLocalVoice("interview", synth as unknown as SpeechSynthesis);
  expect(synth.cancel).toHaveBeenCalled();
  const utterance = synth.speak.mock.calls[0][0] as Utterance;
  expect(utterance.text).toBe("interview");
  expect(utterance.voice).toBe(local);
  expect(utterance.lang).toBe("en-US");
});

test("without an English voice the default local voice reads, and with no local voice nothing does", async () => {
  vi.stubGlobal("SpeechSynthesisUtterance", Utterance);
  const chinese = voice("Huihui", "zh-CN", true, true);
  const synth = synthesizer([voice("Online", "en-US", false), chinese]);
  await speakWithLocalVoice("budget", synth as unknown as SpeechSynthesis);
  expect((synth.speak.mock.calls[0][0] as Utterance).voice).toBe(chinese);

  const remoteOnly = synthesizer([voice("Online", "en-US", false)]);
  await expect(
    speakWithLocalVoice("budget", remoteOnly as unknown as SpeechSynthesis),
  ).rejects.toThrow(NO_LOCAL_VOICE);
  expect(remoteOnly.speak).not.toHaveBeenCalled();
  await expect(speakWithLocalVoice("budget", undefined)).rejects.toThrow(NO_LOCAL_VOICE);
});

test("an empty voice list waits for voiceschanged before choosing", async () => {
  vi.stubGlobal("SpeechSynthesisUtterance", Utterance);
  const local = voice("Daniel", "en-GB", true);
  let voices: SpeechSynthesisVoice[] = [];
  let changed: (() => void) | undefined;
  const synth = {
    ...synthesizer([]),
    getVoices: vi.fn(() => voices),
    addEventListener: vi.fn((_: string, listener: () => void) => (changed = listener)),
  };
  const spoken = speakWithLocalVoice("weekend", synth as unknown as SpeechSynthesis, 5000);
  await Promise.resolve();
  voices = [local];
  changed!();
  await spoken;
  expect((synth.speak.mock.calls[0][0] as Utterance).voice).toBe(local);
  expect(synth.removeEventListener).toHaveBeenCalled();
});

test("only a host that declares read_aloud gets speak, and Linux goes through spd-say", async () => {
  const base = { load: vi.fn() } as unknown as VocabularyReviewClient;
  expect(withReadAloud(base, null)).toBe(base);
  expect(withReadAloud(base, { platform: "macos", read_aloud: false })).toBe(base);
  expect(withReadAloud(base, { platform: "windows" })).toBe(base);
  const linux = withReadAloud(base, { platform: "linux", read_aloud: true });
  await linux.speak!("inspiration");
  expect(invoke).toHaveBeenCalledWith("read_aloud", { text: "inspiration" });
  expect(withReadAloud(base, { platform: "macos", read_aloud: true }).speak).toBeTypeOf("function");
});
