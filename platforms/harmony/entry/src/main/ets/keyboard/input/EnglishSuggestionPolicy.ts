import { TextPolicy } from "../TextPolicy";
/**
 * Text boundaries for direct English completion, ported from
 * platforms/android/java/app/msime/android/keyboard/EnglishSuggestionPolicy.java and
 * platforms/android/java/app/msime/android/candidate/EnglishSuggestionModel.java.
 *
 * Nothing here talks to the editor or the dictionary. The host reads the text before the cursor and
 * hands it over; what counts as the word being typed, whether it is worth asking about, and what
 * replacing it with a completion means are decided here, where they can be tested without a device.
 */
const MAX_ITEMS: number = 32;
const MAX_WORD_LENGTH: number = 128;
const MAX_RESPONSE_LENGTH: number = 262144;
const MIN_PREFIX_LENGTH: number = 2;

/** What the host must do to the editor to accept a completion. */
export interface EnglishReplacement {
  readonly deleteCount: number;
  readonly insert: string;
}

export interface EnglishCompletions {
  readonly prefix: string;
  readonly items: string[];
  /** 只有手机合并展示查询会带：为真时第二行是纠错，否则原文之后都是前缀补全。 */
  readonly corrected: boolean;
}

/** 编辑器回报选区或文字变化后，候选栏的英文建议该怎么办。 */
export enum EnglishRefresh {
  /** 当前词与已查询的相同，已有建议或进行中的查询仍然有效。 */
  KEEP = "keep",
  /** 光标前没有可查询的词，收起建议。 */
  CLEAR = "clear",
  /** 当前词变了，重新查询。 */
  QUERY = "query",
}

interface CompletionsReplyValue {
  prefix: string;
  items: string[];
  corrected?: boolean;
}

interface CompletionsReply {
  ok: boolean;
  value: CompletionsReplyValue;
  error: string;
}

export class EnglishSuggestionPolicy {
  static readonly LIMIT: number = 8;
  /** 手机候选栏合并展示的上限：原文加至多十一个前缀补全，与 Android、iOS 相同。 */
  static readonly DISPLAY_LIMIT: number = 12;
  /** Read one past the accepted word bound so a longer word cannot masquerade as its suffix. */
  static readonly CONTEXT_CHARACTERS: number = MAX_WORD_LENGTH + 1;

  /**
   * The word being typed, read backwards from the cursor.
   *
   * Full-width Latin normalises to ASCII: the packaged dictionary is ASCII, and a user in full-width
   * mode is still typing an English word. Anything else ends the word, which is what makes a space
   * or a comma a boundary without listing every character that could be one.
   */
  static currentWord(beforeCursor: string | null): string {
    if (beforeCursor === null || beforeCursor.length === 0) {
      return "";
    }
    const letters: string[] = [];
    const characters: string[] = TextPolicy.characters(beforeCursor);
    for (let index: number = characters.length - 1; index >= 0; index--) {
      const normalized: string = EnglishSuggestionPolicy.normalize(characters[index]);
      if (normalized.length === 0) {
        break;
      }
      letters.unshift(normalized);
      if (letters.length > MAX_WORD_LENGTH) {
        // Longer than any word the dictionary holds; the query would be refused anyway.
        return "";
      }
    }
    return letters.join("");
  }

  /** 旧补全至少两字母；手机纠错允许单字母和撇号，以便恢复 `I` 和缩写形式。 */
  static suggestible(prefix: string, correction: boolean = false): boolean {
    if (prefix.length < (correction ? 1 : MIN_PREFIX_LENGTH) || prefix.length > MAX_WORD_LENGTH) {
      return false;
    }
    for (const character of prefix) {
      if (!EnglishSuggestionPolicy.isAsciiLetter(character) && !(correction && character === "'")) {
        return false;
      }
    }
    return true;
  }

  /**
   * 重新核对光标前的当前词。编辑器在键盘每次上屏之后也会回报一次选区变化，所以当前词没变时必须保留已有建议，不能当作光标移开而清掉；光标移到别处时当前词随之改变，按新词重新查询或收起。
   */
  static refresh(prefix: string, requested: string, correction: boolean): EnglishRefresh {
    if (!EnglishSuggestionPolicy.suggestible(prefix, correction)) {
      return EnglishRefresh.CLEAR;
    }
    return prefix === requested ? EnglishRefresh.KEEP : EnglishRefresh.QUERY;
  }

  /**
   * What accepting a completion costs the editor, or null when it would change nothing.
   *
   * A word the user has already typed in full is not a completion, and deleting it to insert the
   * same characters would move the cursor for no reason.
   */
  static replacement(
    typed: string,
    candidate: string,
    startedCapitalized: boolean,
  ): EnglishReplacement | null {
    if (candidate.length === 0 || candidate.length > MAX_WORD_LENGTH) {
      return null;
    }
    const word: string = startedCapitalized
      ? candidate.charAt(0).toUpperCase() + candidate.substring(1)
      : candidate;
    return word === typed ? null : { deleteCount: typed.length, insert: word };
  }

  /** Whether the typed prefix asks for a capitalised completion. */
  static startedCapitalized(typed: string): boolean {
    return typed.length > 0 && typed.charAt(0) >= "A" && typed.charAt(0) <= "Z";
  }

  /**
   * Read a reply from the shared ABI, or null if it is not one.
   *
   * Bounded before it is believed. This is a dictionary lookup rather than a network response, but
   * the list goes straight onto the candidate strip, and a strip is a poor place to discover that
   * something upstream produced a megabyte of text.
   */
  static decode(response: string | null): EnglishCompletions | null {
    if (response === null || response.length === 0 || response.length > MAX_RESPONSE_LENGTH) {
      return null;
    }
    let reply: CompletionsReply;
    try {
      reply = JSON.parse(response) as CompletionsReply;
    } catch (error) {
      return null;
    }
    if (reply.ok !== true || reply.value === undefined || reply.value === null) {
      return null;
    }
    const prefix: string = reply.value.prefix;
    if (typeof prefix !== "string" || prefix.length === 0 || prefix.length > MAX_WORD_LENGTH) {
      return null;
    }
    const source: string[] = reply.value.items;
    if (!Array.isArray(source)) {
      return null;
    }
    const items: string[] = [];
    for (const item of source) {
      if (typeof item !== "string" || item.length === 0 || item.length > MAX_WORD_LENGTH) {
        return null;
      }
      items.push(item);
      if (items.length >= MAX_ITEMS) {
        break;
      }
    }
    const corrected: boolean = reply.value.corrected === true;
    if (corrected && items.length !== 2) {
      // 有纠错时引擎只给原文与纠错两行，形状不符的答案不可信。
      return null;
    }
    return { prefix: prefix, items: items, corrected: corrected };
  }

  private static normalize(character: string): string {
    if (character === "'") return character;
    if (EnglishSuggestionPolicy.isAsciiLetter(character)) {
      return character;
    }
    const code: number = character.charCodeAt(0);
    if ((code >= 0xff21 && code <= 0xff3a) || (code >= 0xff41 && code <= 0xff5a)) {
      return String.fromCharCode(code - 0xfee0);
    }
    return "";
  }

  private static isAsciiLetter(character: string): boolean {
    return (character >= "A" && character <= "Z") || (character >= "a" && character <= "z");
  }
}
