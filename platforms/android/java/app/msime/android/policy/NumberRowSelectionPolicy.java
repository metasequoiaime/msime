package app.msime.android;

import android.view.KeyEvent;

/**
 * Maps the hardware number row to the visible candidate slot.
 *
 * <p>The row means two different things depending on what is being spelled. While a pinyin or wubi
 * code is open, `1`..`9` pick off the strip, which is what the number row is for on every desktop
 * input method. In the Unicode local mode the same keys are the code point itself, so they have to
 * reach the Engine as input and the pick moves to `Shift+1`..`Shift+9` - which is the arrangement
 * the source uses and the one the settings page promises the user: 「空格上屏；Shift+数字选词」.
 *
 * <p>Without the mode this class could not tell the two apart, and it did not have it: every digit
 * typed after the first hex character of a code point selected a candidate instead, so U mode was
 * unusable from a hardware keyboard while the on-screen keyboard - which never comes through here -
 * worked. HarmonyOS decides it in the same place for the same reason.
 */
public final class NumberRowSelectionPolicy {
    /** The local mode name the shared view uses for hexadecimal code point entry. */
    public static final String UNICODE_MODE = "unicode";

    /** Not a candidate pick. */
    public static final int NONE = -1;

    private NumberRowSelectionPolicy() {}

    /**
     * 组字中或本地模式里，按键打出的字符被 Engine 列在 View.spelling_symbols 里时，它是 Engine 的输入，宿主要在选候选、翻页之前把它当字符送进去：网址模式的数字和网址符号、组字原文是网址触发词时的 `.` 和 `:`、V 模式的数字和运算符、U 模式的十六进制数字。没有组字时列出的 `/` 和 `@` 不在此列，它们照旧走标点路径。
     */
    public static boolean engineSpells(String localMode, String editingText, String spellingSymbols, int unicode) {
        if (unicode <= 0x20 || unicode >= 0x7f || spellingSymbols == null) return false;
        boolean composing = (localMode != null && !"none".equals(localMode))
            || (editingText != null && !editingText.isEmpty());
        return composing && spellingSymbols.indexOf((char) unicode) >= 0;
    }

    /**
     * The candidate slot this key picks, or {@link #NONE}. 与 Windows `EditPolicy.h` 的 `digit_selects_candidate` 同一套规则：U 模式按键位，Shift+数字选词、裸数字是十六进制；其他状态下 Engine 列为拼写的字符是输入；Engine 把这个键的数字列为拼写时（V、网址模式），打出别的字符的数字键（Shift+1 的 `!` 没被列出时）带不带 Shift 都选词；其余状态裸数字选词。
     *
     * @param localMode the active local input mode from the shared view, `none` when there is none
     * @param spellingSymbols View.spelling_symbols
     * @param unicode 这次按键打出的字符
     */
    public static int slotForKeyCode(int keyCode, boolean shift, boolean enabled, String localMode,
            String spellingSymbols, int unicode) {
        if (!enabled) return NONE;
        if (keyCode < KeyEvent.KEYCODE_1 || keyCode > KeyEvent.KEYCODE_9) return NONE;
        int slot = keyCode - KeyEvent.KEYCODE_1;
        // Outside U mode the shifted faces are the marks above the digits, and the user is entitled
        // to type them mid-composition; inside it they are the only way left to pick.
        if (UNICODE_MODE.equals(localMode)) return shift ? slot : NONE;
        String symbols = spellingSymbols == null ? "" : spellingSymbols;
        if (unicode > 0x20 && unicode < 0x7f && symbols.indexOf((char) unicode) >= 0) return NONE;
        // 只看这个键自己的数字是否被列为拼写，与 macOS 的 ShouldRouteSpellingShiftCandidateDigit 一致：注音选单打开时只列出 `0`，Shift+1..9 仍打出全角符号。
        if (symbols.indexOf((char) ('1' + slot)) >= 0) return slot;
        return shift ? NONE : slot;
    }
}
