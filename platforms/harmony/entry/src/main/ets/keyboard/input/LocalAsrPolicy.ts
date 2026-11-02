import { utf8Length } from "../Utf8";

import { TextPolicy } from "../TextPolicy";
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
/** Silero VAD 的判定阈值缺省值，与 `LocalAsr.cpp` 相同。 */
export const LOCAL_VAD_THRESHOLD: number = 0.5;
/** 判停灵敏度（`voice_input.stop_sensitivity`）的范围，与 client-core 的 `VOICE_STOP_SENSITIVITY_RANGE` 相同。 */
export const LOCAL_STOP_SENSITIVITY_MIN: number = 10;
export const LOCAL_STOP_SENSITIVITY_MAX: number = 90;
/** 段首往前补（`LocalAsrPolicy.vadSegmentStart`）的参数，与 `LocalAsr.cpp` 一致：10 ms 一帧，最多往前 3 秒，低能量间隙不超过 0.25 秒，找到的起音前再留 0.3 秒；帧能量高出噪声底 4 倍（12 dB）算作语音，噪声底不低于 -100 dBFS。 */
const ONSET_FRAME: number = LOCAL_ASR_SAMPLE_RATE / 100;
const ONSET_LOOKBACK: number = LOCAL_ASR_SAMPLE_RATE * 3;
const ONSET_GAP: number = LOCAL_ASR_SAMPLE_RATE / 4;
const ONSET_PRE_ROLL: number = (LOCAL_ASR_SAMPLE_RATE * 3) / 10;
const ONSET_RISE: number = 4;
const ONSET_FLOOR: number = 1e-5;
/** 送进 VAD 的录音最多比一段语音的上限多留这么久（样本数），够往前补 3 秒加上段尾那 0.5 秒静音。 */
export const LOCAL_VAD_HISTORY_SLACK: number = LOCAL_ASR_SAMPLE_RATE * 5;
const MAX_TRANSDUCER_HOTWORDS: number = 200;
const MAX_FUNASR_HOTWORDS: number = 30;
const MAX_MODEL_PATH: number = 4096;
/** The manifest is a small catalog entry, not model data. */
export const LOCAL_MODEL_MANIFEST_MAX_BYTES: number = 256 * 1024;
/** Only the first token column is needed to encode native hotwords. */
export const LOCAL_MODEL_TOKENS_MAX_BYTES: number = 8 * 1024 * 1024;
const MAX_HOTWORDS: number = 200;
const MAX_HOTWORD_TEXT_BYTES: number = 256;
const MAX_HOTWORD_PINYIN_BYTES: number = 1024;

export const LOCAL_MODEL_ONLINE_TRANSDUCER: string = "online_transducer";
export const LOCAL_MODEL_OFFLINE_TRANSDUCER: string = "offline_transducer";
export const LOCAL_MODEL_OFFLINE_SENSE_VOICE: string = "offline_sense_voice";
export const LOCAL_MODEL_OFFLINE_FUNASR_NANO: string = "offline_funasr_nano";

/** 流式模型结束时补的尾部静音（样本数），即 `LocalAsr.cpp` 的 1.2 秒：让最后一块音频越过 encoder 的前瞻再结束输入。补 0.6 秒时 X-ASR 480 ms 流式模型会丢掉句末一两个字，松手即结束的按住说话没有句末停顿，更依赖这一段。 */
export const LOCAL_ONLINE_TAIL_SAMPLES: number = (LOCAL_ASR_SAMPLE_RATE * 12) / 10;
/** 离线 transducer 的 beam 宽度与热词加分，与 `LocalAsr.cpp` 一致：整段解码时 encoder 只跑一遍，宽 beam 的代价只落在 decoder/joiner 上。 */
export const OFFLINE_TRANSDUCER_MAX_ACTIVE_PATHS: number = 16;
export const TRANSDUCER_HOTWORDS_SCORE: number = 2.0;
/** SentencePiece 的词首标记。X-ASR 训练时汉字之间都隔着空格，于是它的 token 表里每个汉字都是带这个前缀的独立的词（`▁写`），一个不带前缀的汉字都没有。 */
const WORD_START: string = "\u2581";
/** 离线 transducer 的热词文件名，放在宿主给的临时目录里，识别器加载完就删掉。 */
const HOTWORDS_FILE_NAME: string = "local-asr-hotwords.txt";

/** Everything the worker needs to build a recognizer, resolved to absolute paths. Empty strings are roles the kind does not use. */
export interface LocalModelPlan {
  kind: string;
  directory: string;
  /** `native` (the recognizer takes them), `pinyin` (post-correction through the host API) or empty. */
  hotwords: string;
  modelingUnit: string;
  /** 清单写明的 sherpa-onnx 模型类型；留空时 sherpa 会为读 encoder 元数据里的类型多建一次推理会话。 */
  modelType: string;
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
  /** 宿主的临时目录，离线 transducer 的热词文件写在这里；空串表示没有，此时不带热词加载。 */
  scratch: string;
  /** 本次会话的判停灵敏度，只有 `start` 用；0 表示沿用缺省的 VAD 阈值，见 `LocalAsrPolicy.vadThreshold`。 */
  stopSensitivity: number;
}

/** One user dictionary word returned by `msime_client_voice_hotwords`. */
export interface LocalHotword {
  text: string;
  pinyin: string;
}

/** 工作线程发给主线程的消息：`started`、`partial`、`speech`、`final` 或 `error`。模型要按拼音事后纠错时，`final` 带上 `correct`。 */
export interface LocalAsrReply {
  type: string;
  id: number;
  text: string;
  correct: boolean;
  /** `final` 带上清单的 kind，主线程据此决定要不要把汉字数字改写成数字。 */
  kind: string;
  /** `speech`：经过 VAD 的整段模型此刻是否听到有人在说话，只在变化时发送。 */
  speaking: boolean;
}

interface ManifestDocument {
  kind?: string;
  files?: Record<string, string>;
  hotwords?: string;
  modeling_unit?: string;
  model_type?: string;
}

/** 紧跟在中文后面的半角标点换成的全角标点。 */
const HALF_TO_FULL_WIDTH: Record<string, string> = { ".": "。", "?": "？", "!": "！", ",": "，" };

export class LocalAsrPolicy {
  /**
   * 判停灵敏度换成 Silero VAD 的判定阈值（#10310）：灵敏度越高，阈值越高，越不容易把声音当作还在说话，停顿计时也就越早结束录音。10–90 的整数按 ÷100 换算，默认 50 正是原来写死的 0.5；0（不按停顿结束的会话）、越界或非整数都用缺省阈值。iOS 的 `VoicePauseSettings.vadThreshold` 用同一个换算。
   */
  static vadThreshold(stopSensitivity: number): number {
    if (
      !Number.isInteger(stopSensitivity) ||
      stopSensitivity < LOCAL_STOP_SENSITIVITY_MIN ||
      stopSensitivity > LOCAL_STOP_SENSITIVITY_MAX
    ) {
      return LOCAL_VAD_THRESHOLD;
    }
    return stopSensitivity / 100;
  }

  /** 纠错响应必须携带字符串，否则保留识别器原文。 */
  static correctedText(value: unknown, fallback: string): string {
    if (value === null || typeof value !== "object" || Array.isArray(value)) return fallback;
    const text: Object | undefined = (value as Record<string, Object>).text;
    return typeof text === "string" && text.length > 0 ? (text as string) : fallback;
  }

  /** Accept a bounded native hotword list, or null when the response crosses its contract. */
  static hotwords(value: unknown): LocalHotword[] | null {
    if (!Array.isArray(value) || value.length > MAX_HOTWORDS) return null;
    const hotwords: LocalHotword[] = [];
    for (const item of value) {
      if (item === null || typeof item !== "object" || Array.isArray(item)) return null;
      const record: Record<string, Object> = item as Record<string, Object>;
      const text: Object | undefined = record.text;
      const pinyin: Object | undefined = record.pinyin;
      if (typeof text !== "string" || typeof pinyin !== "string") return null;
      const textValue: string = text as string;
      const pinyinValue: string = pinyin as string;
      if (
        textValue.trim().length === 0 ||
        LocalAsrPolicy.invalidHotwordText(textValue, MAX_HOTWORD_TEXT_BYTES) ||
        LocalAsrPolicy.invalidHotwordText(pinyinValue, MAX_HOTWORD_PINYIN_BYTES)
      )
        return null;
      hotwords.push({ text: textValue, pinyin: pinyinValue });
    }
    return hotwords;
  }

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
    if (directory.length === 0 || root.length === 0 || root === "/" || directory === root)
      return "";
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
      modelType: typeof document.model_type === "string" ? document.model_type : "",
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
    if (plan.kind === LOCAL_MODEL_OFFLINE_TRANSDUCER) {
      return [plan.encoder, plan.decoder, plan.joiner, plan.tokens, plan.vad];
    }
    if (plan.kind === LOCAL_MODEL_OFFLINE_SENSE_VOICE) {
      return [plan.model, plan.tokens, plan.vad];
    }
    if (plan.kind === LOCAL_MODEL_OFFLINE_FUNASR_NANO) {
      return [plan.encoderAdaptor, plan.llm, plan.embedding, plan.tokenizer, plan.vad];
    }
    return [];
  }

  /** transducer（流式或离线）是否自己吃热词：编码热词要有 BPE 词表和建模单位。 */
  static transducerNativeHotwords(plan: LocalModelPlan): boolean {
    return (
      (plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER ||
        plan.kind === LOCAL_MODEL_OFFLINE_TRANSDUCER) &&
      plan.hotwords === "native" &&
      plan.bpeVocab.length > 0 &&
      plan.modelingUnit.length > 0
    );
  }

  /** Whether the final text goes through the host API's pinyin correction, i.e. the model has no hotword support of its own. */
  static correctsWithPinyin(plan: LocalModelPlan): boolean {
    return plan.hotwords === "pinyin";
  }

  /**
   * 离线 transducer 热词文件的路径，临时目录不可用时为空串。
   *
   * HAR 的离线接口够不到 `LocalAsr.cpp` 用的逐段热词：原生层的 `createOfflineStream` 接受第二个参数（热词串），但 HAR 的 `OfflineRecognizer.createStream()` 只传识别器句柄，`Index.ets` 也不导出原生函数；`OfflineRecognizerConfig` 上的 `hotwordsFfile` 拼错了，原生层读的是 `hotwordsFile`。所以热词按识别器级的 `hotwordsFile` 交给 sherpa：写进这个文件，加载时由 sherpa 读进去，效果与每段传入同一份热词相同，代价是词库变化时重新加载一次模型，与流式模型在本宿主上的做法一样。
   */
  static hotwordsFile(scratch: string): string {
    const directory: string = LocalAsrPolicy.modelDirectory(scratch);
    return directory.length > 1 ? `${directory}/${HOTWORDS_FILE_NAME}` : "";
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
   * token 表里的汉字是否都写成独立的词（`LocalAsr.cpp` 的 `cjk_spelled_as_words`）：没有一个不带前缀的汉字 token（UTF-8 三字节、首字节 0xE4–0xE9，即 U+4000–U+9FFF），却有 `▁` 加一个三字节字符的 token。是的话热词要按 `bpe` 编码、字与字之间隔空格；按清单原来写的 `cjkchar+bpe` 编码时 sherpa-onnx 找不到不带前缀的汉字，每个中文热词都编码失败。旧安装的清单仍写着 `cjkchar+bpe`，所以以 token 表为准，不重装也能用上热词。
   */
  static cjkSpelledAsWords(tokens: Set<string>): boolean {
    let spelled: boolean = false;
    for (const token of tokens) {
      const lead: number = token.codePointAt(0) ?? 0;
      if (utf8Length(token) === 3 && lead >= 0x4000 && lead <= 0x9fff) return false;
      const rest: string = token.startsWith(WORD_START) ? token.slice(WORD_START.length) : "";
      const restLead: number = rest.codePointAt(0) ?? 0;
      if (!spelled && utf8Length(rest) === 3 && restLead >= 0x4000 && restLead <= 0x9fff)
        spelled = true;
    }
    return spelled;
  }

  /** 原生热词实际要用的建模单位：token 表里汉字是独立的词时为 `bpe`，否则照清单。 */
  static hotwordModelingUnit(plan: LocalModelPlan, spacedCjk: boolean): string {
    return spacedCjk ? "bpe" : plan.modelingUnit;
  }

  /**
   * The newline-separated hotword list a transducer is created with.
   *
   * 只要有一个热词编码失败，sherpa-onnx 就丢掉全部热词，所以含有模型 token 表里没有的字的词在这里单独去掉，不连累其他词。ASCII 交给能拼出任何词的 BPE 模型；`/` 和空白合成一个空格；其他 ASCII 标点让整个词不用。`spacedCjk`（见 `cjkSpelledAsWords`）时每个非 ASCII 字自成一个词、前后隔空格，要求 token 表里有 `▁` 加这个字，否则整个词不用。
   */
  static transducerHotwords(words: string[], tokens: Set<string>, spacedCjk: boolean): string {
    let joined: string = "";
    let kept: number = 0;
    for (const word of words) {
      if (kept === MAX_TRANSDUCER_HOTWORDS) break;
      let clean: string = "";
      let usable: boolean = true;
      let previousSpace: boolean = true;
      for (const character of TextPolicy.characters(word)) {
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
        } else if (spacedCjk) {
          if (!tokens.has(`${WORD_START}${character}`)) {
            usable = false;
            break;
          }
          if (!previousSpace) clean += " ";
          clean += `${character} `;
          previousSpace = true;
          continue;
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

  /**
   * 整段模型解码一段语音时实际的起点（会话里的采样序号），与 `LocalAsr.cpp` 的 `local_vad_segment_start` 一致。sherpa-onnx 的 Silero VAD 从概率越过阈值的那一窗往前只留约 26 ms，输入电平低时句首的字就落在段外；这里从 VAD 的段首往前找回起音。`audio` 是会话最近的录音，`audio[0]` 是第 `audioStart` 个采样，`earliest` 是上一段的段尾（第一段为 0）。
   */
  static vadSegmentStart(
    audio: Float32Array,
    audioStart: number,
    vadStart: number,
    earliest: number,
  ): number {
    const audioEnd: number = audioStart + audio.length;
    const lower: number = Math.max(earliest, audioStart, vadStart - ONSET_LOOKBACK);
    if (lower >= vadStart || vadStart > audioEnd) return vadStart;
    const frameRms = (at: number): number => {
      let energy: number = 0;
      for (let index: number = at - audioStart; index < at - audioStart + ONSET_FRAME; index++) {
        energy += audio[index] * audio[index];
      }
      return Math.sqrt(energy / ONSET_FRAME);
    };
    // 噪声底取手里全部录音的帧能量第 10 百分位：说话之间的停顿、段尾那 0.5 秒静音都在里面，比只看段首之前稳。
    const levels: number[] = [];
    for (let at: number = audioStart; at + ONSET_FRAME <= audioEnd; at += ONSET_FRAME)
      levels.push(frameRms(at));
    let noiseFloor: number = ONSET_FLOOR;
    if (levels.length > 0) {
      levels.sort((left: number, right: number): number => left - right);
      noiseFloor = Math.max(noiseFloor, levels[Math.floor(levels.length / 10)]);
    }
    const speech: number = noiseFloor * ONSET_RISE;
    let onset: number = vadStart;
    let gap: number = 0;
    for (let at: number = vadStart - ONSET_FRAME; at >= lower; at -= ONSET_FRAME) {
      if (frameRms(at) > speech) {
        onset = at;
        gap = 0;
      } else {
        gap += ONSET_FRAME;
        if (gap > ONSET_GAP) break;
      }
    }
    return Math.max(lower, onset - ONSET_PRE_ROLL);
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
   *
   * 另有两条与 `tidy_local_transcript` 相同的标点规则：X-ASR 离线模型在中文后面也打半角标点（「是星期三.」），紧跟在汉字（CJK 统一表意文字、扩展区与兼容区）之后的 `.?!,` 换成全角；英文句子之间不留空格（「monday.Today」），夹在小写字母和大写字母之间的 `.?!` 后面补一个空格。「U.S.」「e.g.」「3.5」都不受影响。两条都看原文里的前后字符，而不是已经改写过的输出。
   */
  static tidyTranscript(text: string): string {
    const characters: string[] = TextPolicy.characters(text);
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
      const fullWidth: string | undefined = HALF_TO_FULL_WIDTH[character];
      if (fullWidth !== undefined) {
        const previous: string = index > 0 ? characters[index - 1] : "";
        if (LocalAsrPolicy.isCjkIdeograph(previous)) {
          out += fullWidth;
          continue;
        }
        const next: string = index + 1 < characters.length ? characters[index + 1] : "";
        if (character !== "," && /^[a-z]$/.test(previous) && /^[A-Z]$/.test(next)) {
          out += `${character} `;
          continue;
        }
      }
      out += character;
    }
    return out.replace(/ +$/, "");
  }

  private static invalidHotwordText(value: string, maximumBytes: number): boolean {
    if (utf8Length(value) > maximumBytes) return true;
    return TextPolicy.characters(value).some((character: string): boolean => {
      const code: number = character.codePointAt(0) ?? 0;
      return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
    });
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

  /** 汉字（CJK 统一表意文字、扩展区与兼容区），与 `LocalAsr.cpp` 的 `is_cjk_ideograph` 相同。 */
  private static isCjkIdeograph(character: string): boolean {
    const code: number = character.codePointAt(0) ?? 0;
    return (
      (code >= 0x3400 && code <= 0x9fff) ||
      (code >= 0xf900 && code <= 0xfaff) ||
      (code >= 0x20000 && code <= 0x3ffff)
    );
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
