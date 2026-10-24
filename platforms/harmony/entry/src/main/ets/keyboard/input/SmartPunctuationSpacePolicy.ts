import { TextPolicy } from "../TextPolicy";
/**
 * Space after a just-committed Chinese punctuation rewrites it as ASCII.
 *
 * One of the smart-punctuation family, and the one this host left unconsumed: the settings page has
 * always shown the switch, the shared document has always carried it, and turning it on did
 * nothing here. Windows implements it in the TSF composition; the behaviour below is read from
 * there rather than invented, including the mapping table and the cases that decline to convert.
 *
 * The space is swallowed when it converts. That is the whole gesture — the user is correcting the
 * mark they just typed, not typing a mark and then a space — and inserting one as well would leave
 * them deleting it every time.
 *
 * Nothing here is time-limited. The source guards on the focus session and the foreground window
 * instead: the arming survives for as long as the caret has not moved and the editor has not
 * changed, which is the same thing `editorGeneration` says on this host. A stale arming cannot
 * rewrite the wrong character because the conversion re-reads what is actually before the caret.
 */

export interface SmartPunctuationSpaceSnapshot {
  /** The Chinese mark that was committed, as a scalar. */
  readonly chinese: number;
  /** Its ASCII replacement, already resolved so the conversion cannot disagree with the arming. */
  readonly ascii: number;
  /** Which editor session armed it. A different one means the caret is somewhere else entirely. */
  readonly editorGeneration: number;
}

export enum SpaceConvertDecision {
  /** Not armed, or this is not the key that would convert: the space is the editor's. */
  NONE,
  /** Replace the preceding Chinese mark with its ASCII twin and swallow the space. */
  CONVERT,
}

export class SmartPunctuationSpacePolicy {
  /**
   * The ASCII twin of a Chinese punctuation mark, or 0 where there is none.
   *
   * Transcribed from `SmartPunctuationAsciiFor` in the source's Composition.cpp. Both quote
   * directions map to the same ASCII quote, as they do there: a straight quote has no handedness.
   */
  static asciiFor(
    chinese: number,
    key: number = 0,
    enumerationCommaBackslash: boolean = true,
    enumerationCommaSlash: boolean = false,
  ): number {
    switch (chinese) {
      case 0x3002:
        return 0x2e; // 。 .
      case 0xff0c:
        return 0x2c; // ， ,
      case 0xff01:
        return 0x21; // ！ !
      case 0xff1f:
        return 0x3f; // ？ ?
      case 0xff1b:
        return 0x3b; // ； ;
      case 0xff1a:
        return 0x3a; // ： :
      case 0x3001:
        // 顿号的 ASCII 孪生按产生它的键和 View 的顿号开关定（契约 enumerationComma
        // 段）：按下的键开着时转回它，否则 `\` 优先、`/` 其次，两个都关按归一的 `\`。
        if (key === 0x5c && enumerationCommaBackslash) return 0x5c;
        if (key === 0x2f && enumerationCommaSlash) return 0x2f;
        if (enumerationCommaBackslash) return 0x5c;
        if (enumerationCommaSlash) return 0x2f;
        return 0x5c;
      case 0x201c:
        return 0x22; // “ "
      case 0x201d:
        return 0x22; // ” "
      case 0x2018:
        return 0x27; // ‘ '
      case 0x2019:
        return 0x27; // ’ '
      case 0x300e:
      case 0x300f:
        return 0x22; // 『 』 "，直角引号样式
      case 0x300c:
      case 0x300d:
        return 0x27; // 「 」 '，直角引号样式
      case 0x3010:
      case 0x3016:
        return 0x5b; // 【 〖 [
      case 0x3011:
      case 0x3017:
        return 0x5d; // 】 〗 ]
      case 0x300a:
        return 0x3c; // 《 <
      case 0x300b:
        return 0x3e; // 》 >
      case 0xff08:
        return 0x28; // （ (
      case 0xff09:
        return 0x29; // ） )
      default:
        return 0;
    }
  }

  /**
   * Arms the conversion after a commit, or refuses to.
   *
   * An auto-closed pair is refused, as in the source: the caret sits between the two marks there,
   * so the character before it is the opening one and rewriting it would break the pair. A commit
   * of more than one scalar is refused too — the mark has to be the last thing typed for the space
   * to be about it.
   */
  static arm(
    committed: string | null | undefined,
    autoClosedPair: boolean,
    smartPunctuation: boolean,
    spaceConvert: boolean,
    editorGeneration: number,
    key: number = 0,
    enumerationCommaBackslash: boolean = true,
    enumerationCommaSlash: boolean = false,
  ): SmartPunctuationSpaceSnapshot | null {
    if (!smartPunctuation || !spaceConvert || autoClosedPair) {
      return null;
    }
    if (committed === null || committed === undefined) {
      return null;
    }
    const points: number[] = TextPolicy.characters(committed).map(
      (value: string): number => value.codePointAt(0) ?? 0,
    );
    if (points.length !== 1) {
      return null;
    }
    const chinese: number = points[0];
    const ascii: number = SmartPunctuationSpacePolicy.asciiFor(
      chinese,
      key,
      enumerationCommaBackslash,
      enumerationCommaSlash,
    );
    if (ascii === 0) {
      return null;
    }
    return { chinese: chinese, ascii: ascii, editorGeneration: editorGeneration };
  }

  /**
   * What a key press should do, given what is armed.
   *
   * `preceding` is what the editor actually has before the caret, read at the moment of the press.
   * The source checks the same thing and declines when it disagrees: the arming says what was
   * committed, not what is still there, and anything may have happened in between.
   */
  static decide(
    snapshot: SmartPunctuationSpaceSnapshot | null,
    character: number,
    preceding: number,
    composing: boolean,
    editorGeneration: number,
  ): SpaceConvertDecision {
    if (snapshot === null || character !== 0x20 || composing) {
      return SpaceConvertDecision.NONE;
    }
    if (snapshot.editorGeneration !== editorGeneration) {
      return SpaceConvertDecision.NONE;
    }
    if (preceding !== snapshot.chinese) {
      return SpaceConvertDecision.NONE;
    }
    return SpaceConvertDecision.CONVERT;
  }
}
