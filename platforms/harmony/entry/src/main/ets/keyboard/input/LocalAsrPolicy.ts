/**
 * The decisions behind on-device speech recognition, kept free of ArkUI, NAPI and the sherpa-onnx HAR so they run under node.
 *
 * The recognizer itself lives in `workers/LocalAsrWorker.ets` and the session in `HarmonyLocalAsrRecognizer.ets`; both follow the choices made here, which mirror `shared/voice/LocalAsr.cpp` so a model behaves the same on HarmonyOS as it does on the desktop hosts.
 */

export const LOCAL_ASR_SAMPLE_RATE: number = 16000;
/** The file the installer writes last into a model directory: the catalog entry, verbatim. */
export const LOCAL_MODEL_MANIFEST: string = "msime-model.json";
/** CoreSpeechKit's writeAudio accepts exactly 640 or 1280 bytes per call. */
export const CORE_SPEECH_CHUNK_BYTES: number = 1280;
/** One Silero VAD window at 16 kHz. */
export const LOCAL_VAD_WINDOW: number = 512;
const MAX_TRANSDUCER_HOTWORDS: number = 200;
const MAX_FUNASR_HOTWORDS: number = 30;
const MAX_MODEL_PATH: number = 4096;
/** The manifest is a small catalog entry, not model data. */
export const LOCAL_MODEL_MANIFEST_MAX_BYTES: number = 256 * 1024;
/** Only the first token column is needed to encode native hotwords. */
export const LOCAL_MODEL_TOKENS_MAX_BYTES: number = 8 * 1024 * 1024;

export const LOCAL_MODEL_ONLINE_TRANSDUCER: string = "online_transducer";
export const LOCAL_MODEL_OFFLINE_SENSE_VOICE: string = "offline_sense_voice";
export const LOCAL_MODEL_OFFLINE_FUNASR_NANO: string = "offline_funasr_nano";

/** Everything the worker needs to build a recognizer, resolved to absolute paths. Empty strings are roles the kind does not use. */
export interface LocalModelPlan {
  kind: string;
  directory: string;
  /** `native` (the recognizer takes them), `pinyin` (post-correction through the host API) or empty. */
  hotwords: string;
  modelingUnit: string;
  encoder: string;
  decoder: string;
  joiner: string;
  tokens: string;
  bpeVocab: string;
  model: string;
  vad: string;
  encoderAdaptor: string;
  llm: string;
  embedding: string;
  tokenizer: string;
}

/** Main thread to worker. `audio` carries 16 kHz mono PCM16 little-endian, transferred rather than copied. */
export interface LocalAsrRequest {
  type: string;
  id: number;
  directory: string;
  root: string;
  language: string;
  hotwords: string[];
  threads: number;
  pcm: ArrayBuffer | null;
}

/** Worker to main thread: `started`, `partial`, `final` or `error`. `correct` is set on `final` when the model wants pinyin post-correction. */
export interface LocalAsrReply {
  type: string;
  id: number;
  text: string;
  correct: boolean;
}

interface ManifestDocument {
  kind?: string;
  files?: Record<string, string>;
  hotwords?: string;
  modeling_unit?: string;
}

export class LocalAsrPolicy {
  /** Maximum bytes read from the text files the worker parses itself. */
  static textFileLimit(file: "manifest" | "tokens"): number {
    return file === "manifest" ? LOCAL_MODEL_MANIFEST_MAX_BYTES : LOCAL_MODEL_TOKENS_MAX_BYTES;
  }

  /** 判断配置是否指向宿主管理根目录下的 sherpa-onnx 模型目录。 */
  static usesLocalModel(
    provider: string,
    modelPath: string | undefined,
    trustedRoot: string | undefined,
  ): boolean {
    const directory: string = LocalAsrPolicy.modelUnderRoot(modelPath, trustedRoot);
    return provider === "local" && directory.length > 0;
  }

  /** The trimmed absolute model directory, or empty when the preference is missing, relative, too long or carries control characters. */
  static modelDirectory(modelPath: string | undefined): string {
    const path: string = (modelPath ?? "").trim();
    if (path.length === 0 || path.length > MAX_MODEL_PATH || !path.startsWith("/")) return "";
    if (/[\u0000-\u001f\u007f]/.test(path)) return "";
    if (path.split("/").some((part: string): boolean => part === "." || part === "..")) return "";
    return path.length > 1 && path.endsWith("/") ? path.slice(0, -1) : path;
  }

  /** 已安装模型目录必须位于宿主管理的模型根目录之下。 */
  static modelUnderRoot(modelPath: string | undefined, trustedRoot: string | undefined): string {
    const directory: string = LocalAsrPolicy.modelDirectory(modelPath);
    const root: string = LocalAsrPolicy.modelDirectory(trustedRoot);
    if (directory.length === 0 || root.length === 0 || root === "/" || directory === root) return "";
    return directory.startsWith(`${root}/`) ? directory : "";
  }

  /**
   * Resolve a manifest into the files a recognizer needs, or null when the manifest is malformed, names a kind this host does not run, or leaves out a required role.
   *
   * Paths inside the manifest must stay inside the model directory: an absolute path or a `..` segment is refused rather than resolved.
   */
  static plan(directory: string, manifestText: string): LocalModelPlan | null {
    let document: ManifestDocument;
    try {
      document = JSON.parse(manifestText) as ManifestDocument;
    } catch (error) {
      return null;
    }
    if (
      document === null ||
      typeof document !== "object" ||
      typeof document.kind !== "string" ||
      document.files === undefined ||
      document.files === null ||
      typeof document.files !== "object"
    ) {
      return null;
    }
    const files: Record<string, string> = document.files;
    const file = (role: string): string => LocalAsrPolicy.resolve(directory, files[role]);
    const plan: LocalModelPlan = {
      kind: document.kind,
      directory: directory,
      hotwords: typeof document.hotwords === "string" ? document.hotwords : "",
      modelingUnit: typeof document.modeling_unit === "string" ? document.modeling_unit : "",
      encoder: file("encoder"),
      decoder: file("decoder"),
      joiner: file("joiner"),
      tokens: file("tokens"),
      bpeVocab: file("bpe_vocab"),
      model: file("model"),
      vad: file("vad"),
      encoderAdaptor: file("encoder_adaptor"),
      llm: file("llm"),
      embedding: file("embedding"),
      tokenizer: file("tokenizer"),
    };
    const required: string[] = LocalAsrPolicy.requiredFiles(plan);
    if (required.length === 0 || required.some((path: string): boolean => path.length === 0))
      return null;
    return plan;
  }

  /** The resolved paths the kind cannot run without; empty for an unknown kind. */
  static requiredFiles(plan: LocalModelPlan): string[] {
    if (plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER) {
      return [plan.encoder, plan.decoder, plan.joiner, plan.tokens];
    }
    if (plan.kind === LOCAL_MODEL_OFFLINE_SENSE_VOICE) {
      return [plan.model, plan.tokens, plan.vad];
    }
    if (plan.kind === LOCAL_MODEL_OFFLINE_FUNASR_NANO) {
      return [plan.encoderAdaptor, plan.llm, plan.embedding, plan.tokenizer, plan.vad];
    }
    return [];
  }

  /** Whether the transducer takes hotwords itself: it needs the BPE vocabulary and a modeling unit to encode them. */
  static transducerNativeHotwords(plan: LocalModelPlan): boolean {
    return (
      plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER &&
      plan.hotwords === "native" &&
      plan.bpeVocab.length > 0 &&
      plan.modelingUnit.length > 0
    );
  }

  /** Whether the final text goes through the host API's pinyin correction, i.e. the model has no hotword support of its own. */
  static correctsWithPinyin(plan: LocalModelPlan): boolean {
    return plan.hotwords === "pinyin";
  }

  /** The first column of `tokens.txt`, which is what a hotword character has to be found in. */
  static tokenSet(tokensText: string): Set<string> {
    const tokens: Set<string> = new Set<string>();
    for (const line of tokensText.split("\n")) {
      const trimmed: string = line.replace(/\r$/, "");
      if (trimmed.length === 0) continue;
      const space: number = trimmed.indexOf(" ");
      tokens.add(space < 0 ? trimmed : trimmed.slice(0, space));
    }
    return tokens;
  }

  /**
   * The newline-separated hotword list a transducer is created with.
   *
   * sherpa-onnx drops every hotword when one fails to encode, so a word holding a character the model has no token for is left out rather than taking the others with it. ASCII goes through the BPE model, which can spell anything; `/` and whitespace become single spaces; other ASCII punctuation disqualifies the word.
   */
  static transducerHotwords(words: string[], tokens: Set<string>): string {
    let joined: string = "";
    let kept: number = 0;
    for (const word of words) {
      if (kept === MAX_TRANSDUCER_HOTWORDS) break;
      let clean: string = "";
      let usable: boolean = true;
      let previousSpace: boolean = true;
      for (const character of Array.from(word)) {
        if (character.length === 1 && character.charCodeAt(0) < 0x80) {
          if (/\s/.test(character) || character === "/") {
            if (!previousSpace) clean += " ";
            previousSpace = true;
            continue;
          }
          if (!/[A-Za-z0-9'-]/.test(character)) {
            usable = false;
            break;
          }
        } else if (!tokens.has(character)) {
          usable = false;
          break;
        }
        clean += character;
        previousSpace = false;
      }
      clean = clean.replace(/ +$/, "");
      if (!usable || clean.length === 0) continue;
      joined += `${clean}\n`;
      kept++;
    }
    return joined;
  }

  /** FunASR-nano's comma-separated prompt list: the prompt shares a 512-token context with the audio, so a long list crowds out the speech. */
  static funAsrHotwords(words: string[]): string {
    const kept: string[] = [];
    for (const word of words) {
      if (kept.length === MAX_FUNASR_HOTWORDS) break;
      if (word.length === 0 || word.includes(",")) continue;
      kept.push(word);
    }
    return kept.join(",");
  }

  /** SenseVoice guesses the language itself and handles Mandarin mixed with English best that way, so only the languages it would not reliably guess are pinned. */
  static senseVoiceLanguage(tag: string): string {
    const value: string = tag.trim().toLowerCase();
    if (value.startsWith("yue") || value === "zh-hk" || value === "zh-mo") return "yue";
    if (value.startsWith("ja")) return "ja";
    if (value.startsWith("ko")) return "ko";
    return "auto";
  }

  /** Recognizer threads. Two keeps a phone responsive while the keyboard is on screen; the desktop hosts use up to four. */
  static threads(requested: number): number {
    if (Number.isInteger(requested) && requested > 0) return Math.min(requested, 4);
    return 2;
  }

  /** Little-endian PCM16 to the [-1, 1) floats sherpa-onnx takes. A trailing odd byte is dropped. */
  static pcm16ToFloat(bytes: ArrayBuffer): Float32Array {
    const view: DataView = new DataView(bytes);
    const count: number = Math.floor(bytes.byteLength / 2);
    const samples: Float32Array = new Float32Array(count);
    for (let index: number = 0; index < count; index++) {
      samples[index] = view.getInt16(index * 2, true) / 32768;
    }
    return samples;
  }

  /** Segments joined the way `LocalAsr.cpp` joins them: a space only between two ASCII alphanumerics. */
  static joinSegments(segments: string[]): string {
    let joined: string = "";
    for (const segment of segments) {
      if (segment.length === 0) continue;
      if (joined.length > 0 && /[A-Za-z0-9]$/.test(joined) && /^[A-Za-z0-9]/.test(segment))
        joined += " ";
      joined += segment;
    }
    return joined;
  }

  /**
   * Remove the spaces the recognizers put where Chinese text has none: around CJK punctuation, between the letters of a spelled-out initialism, leading, trailing and repeated.
   */
  static tidyTranscript(text: string): string {
    const characters: string[] = Array.from(text);
    let out: string = "";
    for (let index: number = 0; index < characters.length; index++) {
      const character: string = characters[index];
      if (character === " ") {
        const afterMark: boolean =
          index > 0 && LocalAsrPolicy.isCjkPunctuation(characters[index - 1]);
        const beforeMark: boolean =
          index + 1 < characters.length && LocalAsrPolicy.isCjkPunctuation(characters[index + 1]);
        const insideInitialism: boolean =
          index > 0 &&
          LocalAsrPolicy.isSingleCapital(characters, index - 1) &&
          LocalAsrPolicy.isSingleCapital(characters, index + 1);
        if (afterMark || beforeMark || insideInitialism || out.length === 0 || out.endsWith(" "))
          continue;
      }
      out += character;
    }
    return out.replace(/ +$/, "");
  }

  private static resolve(directory: string, relative: string | undefined): string {
    if (typeof relative !== "string" || relative.length === 0 || relative.startsWith("/"))
      return "";
    if (relative.split("/").some((part: string): boolean => part === ".." || part === "."))
      return "";
    return `${directory}/${relative}`;
  }

  private static isCjkPunctuation(character: string): boolean {
    return "，。？！、：；“”‘’（）《》…".includes(character);
  }

  private static isSingleCapital(characters: string[], index: number): boolean {
    if (index >= characters.length) return false;
    const character: string = characters[index];
    if (!/^[A-Z]$/.test(character)) return false;
    const leftOk: boolean =
      index === 0 ||
      characters[index - 1] === " " ||
      !LocalAsrPolicy.isAscii(characters[index - 1]);
    const rightOk: boolean =
      index + 1 >= characters.length ||
      characters[index + 1] === " " ||
      !LocalAsrPolicy.isAscii(characters[index + 1]);
    return leftOk && rightOk;
  }

  private static isAscii(character: string): boolean {
    return character.length === 1 && character.charCodeAt(0) < 0x80;
  }
}

/**
 * Re-slices capture buffers into the fixed 1280-byte frames CoreSpeechKit's writeAudio requires. The capturer delivers whatever size the device chose; the recognizer refuses anything but 640 or 1280.
 */
export class PcmFrameSlicer {
  private pending: Uint8Array = new Uint8Array(CORE_SPEECH_CHUNK_BYTES);
  private filled: number = 0;

  push(data: ArrayBuffer): Uint8Array[] {
    const frames: Uint8Array[] = [];
    const input: Uint8Array = new Uint8Array(data);
    let offset: number = 0;
    while (offset < input.length) {
      const take: number = Math.min(CORE_SPEECH_CHUNK_BYTES - this.filled, input.length - offset);
      this.pending.set(input.subarray(offset, offset + take), this.filled);
      this.filled += take;
      offset += take;
      if (this.filled === CORE_SPEECH_CHUNK_BYTES) {
        frames.push(this.pending);
        this.pending = new Uint8Array(CORE_SPEECH_CHUNK_BYTES);
        this.filled = 0;
      }
    }
    return frames;
  }

  /** The partial tail padded with silence to a full frame, or null when nothing is pending. */
  flush(): Uint8Array | null {
    if (this.filled === 0) return null;
    const frame: Uint8Array = this.pending;
    frame.fill(0, this.filled);
    this.pending = new Uint8Array(CORE_SPEECH_CHUNK_BYTES);
    this.filled = 0;
    return frame;
  }

  reset(): void {
    this.pending = new Uint8Array(CORE_SPEECH_CHUNK_BYTES);
    this.filled = 0;
  }
}

/**
 * CoreSpeechKit reports one sentence at a time: `isFinal` closes a sentence and `isLast` closes the session. The panel shows every closed sentence followed by the one in progress.
 */
export class SpeechSentenceAccumulator {
  private closed: string = "";

  /** The whole transcript so far, after taking in one result. */
  accept(text: string, sentenceFinal: boolean): string {
    const whole: string = LocalAsrPolicy.joinSegments([this.closed, text]);
    if (sentenceFinal) this.closed = whole;
    return whole;
  }

  reset(): void {
    this.closed = "";
  }
}
