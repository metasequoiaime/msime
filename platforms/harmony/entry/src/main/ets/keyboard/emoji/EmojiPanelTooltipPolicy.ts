import { TextPolicy } from "../TextPolicy";
/**
 * What the 2in1 emoji panel shows when the pointer rests on an item, ported from `ClipboardTooltipText`, `DisplayNameForItem` and `ArmTooltip` in MSIME-Windows/server/src/emoji-panel/EmojiPanel.cpp.
 *
 * A clipboard row shows two lines, so long entries that start the same way cannot be told apart without inserting them; Windows answers that with a preview of the whole entry after the pointer rests for a moment. Emoji, kaomoji and symbol cells show a name the same way. A phone has no pointer to rest, so this is only used on a 2in1.
 */

/** How long the pointer rests on an item before its tooltip appears (`kTooltipDelayMs`). */
export const EMOJI_TOOLTIP_DELAY_MS: number = 600;

/** The longest clipboard preview, in characters, before it is cut (`kClipboardTooltipMaxChars`). */
export const CLIPBOARD_TOOLTIP_MAX_CHARS: number = 200;

/** How many lines a clipboard preview may take (`kClipboardTooltipMaxLines`). */
export const CLIPBOARD_TOOLTIP_MAX_LINES: number = 12;

function isCjk(code: number): boolean {
  return (
    (code >= 0x4e00 && code <= 0x9fff) ||
    (code >= 0x3400 && code <= 0x4dbf) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0x3000 && code <= 0x303f)
  );
}

function hasCjk(token: string): boolean {
  for (let index: number = 0; index < token.length; index++) {
    if (isCjk(token.charCodeAt(index))) {
      return true;
    }
  }
  return false;
}

export class EmojiPanelTooltipPolicy {
  /**
   * The clipboard preview: carriage returns and tabs become spaces, trailing newlines go, and anything past 200 characters is cut with `...`. Characters are counted whole, so an emoji is never split in half the way a UTF-16 cut could.
   */
  static clipboardText(text: string): string {
    let tip: string = text.replace(/[\r\t]/g, " ");
    while (tip.endsWith("\n")) {
      tip = tip.substring(0, tip.length - 1);
    }
    const characters: string[] = TextPolicy.characters(tip);
    if (characters.length > CLIPBOARD_TOOLTIP_MAX_CHARS) {
      return characters.slice(0, CLIPBOARD_TOOLTIP_MAX_CHARS).join("") + "...";
    }
    return tip;
  }

  /**
   * An item's name from its space-separated keywords: the first word with a Chinese character in it, else the first word, else the item itself. The keyword list as a whole is too noisy to read as a name.
   */
  static displayName(keywords: string, fallback: string): string {
    const tokens: string[] = keywords
      .split(" ")
      .filter((token: string): boolean => token.length > 0);
    for (const token of tokens) {
      if (hasCjk(token)) {
        return token;
      }
    }
    return tokens.length > 0 ? tokens[0] : fallback;
  }
}
