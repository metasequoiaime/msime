/**
 * Lifecycle and presentation rules for optional offline candidate glosses, ported from
 * platforms/android/java/app/msime/android/CandidateGlossPolicy.java.
 *
 * A gloss arrives asynchronously, so the token says which session, generation and epoch asked for it.
 * Anything stale is dropped rather than painted onto whatever is on screen now.
 */
import { utf8Length } from "../Utf8";

const MAX_ENTRY_BYTES: number = 4096;

export interface GlossToken {
  readonly session: number;
  readonly generation: number;
  readonly epoch: number;
}

export class CandidateGlossPolicy {
  static token(session: number, generation: number, epoch: number): GlossToken {
    if (session <= 0 || generation < 0 || epoch < 0) {
      throw new Error("Invalid candidate gloss token");
    }
    return { session: session, generation: generation, epoch: epoch };
  }

  static isCurrent(
    token: GlossToken,
    currentSession: number,
    currentGeneration: number,
    currentEpoch: number,
  ): boolean {
    return (
      token.session === currentSession &&
      token.generation === currentGeneration &&
      token.epoch === currentEpoch
    );
  }

  /** Engine annotations, for example Wubi codes, occupy the shared hint slot first. */
  /**
   * Whether the annotation slot is carrying a translation rather than an Engine annotation.
   *
   * The two share a slot here but not in the source, which styles the translation in its own right
   * — `.cand-translation { font-size: 0.78em; opacity: 0.62 }`, identical in all four skins — and
   * says nothing at all about an Engine annotation. Keeping them apart is what lets the one with a
   * stated appearance take it without the one without a stated appearance having one invented.
   */
  static annotationIsTranslation(
    engineAnnotation: string | null,
    translation: string | null,
    glossEnabled: boolean,
  ): boolean {
    if (engineAnnotation !== null && engineAnnotation.length > 0) {
      return false;
    }
    return (
      glossEnabled &&
      CandidateGlossPolicy.bounded(translation) &&
      translation !== null &&
      translation.length > 0
    );
  }

  static annotation(
    engineAnnotation: string | null,
    translation: string | null,
    glossEnabled: boolean,
  ): string {
    if (engineAnnotation !== null && engineAnnotation.length > 0) {
      return engineAnnotation;
    }
    return glossEnabled && CandidateGlossPolicy.bounded(translation) && translation !== null
      ? translation
      : "";
  }

  static accessibilitySuffix(
    engineAnnotation: string | null,
    translation: string | null,
    glossEnabled: boolean,
  ): string {
    if (engineAnnotation !== null && engineAnnotation.length > 0) {
      return "，提示：" + engineAnnotation;
    }
    const gloss: string = CandidateGlossPolicy.annotation(
      engineAnnotation,
      translation,
      glossEnabled,
    );
    return gloss.length === 0 ? "" : "，英文释义：" + gloss;
  }

  /**
   * The 훈음 of a Korean Hanja row, which the Engine sends as the row's annotation (나라 이름 한 for 韓), or "" for any other row.
   *
   * A Korean user picks a Hanja by its 훈음, so it is drawn on the gloss line under the Hanja whatever the translation switches say, and the main text stays the Hanja alone. It is a separate field rather than the shared annotation slot because that slot is what the gloss menu types into the document, and the 훈음 is never committed.
   */
  static hunEum(koreanHanja: boolean, engineAnnotation: string | null): string {
    return koreanHanja &&
      CandidateGlossPolicy.bounded(engineAnnotation) &&
      engineAnnotation !== null
      ? engineAnnotation
      : "";
  }

  /** The Engine annotation left for the shared slot: none for a Korean Hanja row, whose annotation is its 훈음 and has a line of its own, so a translation can take the slot. */
  static slotAnnotation(koreanHanja: boolean, engineAnnotation: string | null): string | null {
    return koreanHanja ? null : engineAnnotation;
  }

  /** What the gloss line under a candidate says: the 훈음 and then the translation, on one line separated by a middle dot, either alone when the other is missing. */
  static glossLine(hunEum: string, translation: string): string {
    if (hunEum.length === 0) {
      return translation;
    }
    return translation.length === 0 ? hunEum : hunEum + " · " + translation;
  }

  /** The screen reader's words for a 훈음, read ahead of any translation suffix. */
  static hunEumAccessibilitySuffix(hunEum: string): string {
    return hunEum.length === 0 ? "" : "，训音：" + hunEum;
  }

  private static bounded(value: string | null): boolean {
    return value !== null && value.length > 0 && utf8Length(value) <= MAX_ENTRY_BYTES;
  }
}

/** 交给 `msime_client_pronunciation_request` 的一段文字；共享表只给英文标音。 */
export interface PronunciationItem {
  text: string;
  language: string;
}

export interface PronunciationRequest {
  generation: number;
  items: PronunciationItem[];
}

interface PronunciationEntry {
  text?: unknown;
  language?: unknown;
  pronunciation?: unknown;
}

interface PronunciationReplyValue {
  generation?: unknown;
  pronunciations?: unknown;
}

interface PronunciationReply {
  ok?: unknown;
  value?: PronunciationReplyValue;
}

/**
 * 候选英文音标的取词与校验，规则与 Android 的 `CandidatePronunciationPolicy`、`CandidatePronunciationModel` 相同：英文候选读它本身，中文候选读英语释义的第一行，首个词组和去冠词交给共享层。
 *
 * 音标只取决于查的那段文字和随包的表，与候选代次无关，所以宿主按文字缓存回答，不必按代次丢弃。
 */
export class CandidatePronunciationPolicy {
  /** 一次查询和缓存的上限；缓存满了整份清空，下一次按键重新查当前这一页。 */
  static readonly MAX_ITEMS: number = 256;

  static visible(enabled: boolean, available: boolean): boolean {
    return enabled && available;
  }

  /** 交给共享表的文字，没有可读的英文时为 ""。`englishTarget` 是释义的目标语言是否为英语，只有那时释义行才是英文。 */
  static lookupText(candidate: string, translation: string | null, englishTarget: boolean): string {
    if (CandidatePronunciationPolicy.plainEnglish(candidate)) {
      return utf8Length(candidate) <= MAX_ENTRY_BYTES ? candidate : "";
    }
    if (!englishTarget || translation === null || translation.length === 0) {
      return "";
    }
    const first: string = translation.split(/\r\n|\r|\n/)[0].trim();
    return first.length > 0 &&
      utf8Length(first) <= MAX_ENTRY_BYTES &&
      !CandidatePronunciationPolicy.hasControl(first)
      ? first
      : "";
  }

  /** 去重后的有界请求文档；同一段文字只查一次。 */
  static request(texts: string[]): string {
    const unique: string[] = [];
    for (const text of texts) {
      if (
        text.length === 0 ||
        CandidatePronunciationPolicy.hasControl(text) ||
        unique.includes(text)
      )
        continue;
      unique.push(text);
      if (unique.length >= CandidatePronunciationPolicy.MAX_ITEMS) break;
    }
    const request: PronunciationRequest = {
      generation: 0,
      items: unique.map((text: string): PronunciationItem => ({ text: text, language: "en" })),
    };
    return JSON.stringify(request);
  }

  /** 文字到 `/…/` 音标；失败的回答返回 null，宿主不缓存。没有音标的文字不在回答里。 */
  static decode(response: string): Map<string, string> | null {
    let reply: PronunciationReply;
    try {
      reply = JSON.parse(response) as PronunciationReply;
    } catch (error) {
      return null;
    }
    if (reply === null || typeof reply !== "object" || reply.ok !== true) return null;
    const value: PronunciationReplyValue | undefined = reply.value;
    if (value === undefined || value === null || !Array.isArray(value.pronunciations)) return null;
    const entries: PronunciationEntry[] = value.pronunciations as PronunciationEntry[];
    const readings: Map<string, string> = new Map<string, string>();
    for (const entry of entries) {
      if (entry === null || typeof entry !== "object" || entry.language !== "en") continue;
      const text: unknown = entry.text;
      const reading: unknown = entry.pronunciation;
      if (
        typeof text !== "string" ||
        typeof reading !== "string" ||
        text.length === 0 ||
        utf8Length(text) > MAX_ENTRY_BYTES ||
        reading.length < 3 ||
        utf8Length(reading) > MAX_ENTRY_BYTES ||
        !reading.startsWith("/") ||
        !reading.endsWith("/")
      )
        continue;
      readings.set(text, reading);
    }
    return readings;
  }

  /** 读屏在释义之后念的音标。 */
  static accessibilitySuffix(reading: string): string {
    return reading.length === 0 ? "" : "，音标：" + reading;
  }

  /** 共享层的请求只要有一段带控制字符（Rust 的 `char::is_control`，即 Cc 类）就整批拒绝，这样的文字（比如在线或 AI 译文里的制表符）不查，免得一条释义让整页都没有音标。 */
  private static hasControl(text: string): boolean {
    return /[\u0000-\u001f\u007f-\u009f]/.test(text);
  }

  private static plainEnglish(text: string): boolean {
    let letter: boolean = false;
    for (let index: number = 0; index < text.length; index++) {
      const value: string = text.charAt(index);
      if ((value >= "A" && value <= "Z") || (value >= "a" && value <= "z")) letter = true;
      else if (value !== " " && value !== "-" && value !== "'") return false;
    }
    return letter;
  }
}
