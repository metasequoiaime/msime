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
 * <p>No key appears twice. Delete and 大小写 belong to the last of the 26 key rows whenever those rows are on screen; the quanpin nine-key and the stroke keypad keep punctuation in their sidebar and delete in their right column, the Zhuyin nine-key grid carries the symbol panel key, punctuation and delete itself, handwriting keeps delete in its tool column, and the Japanese kana grid brings its own side columns and takes no action row at all.
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
        boolean zhuyinNineKey = touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
        List<Entry> entries = new ArrayList<>();
        // 注音 9 键的网格最后一行已经有 @# 和逗号句号。
        if (!zhuyinNineKey) entries.add(new Entry(Slot.SYMBOL_PANEL, NARROW));
        entries.add(new Entry(Slot.LAYER, NARROW));
        if (globe) entries.add(new Entry(Slot.GLOBE, NARROW));
        // 九键和笔画键盘的标点在侧栏里，底排再放一个逗号就是重复。
        if (touchLayout != KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT
                && touchLayout != KeyboardLayout.STROKE_LAYOUT && !zhuyinNineKey)
            entries.add(new Entry(Slot.PUNCTUATION, NARROW));
        entries.add(new Entry(Slot.SPACE, SPACE_WEIGHT));
        entries.add(new Entry(Slot.LANGUAGE, NARROW));
        entries.add(new Entry(Slot.RETURN, RETURN_WEIGHT));
        return List.copyOf(entries);
    }

    /**
     * Whether the 26 key rows are the surface being drawn.
     *
     * <p>Handwriting, the stroke keypad and the Zhuyin nine-key grid hand their symbol layer to those rows rather than carrying a second grid, so the question is not answered by the touch layout alone.
     */
    public static boolean usesLetterRows(int touchLayout, boolean symbols) {
        if (touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
                || touchLayout == KeyboardLayout.KOREAN_LAYOUT
                || touchLayout == KeyboardLayout.ZHUYIN_LAYOUT) return true;
        return (touchLayout == KeyboardLayout.HANDWRITING_LAYOUT
            || touchLayout == KeyboardLayout.STROKE_LAYOUT
            || touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT) && symbols;
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
        if (touchLayout == KeyboardLayout.STROKE_LAYOUT) return StrokeKeyboardLayout.LAYER_TITLE;
        if (touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT) return ZhuyinNineKeyLayout.LAYER_TITLE;
        return touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT ? "九键" : "ABC";
    }

    /** Accessibility label matching {@link #layerTitle}. */
    public static String layerDescription(boolean symbols) {
        return symbols ? "切换到字母键盘" : "切换到数字和符号";
    }

    // ---- 新设计的底行（plan §2.8，N/design-tokens.md §5）：123 1.25 / 中 1.05 / ， 1 / 空格 4 / 。 1 / ↵ 1.9 ----

    /** 新设计底行的键，按排列顺序；地球键只在宿主允许切换输入法时插在中/英之后。 */
    public enum DesignSlot { LAYER, LANGUAGE, GLOBE, COMMA, SPACE, PERIOD, RETURN }

    /** 新设计底行里的一个键与它的宽度份额。 */
    public record DesignEntry(DesignSlot slot, float weight) {}

    public static final float DESIGN_LAYER_WEIGHT = 1.25f;
    public static final float DESIGN_LANGUAGE_WEIGHT = 1.05f;
    public static final float DESIGN_GLOBE_WEIGHT = 1f;
    public static final float DESIGN_PUNCTUATION_WEIGHT = 1f;
    public static final float DESIGN_SPACE_WEIGHT = 4f;
    public static final float DESIGN_RETURN_WEIGHT = 1.9f;
    /** 新设计第三行 ⇧ 与 ⌫ 的宽度份额。 */
    public static final float DESIGN_LETTER_EDGE_WEIGHT = 1.4f;

    /**
     * 新设计的底行。九键与笔画键盘的标点在侧栏、注音 9 键的逗号句号在网格最后一行，底行不再放逗号句号；日文假名网格不带底行。
     *
     * @param touchLayout {@link KeyboardLayout} 的界面常量
     * @param globe 宿主是否可以切换到下一个输入法（`shouldOfferSwitchingToNextInputMethod()`）
     */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe) {
        if (touchLayout == KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT) return List.of();
        boolean sidebarPunctuation = touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT
            || touchLayout == KeyboardLayout.STROKE_LAYOUT
            || touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT;
        List<DesignEntry> entries = new ArrayList<>();
        entries.add(new DesignEntry(DesignSlot.LAYER, DESIGN_LAYER_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.LANGUAGE, DESIGN_LANGUAGE_WEIGHT));
        if (globe) entries.add(new DesignEntry(DesignSlot.GLOBE, DESIGN_GLOBE_WEIGHT));
        if (!sidebarPunctuation)
            entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_SPACE_WEIGHT));
        if (!sidebarPunctuation)
            entries.add(new DesignEntry(DesignSlot.PERIOD, DESIGN_PUNCTUATION_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.RETURN, DESIGN_RETURN_WEIGHT));
        return List.copyOf(entries);
    }

    /** 新设计底行逗号 / 句号键的键面：中文标点模式 `，` `。`，否则 `,` `.`。 */
    public static String punctuationFace(DesignSlot slot, boolean chinesePunctuation) {
        if (slot == DesignSlot.COMMA) return chinesePunctuation ? "，" : ",";
        if (slot == DesignSlot.PERIOD) return chinesePunctuation ? "。" : ".";
        throw new IllegalArgumentException("Not a punctuation slot: " + slot);
    }

    /** 新设计底行各键的 contentDescription（§2.8）；回车由 {@link ReturnKeyAction} 决定，这里返回空闲时的「换行」。 */
    public static String designDescription(DesignSlot slot, boolean chinesePunctuation) {
        return switch (slot) {
            case LAYER -> layerDescription(false);
            case LANGUAGE -> "切换中英文";
            case GLOBE -> "切换输入法";
            case COMMA, PERIOD -> punctuationFace(slot, chinesePunctuation);
            case SPACE -> "空格";
            case RETURN -> "换行";
        };
    }
}
