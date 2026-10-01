package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 底部功能行的构成：哪些键出现，各自占多宽。
 *
 * <p>The Android host used to scroll one strip that held every control it had, so what the user saw
 * at the bottom of the keyboard was whatever happened to fit. The row is fixed now, which means the
 * composition has to be a decision rather than a side effect, and it is made here where a smoke test
 * can read it without an emulator.
 *
 * <p>No key appears twice. Delete and 大小写 belong to the last of the 26 key rows whenever those
 * rows are on screen; the quanpin nine-key keeps punctuation beside its digits, handwriting keeps
 * delete in its tool column, and the Japanese kana grid brings its own side columns and takes no
 * action row at all.
 */
public final class KeyboardActionRow {
    /** A key the row can carry, in the order the row lays them out. */
    public enum Slot { SYMBOL_PANEL, LAYER, GLOBE, PUNCTUATION, SPACE, LANGUAGE, RETURN }

    /** One laid-out key: its slot and its share of the row's width. */
    public record Entry(Slot slot, float weight) {}

    private static final float NARROW = 1f;
    private static final float SPACE_WEIGHT = 3.4f;
    private static final float RETURN_WEIGHT = 1.4f;

    private KeyboardActionRow() {}

    /**
     * The row for one touch layout.
     *
     * @param touchLayout one of the {@link KeyboardLayout} surface constants
     * @param globe whether the host may switch to another input method
     */
    public static List<Entry> entries(int touchLayout, boolean globe) {
        if (touchLayout == KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT) return List.of();
        List<Entry> entries = new ArrayList<>();
        entries.add(new Entry(Slot.SYMBOL_PANEL, NARROW));
        entries.add(new Entry(Slot.LAYER, NARROW));
        if (globe) entries.add(new Entry(Slot.GLOBE, NARROW));
        // 九键的标点在侧栏里，底排再放一个逗号就是重复。
        if (touchLayout != KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT)
            entries.add(new Entry(Slot.PUNCTUATION, NARROW));
        entries.add(new Entry(Slot.SPACE, SPACE_WEIGHT));
        entries.add(new Entry(Slot.LANGUAGE, NARROW));
        entries.add(new Entry(Slot.RETURN, RETURN_WEIGHT));
        return List.copyOf(entries);
    }

    /**
     * Whether the 26 key rows are the surface being drawn.
     *
     * <p>Handwriting hands its symbol layer to those rows rather than carrying a second grid, so the
     * question is not answered by the touch layout alone.
     */
    public static boolean usesLetterRows(int touchLayout, boolean symbols) {
        if (touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
                || touchLayout == KeyboardLayout.KOREAN_LAYOUT
                || touchLayout == KeyboardLayout.ZHUYIN_LAYOUT) return true;
        return touchLayout == KeyboardLayout.HANDWRITING_LAYOUT && symbols;
    }

    /** Whether the last letter row ends with the delete key. */
    public static boolean rowsCarryDelete(int touchLayout, boolean symbols) {
        return usesLetterRows(touchLayout, symbols);
    }

    /** Whether the last letter row starts with the case key; the digit page and the Dachen rows have no case to shift. */
    public static boolean rowsCarryCase(int touchLayout, boolean symbols) {
        return usesLetterRows(touchLayout, symbols) && !symbols
            && touchLayout != KeyboardLayout.ZHUYIN_LAYOUT;
    }

    /** Width share for the case and delete keys that bracket the last letter row. */
    public static float letterRowEdgeWeight(boolean symbols) { return symbols ? 1.4f : 1.6f; }

    /** The face the layer key prints for the surface it switches. */
    public static String layerTitle(int touchLayout, boolean symbols) {
        if (!symbols) return "123";
        if (touchLayout == KeyboardLayout.KOREAN_LAYOUT) return "한";
        if (touchLayout == KeyboardLayout.ZHUYIN_LAYOUT) return ZhuyinKeyboardLayout.LAYER_TITLE;
        return touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT ? "九键" : "ABC";
    }

    /** Accessibility label matching {@link #layerTitle}. */
    public static String layerDescription(boolean symbols) {
        return symbols ? "切换到字母键盘" : "切换到数字和符号";
    }
}
