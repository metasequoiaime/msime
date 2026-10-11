import { TextPolicy } from "../TextPolicy";
/**
 * What may be polished, and what the editor has to be told to replace it.
 *
 * The Apple keyboard polishes the user's *selection*: they highlight a sentence, tap 润色, and the
 * panel offers a rewrite of exactly that. HarmonyOS gives an input method no way to read a
 * selection — `InputClient` offers `getForwardSync` (before the caret) and `getBackwardSync`
 * (after it), and the selection is only ever reported to the keyboard as a pair of indices — so
 * copying the gesture would mean drawing a button that cannot know what it is acting on.
 *
 * The adaptation is to polish the text immediately before the caret: the message the user has just
 * finished typing. That is the same intent, expressed with what this platform does provide, and on
 * a phone it is also the more natural gesture — type, then tidy.
 *
 * Everything here is arithmetic and eligibility. The request itself is `HarmonyVoicePolisher`,
 * which is already this host's polisher and already allows the newlines a rewritten paragraph
 * contains; the panel is ArkUI. Neither can be tested off a device, and both are less likely to be
 * wrong than the question of how much text to take and how much to delete.
 */

/**
 * How much text before the caret is offered.
 *
 * Apple's ceiling is ten thousand characters, which is a sensible bound on something the user has
 * deliberately selected. Nothing is being selected here: this is read speculatively every time the
 * panel opens, so the number is a bound on what one editor read costs rather than on the user's
 * intent. A long message is a few hundred characters.
 */
export const MAX_POLISH_SOURCE_CHARACTERS = 1000;

/** The rewrite is bounded where the polisher bounds it, so a reply it would reject is refused here. */
export const MAX_POLISH_RESULT_CHARACTERS = 4000;

/** Below this there is nothing to tidy, and the request would cost more than the result is worth. */
const MIN_POLISH_SOURCE_CHARACTERS = 4;

export interface PolishReplacement {
  /** Characters to remove before the caret. Code points, not UTF-16 units — see below. */
  readonly deleteCount: number;
  readonly insert: string;
}

/**
 * Counted in code points rather than in `String.length`.
 *
 * 删除走 `deleteForwardSync(length)`（鸿蒙的 forward 是光标之前，见 `KeyboardSession.deleteEditorBackwardSync`），文档只写了 "length of text"，两种读法对 BMP 之外的字符不一样——句中一个 emoji 是一个码点、两个 UTF-16 单元。设备上实测（HarmonyOS 6.0.2 模拟器）`deleteForwardSync(1)` 把光标前的 😀 整个删掉，单位不是 UTF-16 单元，与这里按码点计算一致；由多个码点组成的字形（带肤色的 emoji 等）还没有在设备上试过。
 *
 * This is the single thing in this file that a device could still contradict, which is why the
 * caller re-reads the text before the caret and refuses to replace anything that no longer matches
 * what it captured. A wrong unit then costs a refusal rather than a corrupted message.
 */
export function codePointLength(value: string): number {
  return TextPolicy.characters(value).length;
}

export class AiPolishPolicy {
  /**
   * The text to offer, taken from what the editor reported before the caret.
   *
   * Trailing whitespace is dropped and leading whitespace is kept: the trailing newline a user has
   * just typed is not part of the sentence, while the indentation in front of it may be. Returns
   * an empty string when there is nothing worth sending, which is what the panel shows as its
   * "type something first" state rather than an error.
   */
  static source(before: string): string {
    if (before.length === 0) return "";
    const characters = TextPolicy.characters(before);
    const bounded =
      characters.length > MAX_POLISH_SOURCE_CHARACTERS
        ? characters.slice(characters.length - MAX_POLISH_SOURCE_CHARACTERS).join("")
        : before;
    const trimmed = bounded.replace(/\s+$/u, "");
    return codePointLength(trimmed) >= MIN_POLISH_SOURCE_CHARACTERS ? trimmed : "";
  }

  /** Whether a rewrite is worth offering: present, bounded, and not the text that was sent. */
  static usable(source: string, result: string): boolean {
    const trimmed = result.trim();
    if (trimmed.length === 0) return false;
    if (codePointLength(result) > MAX_POLISH_RESULT_CHARACTERS) return false;
    return trimmed !== source.trim();
  }

  /**
   * What to do to the editor, given the text still in front of the caret.
   *
   * `current` is re-read at the moment of replacing rather than trusted from when the panel opened.
   * A polish request takes seconds and the user can type, move the caret or switch field during it;
   * replacing then would delete whatever happens to be there now and put the rewrite of something
   * else in its place. Returning null is the refusal the panel reports.
   */
  static replacement(source: string, result: string, current: string): PolishReplacement | null {
    if (!AiPolishPolicy.usable(source, result)) return null;
    // `current` is whatever the editor reports before the caret now. The source has to still be
    // sitting at the end of it, which is true when nothing was typed and false as soon as anything
    // was — including a single space.
    if (source.length === 0 || !current.endsWith(source)) return null;
    return { deleteCount: codePointLength(source), insert: result };
  }
}
