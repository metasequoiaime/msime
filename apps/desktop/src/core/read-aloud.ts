import { invoke } from "@tauri-apps/api/core";
import type { HostCapabilities, VocabularyReviewClient } from "@msime/ui";

/** 没有任何本机声音时给用户看的话。 */
export const NO_LOCAL_VOICE = "系统没有可用的朗读语音";

/**
 * 网页的 `speechSynthesis` 列出的本机声音。Chromium 内核（WebView2）和 WebKit 第一次调用 `getVoices()` 时可能还是空的，等一次 `voiceschanged`，最多等 `timeoutMs`。只留 `localService` 为真的：联网合成的声音会把词发出去，这里不用。
 */
async function localVoices(
  synth: SpeechSynthesis,
  timeoutMs: number,
): Promise<SpeechSynthesisVoice[]> {
  let voices = synth.getVoices();
  if (voices.length === 0)
    voices = await new Promise<SpeechSynthesisVoice[]>((resolve) => {
      const done = () => {
        synth.removeEventListener("voiceschanged", done);
        clearTimeout(timer);
        resolve(synth.getVoices());
      };
      const timer = setTimeout(done, timeoutMs);
      synth.addEventListener("voiceschanged", done);
    });
  return voices.filter((voice) => voice.localService);
}

/**
 * 用网页自带的语音合成读出 `text`：优先本机的英文声音，没有时用本机的默认声音（中文系统上常常只有中文声音，读英文单词能听懂），一个本机声音都没有时拒绝。总是显式指定声音，不让浏览器替我们挑一个可能联网的。新的一次打断还在读的上一次。macOS（WKWebView）和 Windows（WebView2）的设置应用用它，见 `.agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md`。
 */
export async function speakWithLocalVoice(
  text: string,
  synth: SpeechSynthesis | undefined = globalThis.speechSynthesis,
  timeoutMs = 1000,
): Promise<void> {
  if (!synth) throw new Error(NO_LOCAL_VOICE);
  const voices = await localVoices(synth, timeoutMs);
  const voice =
    voices.find((candidate) => candidate.lang.toLowerCase().startsWith("en")) ??
    voices.find((candidate) => candidate.default) ??
    voices[0];
  if (!voice) throw new Error(NO_LOCAL_VOICE);
  synth.cancel();
  const utterance = new SpeechSynthesisUtterance(text);
  utterance.voice = voice;
  utterance.lang = voice.lang;
  synth.speak(utterance);
}

/**
 * 给背单词页的客户端补上 `speak`：宿主声明 `read_aloud` 时才补。Linux 经 Tauri 命令起 `spd-say`（WebKitGTK 有没有 `speechSynthesis` 由发行版的编译选项决定），macOS 和 Windows 用网页自己的本机声音。
 */
export function withReadAloud(
  client: VocabularyReviewClient,
  host: Pick<HostCapabilities, "platform" | "read_aloud"> | null | undefined,
): VocabularyReviewClient {
  if (!host?.read_aloud) return client;
  const speak =
    host.platform === "linux"
      ? (word: string) => invoke<void>("read_aloud", { text: word })
      : (word: string) => speakWithLocalVoice(word);
  return { ...client, speak };
}
