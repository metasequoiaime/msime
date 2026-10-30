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
 * <p>No key appears twice. Delete and 大小写 belong to the last of the 26 key rows whenever those rows are on screen; the quanpin nine-key and the stroke keypad keep punctuation in their sidebar and delete in their right column, the Zhuyin nine-key grid carries the symbol panel key, punctuation and delete itself, handwriting's writing pad spans the whole row so its 重写 and delete keys sit in this row, and the Japanese kana grid brings its own side columns and takes no action row at all.
 */
public final class KeyboardActionRow {
    /** A key the row can carry, in the order the row lays them out. */
    public enum Slot { SYMBOL_PANEL, LAYER, GLOBE, PUNCTUATION, SPACE, LANGUAGE, RETURN }

    /** One laid-out key: its slot and its share of the row's width. */
    public record Entry(Slot slot, float weight) {}

    private static final float NARROW = 1f;
    private static final float SPACE_WEIGHT = 3.4f;
    private static final float RETURN_WEIGHT = 1.4f;
    private static final int MAX_ROW_ENTRIES = 7;

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
        List<Entry> entries = new ArrayList<>(MAX_ROW_ENTRIES);
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

    /**
     * Width share for the case and delete keys that bracket the last letter row.
     *
     * <p>The wide edge keys make up for the 26-key letter row holding fewer letters than the rows above. The Dachen row already holds ten keys and carries no case key, so a wide ⌫ there only takes width from ㄈ…ㄥ; it gets one key's share and leaves that row as wide as the digit row.
     */
    public static float letterRowEdgeWeight(int touchLayout, boolean symbols) {
        if (symbols) return 1.4f;
        return touchLayout == KeyboardLayout.ZHUYIN_LAYOUT ? 1f : 1.6f;
    }

    /** 借用数字九宫格时返回键标示「返回」（与 123 层的 {@link KeyboardLayout#lettersKeyTitle} 一致），专属键面仍印各自的短名。 */
    public static String layerTitle(int touchLayout, boolean symbols, boolean chineseNumberPad) {
        String title = layerTitle(touchLayout, symbols);
        return chineseNumberPad && (touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
            || touchLayout == KeyboardLayout.HANDWRITING_LAYOUT) ? KeyboardLayout.lettersKeyTitle(true) : title;
    }

    /** The face the layer key prints for the surface it switches. */
    public static String layerTitle(int touchLayout, boolean symbols) {
        if (!symbols) return "123";
        if (touchLayout == KeyboardLayout.KOREAN_LAYOUT) return "한";
        if (touchLayout == KeyboardLayout.ZHUYIN_LAYOUT) return ZhuyinKeyboardLayout.LAYER_TITLE;
        if (touchLayout == KeyboardLayout.STROKE_LAYOUT) return StrokeKeyboardLayout.LAYER_TITLE;
        if (touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT) return ZhuyinNineKeyLayout.LAYER_TITLE;
        if (touchLayout == KeyboardLayout.FOURTEEN_KEY_LAYOUT) return FourteenKeyLayout.LAYER_TITLE;
        if (touchLayout == KeyboardLayout.EIGHTEEN_KEY_LAYOUT) return EighteenKeyLayout.LAYER_TITLE;
        return touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT ? "九键" : "ABC";
    }

    /** 与 {@link #layerTitle} 对应的无障碍操作说明。 */
    public static String layerDescription(boolean symbols) {
        return symbols ? "返回原键盘" : "切换到数字和符号";
    }

    // ---- 新设计的底行（plan §2.8，N/design-tokens.md §5）：14 键和大千注音是 123 1.25 / ， 1 / 空格 4 / 。 1 / 中 1.05 / ↵ 1.9；手写是 123 1.25 / ， 1 / 空格 2.4 / 。 1 / 中 1.05 / 重写 1.25 / ⌫ 1.25 / ↵ 1.6；26 键是 123 / 符 / 空格 / ， / 中 / ↵，九键外框是 符 / 123 / 空格 / 中 / ↵ ----

    /** 新设计底行的键，按排列顺序；符 在 26 键一族的数字入口之后、九键外框的最左边出现，地球键只在宿主允许切换输入法时插在 123 之后，中/英紧挨在回车左边（手写在两者之间放「重写」和 ⌫）。 */
    public enum DesignSlot { SYMBOL_PANEL, LAYER, GLOBE, EMOJI, COMMA, SPACE, ZERO, PERIOD, LANGUAGE, REWRITE, DELETE, RETURN }

    /** 新设计底行里的一个键与它的宽度份额。 */
    public record DesignEntry(DesignSlot slot, float weight) {}

    /** 26 键一族和九键外框底行 符 的宽度份额，与 中/英 相同（iOS 的这两种底行同样给它 1.05）。 */
    public static final float DESIGN_SYMBOL_PANEL_WEIGHT = 1.05f;
    public static final float DESIGN_LAYER_WEIGHT = 1.25f;
    public static final float DESIGN_LANGUAGE_WEIGHT = 1.05f;
    public static final float DESIGN_GLOBE_WEIGHT = 1f;
    public static final float DESIGN_PUNCTUATION_WEIGHT = 1f;
    public static final float DESIGN_SPACE_WEIGHT = 4f;
    /** 长空格模式把底行符号键的宽度交给空格键。 */
    public static final float DESIGN_LONG_SPACE_WEIGHT = DESIGN_SPACE_WEIGHT + DESIGN_SYMBOL_PANEL_WEIGHT;
    /** 英文 26 键底行在空格左边多一个句点（{@link #DESIGN_PUNCTUATION_WEIGHT}），这一份从空格里让出，其余键与中文 26 键同宽；iOS `englishPhoneSpaceWeight`、HarmonyOS `ENGLISH_SPACE_SHARE` 相同。 */
    public static final float DESIGN_ENGLISH_SPACE_WEIGHT = DESIGN_SPACE_WEIGHT - DESIGN_PUNCTUATION_WEIGHT;
    public static final float DESIGN_RETURN_WEIGHT = 1.9f;
    /** 新设计第三行 ⇧ 与 ⌫ 的宽度份额。 */
    public static final float DESIGN_LETTER_EDGE_WEIGHT = 1.4f;

    /**
     * 手写底行 123 |（地球键 1）| ， | 空格 | 。 | 中 | 重写 | ⌫ | 回车 的宽度份额，iOS（`KeyboardViewController.handwriting*Weight`）与 HarmonyOS（`HANDWRITING_*_SHARE`）相同，由 scripts/test-mobile-handwriting-pad-parity.py 比对。书写区铺满整行后「重写」和 ⌫ 搬进这一行，空格从 4 让到 2.4、回车从 1.9 让到 1.6（与九键底行的回车相同）：360 dp 宽时 ， 。 约 27 dp，与 26 键字母键相当，空格仍画得下麦克风和「手写」。
     */
    public static final float HANDWRITING_LAYER_WEIGHT = 1.25f;
    public static final float HANDWRITING_PUNCTUATION_WEIGHT = 1f;
    public static final float HANDWRITING_SPACE_WEIGHT = 2.4f;
    public static final float HANDWRITING_LANGUAGE_WEIGHT = 1.05f;
    public static final float HANDWRITING_REWRITE_WEIGHT = 1.25f;
    public static final float HANDWRITING_DELETE_WEIGHT = 1.25f;
    public static final float HANDWRITING_RETURN_WEIGHT = 1.6f;

    /**
     * 新设计的底行：123 |（地球）| ， | 空格 | 。 | 中/英 | 回车。中/英放在回车左边，右手拇指够得着；注音 9 键的逗号句号在网格最后一行，日文假名网格不带底行。这一行留给 14 键（第三行左端已经有 符）和大千注音；手写另有一行（{@code handwritingEntries}）。
     *
     * <p>26 键一族（标准 26 键，含全拼、双拼、五笔、英文等，以及韩文键面）是 123 |（地球）| 符 | 空格 | ， | 中/英 | 回车。符 打开符号面板，逗号长按输入句号。英文模式在空格左边另放句点：123 |（地球）| 符 | . | 空格 | , | 中/英 | 回车（{@link #designEntries(int, boolean, boolean, boolean, boolean)}）。123 / #+= 层自带底行，有自己的符号入口。
     *
     * <p>拼音九键和笔画键盘的字母层是 符 | 123 |（地球）| 空格 | 中/英 | 回车。拼音九键数字层由四参数重载排布，0 接替空格、空格移到右列；笔画键盘的数字键面是自带底行的 123 层。
     *
     * @param touchLayout {@link KeyboardLayout} 的界面常量
     * @param globe 宿主是否可以切换到下一个输入法（`shouldOfferSwitchingToNextInputMethod()`）
     */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe) {
        return baseEntries(touchLayout, globe, false, TouchKeyStyle.Row.STANDARD);
    }

    /** 26 键表情样式里 😀 占 符 的位置和宽度；九键表情样式的 😀 同宽，从空格里让出。 */
    public static final float DESIGN_EMOJI_WEIGHT = DESIGN_SYMBOL_PANEL_WEIGHT;
    /** 14 键与 18 键的长空格：句号的一份交给空格。 */
    public static final float DESIGN_FOURTEEN_LONG_SPACE_WEIGHT = DESIGN_SPACE_WEIGHT + DESIGN_PUNCTUATION_WEIGHT;
    /** 26 键「逗号句号在空格两侧」：句号的一份从空格里让出，其余键宽度不变。 */
    public static final float DESIGN_COMMA_PERIOD_SPACE_WEIGHT = DESIGN_SPACE_WEIGHT - DESIGN_PUNCTUATION_WEIGHT;
    /** 九键「底栏带标点」：逗号、句号各从空格里让出一份。 */
    public static final float DESIGN_NINE_COMMA_PERIOD_SPACE_WEIGHT = DESIGN_SPACE_WEIGHT - 2 * DESIGN_PUNCTUATION_WEIGHT;
    /** 九键表情样式：😀 的一份从空格里让出。 */
    public static final float DESIGN_NINE_EMOJI_SPACE_WEIGHT = DESIGN_SPACE_WEIGHT - DESIGN_EMOJI_WEIGHT;

    /**
     * 字母层底行按样式（{@link TouchKeyStyle.Row}）排布，每一行的总宽份额与标准底行相同，样式多出的键从空格里让出宽度：
     *
     * <ul>
     * <li>26 键一族（{@link #usesCommaSecondaryPeriod}）：标准 123 |（地球）| 符 | 空格 | ， | 中/英 | 回车；`COMMA_PERIOD` 123 |（地球）| 符 | ， | 空格 | 。 | 中/英 | 回车；`LONG_SPACE` 123 |（地球）| ， | 长空格 | 中/英 | 回车；`EMOJI` 123 |（地球）| 😀 | 空格 | ， | 中/英 | 回车。英文模式（`english`）画英文底行 123 |（地球）| 符 | . | 空格 | , | 中/英 | 回车；英文 26 键总是这一行，宿主对英文一律传 `STANDARD`（{@link TouchKeyStyle.Styles#forLayout}），排在英文分支之前的 `LONG_SPACE` 只在旧的 {@code longSpace} 重载显式传入时才会遇到英文。</li>
     * <li>14 键、18 键（大千注音共用这一行，但没有可选样式）：标准 123 |（地球）| ， | 空格 | 。 | 中/英 | 回车；`LONG_SPACE` 123 |（地球）| ， | 长空格 | 中/英 | 回车；`EMOJI` 123 |（地球）| 😀 | 空格 | ， | 中/英 | 回车，😀 取原逗号的宽度，逗号取原句号的宽度。</li>
     * <li>拼音九键与笔画外框：标准 符 | 123 |（地球）| 空格 | 中/英 | 回车；`COMMA_PERIOD` 符 | 123 |（地球）| ， | 空格 | 。 | 中/英 | 回车；`EMOJI` 符 | 123 |（地球）| 😀 | 空格 | 中/英 | 回车。</li>
     * </ul>
     *
     * 某类键面不提供的样式按标准底行画。
     *
     * @param english 宿主处在英文模式（{@code dedicatedEnglish}）
     */
    private static List<DesignEntry> baseEntries(int touchLayout, boolean globe, boolean english,
            TouchKeyStyle.Row style) {
        // 日语九键和注音九键的四行键块自带 123、中/英、空格、换行（注音九键的这几个是挪过去的底栏键），不再要底栏。
        if (touchLayout == KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT
                || touchLayout == KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT) return List.of();
        if (touchLayout == KeyboardLayout.HANDWRITING_LAYOUT) return handwritingEntries(globe);
        List<DesignEntry> entries = new ArrayList<>(MAX_ROW_ENTRIES + 1);
        if (touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT || touchLayout == KeyboardLayout.STROKE_LAYOUT) {
            // 九键外框以符号键打头；标点在左侧栏里，标准底行不再放逗号。
            entries.add(new DesignEntry(DesignSlot.SYMBOL_PANEL, DESIGN_SYMBOL_PANEL_WEIGHT));
            entries.add(new DesignEntry(DesignSlot.LAYER, DESIGN_LAYER_WEIGHT));
            if (globe) entries.add(new DesignEntry(DesignSlot.GLOBE, DESIGN_GLOBE_WEIGHT));
            switch (style) {
                case COMMA_PERIOD -> {
                    entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_NINE_COMMA_PERIOD_SPACE_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.PERIOD, DESIGN_PUNCTUATION_WEIGHT));
                }
                case EMOJI -> {
                    entries.add(new DesignEntry(DesignSlot.EMOJI, DESIGN_EMOJI_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_NINE_EMOJI_SPACE_WEIGHT));
                }
                default -> entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_SPACE_WEIGHT));
            }
        } else if (usesCommaSecondaryPeriod(touchLayout)) {
            // 26 键把符号入口放在数字入口之后，逗号放在空格右边。
            entries.add(new DesignEntry(DesignSlot.LAYER, DESIGN_LAYER_WEIGHT));
            if (globe) entries.add(new DesignEntry(DesignSlot.GLOBE, DESIGN_GLOBE_WEIGHT));
            if (style == TouchKeyStyle.Row.LONG_SPACE) {
                entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_LONG_SPACE_WEIGHT));
            } else if (english) {
                entries.add(new DesignEntry(DesignSlot.SYMBOL_PANEL, DESIGN_SYMBOL_PANEL_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.PERIOD, DESIGN_PUNCTUATION_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_ENGLISH_SPACE_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
            } else if (style == TouchKeyStyle.Row.COMMA_PERIOD) {
                entries.add(new DesignEntry(DesignSlot.SYMBOL_PANEL, DESIGN_SYMBOL_PANEL_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_COMMA_PERIOD_SPACE_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.PERIOD, DESIGN_PUNCTUATION_WEIGHT));
            } else {
                entries.add(style == TouchKeyStyle.Row.EMOJI
                    ? new DesignEntry(DesignSlot.EMOJI, DESIGN_EMOJI_WEIGHT)
                    : new DesignEntry(DesignSlot.SYMBOL_PANEL, DESIGN_SYMBOL_PANEL_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_SPACE_WEIGHT));
                entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
            }
        } else {
            // 14 键、18 键和大千注音：逗号句号在空格两侧。
            entries.add(new DesignEntry(DesignSlot.LAYER, DESIGN_LAYER_WEIGHT));
            if (globe) entries.add(new DesignEntry(DesignSlot.GLOBE, DESIGN_GLOBE_WEIGHT));
            switch (style) {
                case LONG_SPACE -> {
                    entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_FOURTEEN_LONG_SPACE_WEIGHT));
                }
                case EMOJI -> {
                    entries.add(new DesignEntry(DesignSlot.EMOJI, DESIGN_PUNCTUATION_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_SPACE_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
                }
                default -> {
                    entries.add(new DesignEntry(DesignSlot.COMMA, DESIGN_PUNCTUATION_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.SPACE, DESIGN_SPACE_WEIGHT));
                    entries.add(new DesignEntry(DesignSlot.PERIOD, DESIGN_PUNCTUATION_WEIGHT));
                }
            }
        }
        entries.add(new DesignEntry(DesignSlot.LANGUAGE, DESIGN_LANGUAGE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.RETURN, DESIGN_RETURN_WEIGHT));
        return List.copyOf(entries);
    }

    /**
     * 手写底行：123 |（地球）| ， | 空格 | 。 | 中/英 | 重写 | ⌫ | 回车。书写区铺满键区宽度，原来书写区两侧的 ⌫、「重写」排在中/英和回车之间；原来左列的 ？ ！ 由逗号、句号的长按输入（{@link #usesHandwritingMarks}）。三个移动端的手写底行一致。
     */
    private static List<DesignEntry> handwritingEntries(boolean globe) {
        List<DesignEntry> entries = new ArrayList<>(9);
        entries.add(new DesignEntry(DesignSlot.LAYER, HANDWRITING_LAYER_WEIGHT));
        if (globe) entries.add(new DesignEntry(DesignSlot.GLOBE, DESIGN_GLOBE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.COMMA, HANDWRITING_PUNCTUATION_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.SPACE, HANDWRITING_SPACE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.PERIOD, HANDWRITING_PUNCTUATION_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.LANGUAGE, HANDWRITING_LANGUAGE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.REWRITE, HANDWRITING_REWRITE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.DELETE, HANDWRITING_DELETE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.RETURN, HANDWRITING_RETURN_WEIGHT));
        return List.copyOf(entries);
    }

    /** 手写底行的逗号长按输入问号、句号长按输入叹号，两键都画角标；它们接替原来书写区左列的 ？ ！。 */
    public static boolean usesHandwritingMarks(int touchLayout) {
        return touchLayout == KeyboardLayout.HANDWRITING_LAYOUT;
    }

    /** 26 键字母层用逗号的长按动作输入句号，其他底行保留原标点菜单。 */
    public static boolean usesCommaSecondaryPeriod(int touchLayout) {
        return touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
            || touchLayout == KeyboardLayout.KOREAN_LAYOUT;
    }

    /** 可选交换语言键与数字键，键的宽度跟随动作，其它键顺序不变。 */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe, boolean swapLanguageNumber) {
        return designEntries(touchLayout, globe, swapLanguageNumber, false);
    }

    /** 九键数字层保留符号和层键的位置，0 位于原空格处；常驻空格键移到右列，手势保持。 */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe,
            boolean swapLanguageNumber, boolean digits) {
        return designEntries(touchLayout, globe, swapLanguageNumber, digits, false);
    }

    /** 同上，英文模式下 26 键字母层在空格左边加句点：123 |（地球）| 符 | . | 空格 | , | 中/英 | 回车（{@link #baseEntries}）。 */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe,
            boolean swapLanguageNumber, boolean digits, boolean english) {
        return designEntries(touchLayout, globe, swapLanguageNumber, digits, english, false);
    }

    /** 长空格模式只改 26 键字母底行；其他布局沿用原来的键位。 */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe,
            boolean swapLanguageNumber, boolean digits, boolean english, boolean longSpace) {
        return designEntries(touchLayout, globe, swapLanguageNumber, digits, english,
            longSpace && usesCommaSecondaryPeriod(touchLayout) ? TouchKeyStyle.Row.LONG_SPACE : TouchKeyStyle.Row.STANDARD);
    }

    /** 按底行样式（{@link #baseEntries}）排布字母层底行；九键数字层不受样式影响，只有右侧大回车例外：那时字母层和数字层都没有单独的底行（{@link #drawsTallReturn}）。「符」与「123」不互换，数字层空格在右列（Android 原来的排法）。 */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe,
            boolean swapLanguageNumber, boolean digits, boolean english, TouchKeyStyle.Row style) {
        return designEntries(touchLayout, globe, swapLanguageNumber, digits, english, style, false,
            TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT);
    }

    /** 数字层底行里与 0 并排的那个小键（空格或 0）的宽度份额，与中/英同宽；iOS `TouchBottomRowLayout.zeroWeight`、鸿蒙 `NineKeyLayout.ZERO_WEIGHT` 相同。 */
    public static final float NUMBER_PAD_SMALL_KEY_WEIGHT = DESIGN_LANGUAGE_WEIGHT;

    /**
     * 同上，另按两项共享偏好排：
     *
     * <ul>
     * <li>{@code symbolFirst}（`touch_twenty_six_symbol_first`）：26 键字母层底行里有 符 时（标准、逗号句号在空格两侧和英文底行）把 符 排到 123 左边：符 | 123 |（地球）| …，与九键外框的前两键同序；长空格和表情键样式没有 符，不变。其他键面的底行本来就以 符 打头或没有 符，也不变。</li>
     * <li>{@code space}（`touch_number_pad_space`）：拼音九键数字层（{@code digits}）里空格放在哪。右列（{@link TouchKeyStyle.NumberPadSpace#RIGHT_COLUMN}）时 0 接替整个空格份额，空格由右列提供；在 0 右边或左边时两键都在底行，左边那个占空格份额减去 {@link #NUMBER_PAD_SMALL_KEY_WEIGHT}，右边那个是 {@link #NUMBER_PAD_SMALL_KEY_WEIGHT}，整行总份额不变。</li>
     * </ul>
     */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe,
            boolean swapLanguageNumber, boolean digits, boolean english, TouchKeyStyle.Row style,
            boolean symbolFirst, TouchKeyStyle.NumberPadSpace space) {
        return designEntries(touchLayout, globe, swapLanguageNumber, digits, english, style, symbolFirst, space, false);
    }

    /**
     * 同上，另按共享偏好 `touch_language_key_side` 排：{@code languageLeft} 为真时，样式、「符」与「123」互换和数字层空格位置都排完之后，把中/英挪到空格左边（{@link #languageBeforeSpace}），宽度不变；「交换语言键与数字键」最后一步在挪好的次序上对调 123 与中/英。手写底行、九键数字层同样作用。
     */
    public static List<DesignEntry> designEntries(int touchLayout, boolean globe,
            boolean swapLanguageNumber, boolean digits, boolean english, TouchKeyStyle.Row style,
            boolean symbolFirst, TouchKeyStyle.NumberPadSpace space, boolean languageLeft) {
        if (drawsTallReturn(touchLayout, style)) return List.of();
        List<DesignEntry> entries = baseEntries(touchLayout, globe, english && !digits,
            digits ? TouchKeyStyle.Row.STANDARD : style);
        if (digits && touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT) {
            List<DesignEntry> numeric = new ArrayList<>(entries.size() + 1);
            TouchKeyStyle.NumberPadSpace resolved = space.resolved();
            for (DesignEntry entry : entries) {
                if (entry.slot() != DesignSlot.SPACE) {
                    numeric.add(entry);
                    continue;
                }
                float wide = entry.weight() - NUMBER_PAD_SMALL_KEY_WEIGHT;
                switch (resolved) {
                    case AFTER_ZERO -> {
                        numeric.add(new DesignEntry(DesignSlot.ZERO, wide));
                        numeric.add(new DesignEntry(DesignSlot.SPACE, NUMBER_PAD_SMALL_KEY_WEIGHT));
                    }
                    case BEFORE_ZERO -> {
                        numeric.add(new DesignEntry(DesignSlot.SPACE, wide));
                        numeric.add(new DesignEntry(DesignSlot.ZERO, NUMBER_PAD_SMALL_KEY_WEIGHT));
                    }
                    default -> numeric.add(new DesignEntry(DesignSlot.ZERO, entry.weight()));
                }
            }
            entries = List.copyOf(numeric);
        } else if (symbolFirst && !digits) {
            entries = symbolFirst(entries);
        }
        if (languageLeft) entries = languageBeforeSpace(entries);
        return swapLanguageNumber ? swapLanguageNumber(entries) : entries;
    }

    /** 中/英左边能被它越过的键：空格、0 和逗号句号。 */
    private static final java.util.Set<DesignSlot> LANGUAGE_PASSES = java.util.EnumSet.of(
        DesignSlot.SPACE, DesignSlot.ZERO, DesignSlot.COMMA, DesignSlot.PERIOD);

    /**
     * 「中/英键放在空格左边」（`touch_language_key_side` 为 `left`）对排好的底行做的一步重排：中/英左边紧挨着的、由空格、0、逗号、句号组成的一串连续的键整串右移一格，中/英挪到这一串的最左边，所有键宽度不变。空格左边没有标点时中/英紧挨空格（123 | 符 | 中 | 空格 | ， | 回车，与豆包、百度九键「系统布局」相同）；空格左边有标点时标点仍紧挨空格，中/英排在标点左边（123 | 符 | 中 | ， | 空格 | 。 | 回车），逗号句号样式不被拆开，中文和英文 26 键的中/英停在同一格。行里没有中/英、或它左边紧挨着的不是这四种键时原样返回。iOS `TouchBottomRowLayout.languageBeforeSpace`、鸿蒙 `TouchLayoutStylePolicy.languageBeforeSpace` 是同一条规则。
     */
    public static List<DesignEntry> languageBeforeSpace(List<DesignEntry> entries) {
        int language = -1;
        for (int index = 0; index < entries.size(); index++) {
            if (entries.get(index).slot() == DesignSlot.LANGUAGE) {
                language = index;
                break;
            }
        }
        if (language < 0) return entries;
        int start = language;
        while (start > 0 && LANGUAGE_PASSES.contains(entries.get(start - 1).slot())) start--;
        if (start == language) return entries;
        List<DesignEntry> moved = new ArrayList<>(entries);
        moved.add(start, moved.remove(language));
        return List.copyOf(moved);
    }

    /** 把排在 123 后面的 符 挪到 123 左边，其余键次序和宽度不变；没有 符 或 符 已经在前面时原样返回。 */
    private static List<DesignEntry> symbolFirst(List<DesignEntry> entries) {
        int layer = -1, symbol = -1;
        for (int index = 0; index < entries.size(); index++) {
            if (entries.get(index).slot() == DesignSlot.LAYER) layer = index;
            if (entries.get(index).slot() == DesignSlot.SYMBOL_PANEL) symbol = index;
        }
        if (layer < 0 || symbol < layer) return entries;
        List<DesignEntry> moved = new ArrayList<>(entries);
        DesignEntry panel = moved.remove(symbol);
        moved.add(layer, panel);
        return List.copyOf(moved);
    }

    /**
     * 数字九宫格的底行，按进入它的键面（{@code sourceLayout}）排。往返字母层和数字层时，数字层的返回键留在字母层 123 那一格，符 留在字母层 符 那一格，不换位置（#9113、#10061）：
     *
     * <ul>
     * <li>字母层底行以 符 打头的键面（拼音九键、九键双拼、笔画）：符 | 九键/原方案 |（地球）| 0 | 中/英 | 回车，与 {@link #designEntries(int, boolean, boolean, boolean, boolean, TouchKeyStyle.Row)} 的九键数字层相同。</li>
     * <li>字母层底行以 123 打头的键面（26 键一族、韩文、14 键、18 键、手写、大千注音）：返回 |（地球）| 符 | 0 | 中/英 | 回车，与 26 键字母层 123 |（地球）| 符 的次序一致；14 键、18 键和手写的字母层底行没有 符，它排在返回和地球键之后。</li>
     * <li>注音九键、日文假名网格这类自带键块、字母层没有底行的键面，沿用九键的次序。</li>
     * </ul>
     *
     * 各键宽度与九键数字层相同，只换次序。「交换语言键与数字键」照旧在排好的次序上对调返回键和中/英；右侧大回车外框仍没有单独的底行（返回空表）。
     *
     * @param sourceLayout 进入数字九宫格之前画着的键面（{@link KeyboardLayout} 的界面常量）
     * @param style 来源键面的底行样式，只用来判断右侧大回车
     */
    public static List<DesignEntry> numberPadEntries(int sourceLayout, boolean globe, boolean swapLanguageNumber,
            TouchKeyStyle.Row style) {
        return numberPadEntries(sourceLayout, globe, swapLanguageNumber, style, false,
            TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT);
    }

    /**
     * 同上，另按两项共享偏好排：{@code symbolFirst} 打开时 26 键字母层底行以 符 打头（{@link #designEntries(int, boolean, boolean, boolean, boolean, TouchKeyStyle.Row, boolean, TouchKeyStyle.NumberPadSpace)}），从它借来的数字九宫格因此按九键的次序排（符 | 返回 | …），返回键仍在字母层 123 那一格；{@code space} 决定 0 和空格的排法。
     *
     * @param style 来源键面的底行样式，用来判断右侧大回车和来源底行有没有 符
     */
    public static List<DesignEntry> numberPadEntries(int sourceLayout, boolean globe, boolean swapLanguageNumber,
            TouchKeyStyle.Row style, boolean symbolFirst, TouchKeyStyle.NumberPadSpace space) {
        return numberPadEntries(sourceLayout, globe, swapLanguageNumber, style, symbolFirst, space, false);
    }

    /** 同上，{@code languageLeft}（`touch_language_key_side` 为 `left`）为真时在返回键和 符 排好之后把中/英挪到 0 和空格左边（{@link #languageBeforeSpace}），再按「交换语言键与数字键」对调；26 键借来的是 返回 | 符 | 中/英 | 0 …，中/英与字母层同一格。 */
    public static List<DesignEntry> numberPadEntries(int sourceLayout, boolean globe, boolean swapLanguageNumber,
            TouchKeyStyle.Row style, boolean symbolFirst, TouchKeyStyle.NumberPadSpace space, boolean languageLeft) {
        List<DesignEntry> entries = designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, globe, false, true, false,
            style, false, space);
        if (!entries.isEmpty() && numberPadLayerFirst(sourceLayout, style, symbolFirst)) {
            List<DesignEntry> reordered = new ArrayList<>(entries);
            int symbol = -1;
            for (int index = 0; index < reordered.size(); index++)
                if (reordered.get(index).slot() == DesignSlot.SYMBOL_PANEL) symbol = index;
            DesignEntry panel = reordered.remove(symbol);
            // 符 跟在返回键（和地球键）后面，与 26 键字母层 123 |（地球）| 符 相同。
            int after = 0;
            for (int index = 0; index < reordered.size(); index++) {
                DesignSlot slot = reordered.get(index).slot();
                if (slot == DesignSlot.LAYER || slot == DesignSlot.GLOBE) after = index + 1;
            }
            reordered.add(after, panel);
            entries = List.copyOf(reordered);
        }
        if (languageLeft) entries = languageBeforeSpace(entries);
        return swapLanguageNumber ? swapLanguageNumber(entries) : entries;
    }

    /** 这个键面的字母层底行是否以 123 打头；是的话借用数字九宫格时返回键也排在第一格（{@link #numberPadEntries}）。 */
    public static boolean numberPadLayerFirst(int sourceLayout) {
        return numberPadLayerFirst(sourceLayout, TouchKeyStyle.Row.STANDARD, false);
    }

    /** 同上，按来源键面此刻的底行样式和「符」与「123」互换判断：互换后 26 键底行以 符 打头，长空格和表情键样式没有 符、仍以 123 打头。 */
    public static boolean numberPadLayerFirst(int sourceLayout, TouchKeyStyle.Row style, boolean symbolFirst) {
        // 右侧大回车没有单独的底行，按九键外框（符 打头）算。
        if (drawsTallReturn(sourceLayout, style)) return false;
        List<DesignEntry> letters = designEntries(sourceLayout, false, false, false, false, style, symbolFirst,
            TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT);
        return !letters.isEmpty() && letters.get(0).slot() == DesignSlot.LAYER;
    }

    /** 打开「符」与「123」互换时，这个键面的字母层底行是否真的换了（原来以 123 打头、换后以 符 打头）。是的话 123 / #+= 层的底行也把返回键排到第二格（{@link KeyboardLayout#numberLayer(boolean, boolean, boolean, boolean, boolean, boolean)}），返回键仍在字母层 123 那一格；长空格、表情键样式和本来就以 符 打头或没有 符 的键面不换。 */
    public static boolean symbolFirstMoves(int touchLayout, TouchKeyStyle.Row style) {
        return numberPadLayerFirst(touchLayout, style, false) && !numberPadLayerFirst(touchLayout, style, true);
    }

    /** 对调层键和中/英，两键的宽度跟着键走；缺任何一个时原样返回。 */
    private static List<DesignEntry> swapLanguageNumber(List<DesignEntry> entries) {
        if (entries.isEmpty()) return entries;
        List<DesignEntry> swapped = new ArrayList<>(entries);
        int layer = -1, language = -1;
        for (int index = 0; index < swapped.size(); index++) {
            if (swapped.get(index).slot() == DesignSlot.LAYER) layer = index;
            if (swapped.get(index).slot() == DesignSlot.LANGUAGE) language = index;
        }
        if (layer >= 0 && language >= 0) java.util.Collections.swap(swapped, layer, language);
        return List.copyOf(swapped);
    }

    /**
     * 这个键面是否画右侧大回车外框：拼音九键与笔画（共用九键外框）选了 {@link TouchKeyStyle.Row#TALL_RETURN}。这时外框连同底行是一整块，像注音九键一样没有单独的底行：回车从右列第三行跨到底行，符 在符号栏下方，九宫格下方是 {@link #tallReturnEntries}。宿主排数字层时传 {@link KeyboardLayout#QUANPIN_NINE_KEY_LAYOUT} 和当前键面的样式，笔画借用的数字九宫格因此同样画这个外框。
     */
    public static boolean drawsTallReturn(int touchLayout, TouchKeyStyle.Row style) {
        return style == TouchKeyStyle.Row.TALL_RETURN && (touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT
            || touchLayout == KeyboardLayout.STROKE_LAYOUT);
    }

    /**
     * 右侧大回车外框左列（符号栏和它下面的 符）与右列（⌫、重输、回车）各自的宽度份额，按当前画着的键面（`displayedTouchLayout`）取：笔画按笔画外框（{@link StrokeKeyboardLayout#SIDE_COLUMN_WEIGHT}），拼音九键和九键双拼按九键外框（{@link NineKeyLayout#SIDE_COLUMN_WEIGHT}）。笔画的数字层借的是九键数字九宫格，键面仍是笔画，所以字母层和数字层取同一组份额，切 123 / 返回时 符、切换键、中/英和回车不横移。
     */
    public static float tallReturnSideWeight(int touchLayout) {
        return touchLayout == KeyboardLayout.STROKE_LAYOUT ? StrokeKeyboardLayout.SIDE_COLUMN_WEIGHT
            : NineKeyLayout.SIDE_COLUMN_WEIGHT;
    }

    /** 同上，中列（九宫格或笔画网格，连同它正下方那一行）的宽度份额。 */
    public static float tallReturnGridWeight(int touchLayout) {
        return touchLayout == KeyboardLayout.STROKE_LAYOUT ? StrokeKeyboardLayout.GRID_WEIGHT : NineKeyLayout.GRID_WEIGHT;
    }

    /** 中列占外框整行宽度的比例，即 {@link #tallReturnEntries} 的 {@code gridShare}：拼音九键 14/20，笔画 3/4.8。 */
    public static float tallReturnGridShare(int touchLayout) {
        float grid = tallReturnGridWeight(touchLayout);
        return grid / (2 * tallReturnSideWeight(touchLayout) + grid);
    }

    /** 右侧大回车外框里数字层的空格：0 右边、从 0 里让出、与中/英同宽（1.05），三端相同；小数点在右列 ⌫ 下面。 */
    public static final float TALL_RETURN_DIGIT_SPACE_WEIGHT = DESIGN_LANGUAGE_WEIGHT;

    /**
     * 右侧大回车外框里九宫格正下方那一行：123 |（地球）| 空格 | 中/英，恰好与九宫格同宽。123、地球键和中/英保持在标准九键底行里的绝对宽度，空格占剩下的宽度；份额的单位与 {@link #designEntries} 相同，总和是标准九键底行（含地球键时连地球键）的总份额乘以九宫格占整行的比例 {@code gridShare}，宿主按九宫格的宽度铺满即可。
     *
     * <p>数字层（{@code digits}）照同一框架：123 的位置是切换键（{@link DesignSlot#LAYER}），空格的位置放 0（{@link DesignSlot#ZERO}），0 右边是空格（{@link DesignSlot#SPACE}，{@link #TALL_RETURN_DIGIT_SPACE_WEIGHT}），0 占剩下的宽度；小数点在右列重输的位置，与 iOS、鸿蒙相同。交换中/英与 123 时两键对调，宽度跟着键走。
     *
     * @param gridShare 九宫格占外框整行宽度的比例（拼音九键 14/20，笔画 3/4.8）
     */
    public static List<DesignEntry> tallReturnEntries(float gridShare, boolean globe,
            boolean swapLanguageNumber, boolean digits) {
        return tallReturnEntries(gridShare, globe, swapLanguageNumber, digits, false);
    }

    /** 同上，{@code languageLeft}（`touch_language_key_side` 为 `left`）为真时中/英挪到空格（数字层是 0）左边：123 |（地球）| 中/英 | 空格，数字层 切换 |（地球）| 中/英 | 0 | 空格（{@link #languageBeforeSpace}），各键宽度不变；交换中/英与 123 在挪好的次序上按键对调。 */
    public static List<DesignEntry> tallReturnEntries(float gridShare, boolean globe,
            boolean swapLanguageNumber, boolean digits, boolean languageLeft) {
        float total = 0;
        for (DesignEntry entry : designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, globe)) total += entry.weight();
        float fixed = DESIGN_LAYER_WEIGHT + (globe ? DESIGN_GLOBE_WEIGHT : 0) + DESIGN_LANGUAGE_WEIGHT
            + (digits ? TALL_RETURN_DIGIT_SPACE_WEIGHT : 0);
        List<DesignEntry> entries = new ArrayList<>(5);
        entries.add(new DesignEntry(DesignSlot.LAYER, DESIGN_LAYER_WEIGHT));
        if (globe) entries.add(new DesignEntry(DesignSlot.GLOBE, DESIGN_GLOBE_WEIGHT));
        entries.add(new DesignEntry(digits ? DesignSlot.ZERO : DesignSlot.SPACE, total * gridShare - fixed));
        if (digits) entries.add(new DesignEntry(DesignSlot.SPACE, TALL_RETURN_DIGIT_SPACE_WEIGHT));
        entries.add(new DesignEntry(DesignSlot.LANGUAGE, DESIGN_LANGUAGE_WEIGHT));
        List<DesignEntry> laid = languageLeft ? languageBeforeSpace(entries) : List.copyOf(entries);
        // 按键对调 123 与中/英，不按首尾两格：中/英挪到左边后它不在最后一格。
        return swapLanguageNumber ? swapLanguageNumber(laid) : laid;
    }

    /**
     * 紧挨在中/英右边的回车让给中/英的那几 dp。两键之间拿不准的按下算成中/英：误按中/英，再按一下就回来了；误按回车，消息就发出去了。回车约是中/英的两倍宽，让出这一点不影响正常按它。鸿蒙的 `KeyboardGeometry.RETURN_YIELD_VP` 与 iOS 的 `KeyGapRouting.returnYield` 是同一个值。
     */
    public static final float RETURN_YIELD_DP = 6f;

    /** 底行里中/英是否紧挨在回车左边，这时回车让出 {@link #RETURN_YIELD_DP}。 */
    public static boolean languageBesideReturn(List<DesignEntry> entries) {
        for (int index = 1; index < entries.size(); index++) {
            if (entries.get(index).slot() == DesignSlot.RETURN)
                return entries.get(index - 1).slot() == DesignSlot.LANGUAGE;
        }
        return false;
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
            case SYMBOL_PANEL -> "打开符号面板";
            case LAYER -> layerDescription(false);
            case LANGUAGE -> "切换中英文";
            case GLOBE -> "切换输入法";
            case EMOJI -> "表情";
            case COMMA, PERIOD -> punctuationFace(slot, chinesePunctuation);
            case SPACE -> "空格";
            case ZERO -> "数字 0";
            case REWRITE -> "清空手写";
            case DELETE -> "删除";
            case RETURN -> "换行";
        };
    }
}
