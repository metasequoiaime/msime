package app.msime.android;

import android.view.KeyEvent;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The key ids the shared statistics store accepts, and how this host's keys map onto them.
 *
 * <p>The ids are W3C KeyboardEvent.code names plus the on-screen ids, copied verbatim from KEY_IDS in crates/client-core/src/typing_statistics.rs. The store rejects a whole batch that carries one id outside that list, so nothing here invents an id: a key that has no mapping returns {@code null} and is simply not counted.
 *
 * <p>Pure Java (the KeyEvent constants are compile-time ints) so the mapping can be checked on a host JVM.
 */
public final class KeyPressIds {
    /** Exactly the store's whitelist, in its order. */
    public static final List<String> KEY_IDS = List.of(
        "KeyA", "KeyB", "KeyC", "KeyD", "KeyE", "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK",
        "KeyL", "KeyM", "KeyN", "KeyO", "KeyP", "KeyQ", "KeyR", "KeyS", "KeyT", "KeyU", "KeyV",
        "KeyW", "KeyX", "KeyY", "KeyZ",
        "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
        "Digit9",
        "Backquote", "Minus", "Equal", "BracketLeft", "BracketRight", "Backslash", "Semicolon",
        "Quote", "Comma", "Period", "Slash",
        "IntlBackslash", "IntlRo", "IntlYen", "Lang1", "Lang2", "Convert", "NonConvert", "KanaMode",
        "Space", "Enter", "Backspace", "Tab", "Escape", "Delete", "Insert", "Home", "End", "PageUp",
        "PageDown", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
        "CapsLock", "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight",
        "MetaLeft", "MetaRight", "Fn", "ContextMenu",
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
        "Numpad0", "Numpad1", "Numpad2", "Numpad3", "Numpad4", "Numpad5", "Numpad6", "Numpad7",
        "Numpad8", "Numpad9", "NumpadDecimal", "NumpadEnter", "NumpadAdd", "NumpadSubtract",
        "NumpadMultiply", "NumpadDivide", "NumLock",
        "Nine0", "Nine1", "Nine2", "Nine3", "Nine4", "Nine5", "Nine6", "Nine7", "Nine8", "Nine9",
        "SoftPunctuation", "SoftSymbol", "SoftLayer", "SoftLanguage", "SoftGlobe", "SoftEmoji",
        "SoftVoice",
        "FourteenQW", "FourteenER", "FourteenTY", "FourteenUI", "FourteenOP", "FourteenAS",
        "FourteenDF", "FourteenGH", "FourteenJK", "FourteenL", "FourteenZX", "FourteenCV",
        "FourteenBN", "FourteenM");

    private static final Set<String> KNOWN = Set.copyOf(KEY_IDS);

    /** The ANSI key that types each printable ASCII punctuation mark, unshifted or shifted. */
    private static final Map<Character, String> PUNCTUATION = Map.ofEntries(
        Map.entry('`', "Backquote"), Map.entry('~', "Backquote"),
        Map.entry('!', "Digit1"), Map.entry('@', "Digit2"), Map.entry('#', "Digit3"),
        Map.entry('$', "Digit4"), Map.entry('%', "Digit5"), Map.entry('^', "Digit6"),
        Map.entry('&', "Digit7"), Map.entry('*', "Digit8"), Map.entry('(', "Digit9"),
        Map.entry(')', "Digit0"),
        Map.entry('-', "Minus"), Map.entry('_', "Minus"), Map.entry('=', "Equal"),
        Map.entry('+', "Equal"),
        Map.entry('[', "BracketLeft"), Map.entry('{', "BracketLeft"),
        Map.entry(']', "BracketRight"), Map.entry('}', "BracketRight"),
        Map.entry('\\', "Backslash"), Map.entry('|', "Backslash"),
        Map.entry(';', "Semicolon"), Map.entry(':', "Semicolon"),
        Map.entry('\'', "Quote"), Map.entry('"', "Quote"),
        Map.entry(',', "Comma"), Map.entry('<', "Comma"),
        Map.entry('.', "Period"), Map.entry('>', "Period"),
        Map.entry('/', "Slash"), Map.entry('?', "Slash"));

    private KeyPressIds() {}

    public static boolean isKnown(String id) { return id != null && KNOWN.contains(id); }

    /**
     * The key that types one ASCII character on an ANSI keyboard: a letter in either case, a digit, a punctuation mark by the key that carries it shifted or not, space or return.
     *
     * <p>This is how a soft 26-key letter or a symbol-layer key gets its id: "!" on the symbol layer is Digit1, the key a hardware keyboard types it with.
     */
    public static String forCharacter(char character) {
        if (TextPolicy.isAsciiLetter(character)) return "Key" + Character.toUpperCase(character);
        if (character >= '0' && character <= '9') return "Digit" + character;
        if (character == ' ') return "Space";
        if (character == '\n') return "Enter";
        return PUNCTUATION.get(character);
    }

    /** A nine-key grid cell by the digit printed on it; Nine1 is the separator and punctuation cell. */
    public static String forNineKeyDigit(int digit) {
        return digit >= 0 && digit <= 9 ? "Nine" + digit : null;
    }

    /**
     * The Japanese kana grid's cells as nine-key cells.
     *
     * <p>The kana list runs あ to ら over the three rows of three, then わ under them where a phone keypad has 0, then the 、。？！ key. That last one is the grid's punctuation key, so it counts with the nine-key punctuation keys.
     */
    public static String forJapaneseKeyIndex(int index) {
        if (index >= 0 && index <= 8) return "Nine" + (index + 1);
        if (index == 9) return "Nine0";
        if (index == 10) return "SoftPunctuation";
        return null;
    }

    /** A hardware key by its Android key code, or {@code null} for a key the store has no id for. */
    public static String forKeyCode(int keyCode) {
        if (keyCode >= KeyEvent.KEYCODE_A && keyCode <= KeyEvent.KEYCODE_Z)
            return "Key" + (char) ('A' + keyCode - KeyEvent.KEYCODE_A);
        if (keyCode >= KeyEvent.KEYCODE_0 && keyCode <= KeyEvent.KEYCODE_9)
            return "Digit" + (keyCode - KeyEvent.KEYCODE_0);
        if (keyCode >= KeyEvent.KEYCODE_F1 && keyCode <= KeyEvent.KEYCODE_F12)
            return "F" + (keyCode - KeyEvent.KEYCODE_F1 + 1);
        if (keyCode >= KeyEvent.KEYCODE_NUMPAD_0 && keyCode <= KeyEvent.KEYCODE_NUMPAD_9)
            return "Numpad" + (keyCode - KeyEvent.KEYCODE_NUMPAD_0);
        return switch (keyCode) {
            case KeyEvent.KEYCODE_GRAVE -> "Backquote";
            case KeyEvent.KEYCODE_MINUS -> "Minus";
            case KeyEvent.KEYCODE_EQUALS -> "Equal";
            case KeyEvent.KEYCODE_LEFT_BRACKET -> "BracketLeft";
            case KeyEvent.KEYCODE_RIGHT_BRACKET -> "BracketRight";
            case KeyEvent.KEYCODE_BACKSLASH -> "Backslash";
            case KeyEvent.KEYCODE_SEMICOLON -> "Semicolon";
            case KeyEvent.KEYCODE_APOSTROPHE -> "Quote";
            case KeyEvent.KEYCODE_COMMA -> "Comma";
            case KeyEvent.KEYCODE_PERIOD -> "Period";
            case KeyEvent.KEYCODE_SLASH -> "Slash";
            case KeyEvent.KEYCODE_RO -> "IntlRo";
            case KeyEvent.KEYCODE_YEN -> "IntlYen";
            case KeyEvent.KEYCODE_KANA -> "Lang1";
            case KeyEvent.KEYCODE_EISU -> "Lang2";
            case KeyEvent.KEYCODE_HENKAN -> "Convert";
            case KeyEvent.KEYCODE_MUHENKAN -> "NonConvert";
            case KeyEvent.KEYCODE_KATAKANA_HIRAGANA -> "KanaMode";
            case KeyEvent.KEYCODE_SPACE -> "Space";
            case KeyEvent.KEYCODE_ENTER -> "Enter";
            case KeyEvent.KEYCODE_DEL -> "Backspace";
            case KeyEvent.KEYCODE_TAB -> "Tab";
            case KeyEvent.KEYCODE_ESCAPE -> "Escape";
            case KeyEvent.KEYCODE_FORWARD_DEL -> "Delete";
            case KeyEvent.KEYCODE_INSERT -> "Insert";
            case KeyEvent.KEYCODE_MOVE_HOME -> "Home";
            case KeyEvent.KEYCODE_MOVE_END -> "End";
            case KeyEvent.KEYCODE_PAGE_UP -> "PageUp";
            case KeyEvent.KEYCODE_PAGE_DOWN -> "PageDown";
            case KeyEvent.KEYCODE_DPAD_UP -> "ArrowUp";
            case KeyEvent.KEYCODE_DPAD_DOWN -> "ArrowDown";
            case KeyEvent.KEYCODE_DPAD_LEFT -> "ArrowLeft";
            case KeyEvent.KEYCODE_DPAD_RIGHT -> "ArrowRight";
            case KeyEvent.KEYCODE_CAPS_LOCK -> "CapsLock";
            case KeyEvent.KEYCODE_SHIFT_LEFT -> "ShiftLeft";
            case KeyEvent.KEYCODE_SHIFT_RIGHT -> "ShiftRight";
            case KeyEvent.KEYCODE_CTRL_LEFT -> "ControlLeft";
            case KeyEvent.KEYCODE_CTRL_RIGHT -> "ControlRight";
            case KeyEvent.KEYCODE_ALT_LEFT -> "AltLeft";
            case KeyEvent.KEYCODE_ALT_RIGHT -> "AltRight";
            case KeyEvent.KEYCODE_META_LEFT -> "MetaLeft";
            case KeyEvent.KEYCODE_META_RIGHT -> "MetaRight";
            case KeyEvent.KEYCODE_FUNCTION -> "Fn";
            case KeyEvent.KEYCODE_MENU -> "ContextMenu";
            case KeyEvent.KEYCODE_NUMPAD_DOT -> "NumpadDecimal";
            case KeyEvent.KEYCODE_NUMPAD_ENTER -> "NumpadEnter";
            case KeyEvent.KEYCODE_NUMPAD_ADD -> "NumpadAdd";
            case KeyEvent.KEYCODE_NUMPAD_SUBTRACT -> "NumpadSubtract";
            case KeyEvent.KEYCODE_NUMPAD_MULTIPLY -> "NumpadMultiply";
            case KeyEvent.KEYCODE_NUMPAD_DIVIDE -> "NumpadDivide";
            case KeyEvent.KEYCODE_NUM_LOCK -> "NumLock";
            default -> null;
        };
    }

    /** What the statistics page prints on a key, and reads out for it. */
    public static String label(String id) {
        if (id == null) return "";
        if (id.length() == 4 && id.startsWith("Key")) return id.substring(3);
        if (id.length() == 6 && id.startsWith("Digit")) return id.substring(5);
        if (id.length() == 5 && id.startsWith("Nine")) return "九键 " + id.substring(4);
        if (id.startsWith("Fourteen") && id.length() > 8) return "14 键 " + id.substring(8);
        if (id.startsWith("Numpad") && id.length() == 7) return "小键盘 " + id.substring(6);
        if (id.matches("F[0-9]{1,2}")) return id;
        return switch (id) {
            case "Backquote" -> "`";
            case "Minus" -> "-";
            case "Equal" -> "=";
            case "BracketLeft" -> "[";
            case "BracketRight" -> "]";
            case "Backslash" -> "\\";
            case "Semicolon" -> ";";
            case "Quote" -> "'";
            case "Comma" -> ",";
            case "Period" -> ".";
            case "Slash" -> "/";
            case "IntlBackslash" -> "ISO \\";
            case "IntlRo" -> "ろ";
            case "IntlYen" -> "¥";
            case "Lang1" -> "かな";
            case "Lang2" -> "英数";
            case "Convert" -> "変換";
            case "NonConvert" -> "無変換";
            case "KanaMode" -> "カナ";
            case "Space" -> "空格";
            case "Enter" -> "换行";
            case "Backspace" -> "删除";
            case "Tab" -> "Tab";
            case "Escape" -> "Esc";
            case "Delete" -> "向后删除";
            case "Insert" -> "Insert";
            case "Home" -> "Home";
            case "End" -> "End";
            case "PageUp" -> "上翻页";
            case "PageDown" -> "下翻页";
            case "ArrowUp" -> "↑";
            case "ArrowDown" -> "↓";
            case "ArrowLeft" -> "←";
            case "ArrowRight" -> "→";
            case "CapsLock" -> "大写锁定";
            case "ShiftLeft" -> "⇧";
            case "ShiftRight" -> "右 ⇧";
            case "ControlLeft" -> "Ctrl";
            case "ControlRight" -> "右 Ctrl";
            case "AltLeft" -> "Alt";
            case "AltRight" -> "右 Alt";
            case "MetaLeft" -> "Meta";
            case "MetaRight" -> "右 Meta";
            case "Fn" -> "Fn";
            case "ContextMenu" -> "菜单";
            case "NumpadDecimal" -> "小键盘 .";
            case "NumpadEnter" -> "小键盘回车";
            case "NumpadAdd" -> "小键盘 +";
            case "NumpadSubtract" -> "小键盘 -";
            case "NumpadMultiply" -> "小键盘 *";
            case "NumpadDivide" -> "小键盘 /";
            case "NumLock" -> "Num Lock";
            case "SoftPunctuation" -> "标点";
            case "SoftSymbol" -> "符";
            case "SoftLayer" -> "123";
            case "SoftLanguage" -> "中/英";
            case "SoftGlobe" -> "切换";
            case "SoftEmoji" -> "表情";
            case "SoftVoice" -> "语音";
            default -> id;
        };
    }

    /**
     * The soft 26-key keyboard the statistics page draws: the letter rows, then the case and delete keys around the last of them, then the bottom row. Mirrors the faces KeyboardPreview draws for the 26-key layout.
     */
    public static final List<List<String>> SOFT_ROWS = List.of(
        List.of("KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP"),
        List.of("KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL"),
        List.of("ShiftLeft", "KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Backspace"),
        List.of("SoftSymbol", "SoftLayer", "Comma", "Space", "SoftLanguage", "Enter"));

    /** The nine-key grid: three rows of three, then the punctuation column's key and 0 under them. */
    public static final List<List<String>> NINE_ROWS = List.of(
        List.of("Nine1", "Nine2", "Nine3"),
        List.of("Nine4", "Nine5", "Nine6"),
        List.of("Nine7", "Nine8", "Nine9"),
        List.of("SoftPunctuation", "Nine0"));

    /** Whether any nine-key cell was pressed, which is when the page draws the grid at all. */
    public static boolean hasNineKey(Map<String, Long> counts) {
        for (Map.Entry<String, Long> entry : counts.entrySet()) {
            if (entry.getKey().startsWith("Nine") && entry.getValue() > 0) return true;
        }
        return false;
    }

    /**
     * The pressed keys that have no drawn position, most pressed first, for the 其他键 list.
     *
     * @param nineGrid whether the nine-key grid is drawn, so its cells are not listed twice
     */
    public static List<String> others(Map<String, Long> counts, boolean nineGrid) {
        int drawnCapacity = 0;
        for (List<String> row : SOFT_ROWS) drawnCapacity += row.size();
        if (nineGrid) for (List<String> row : NINE_ROWS) drawnCapacity += row.size();
        Set<String> drawn = new LinkedHashSet<>(drawnCapacity);
        for (List<String> row : SOFT_ROWS) drawn.addAll(row);
        if (nineGrid) for (List<String> row : NINE_ROWS) drawn.addAll(row);
        List<String> result = new ArrayList<>(counts.size());
        for (Map.Entry<String, Long> entry : counts.entrySet()) {
            if (entry.getValue() > 0 && !drawn.contains(entry.getKey())) result.add(entry.getKey());
        }
        result.sort(Comparator.comparingLong((String id) -> counts.get(id)).reversed()
            .thenComparingInt(KEY_IDS::indexOf));
        return List.copyOf(result);
    }
}
