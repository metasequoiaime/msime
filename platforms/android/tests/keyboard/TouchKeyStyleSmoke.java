package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 触屏按键布局样式（.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md）：三类键面每种样式底行的键序与宽度份额逐个钉住，样式多出的键从空格里让宽度、整行总份额不变；偏好写法的宽松读取；哪些键面读哪一类样式；顶部数字行的门槛；各版本列出哪几组样式、是否列「按键布局」入口；本机旧设置迁进共享偏好的规则。iOS 与鸿蒙要画出同样的行。
 */
public final class TouchKeyStyleSmoke {
    private static final KeyboardActionRow.DesignSlot SYMBOL = KeyboardActionRow.DesignSlot.SYMBOL_PANEL;
    private static final KeyboardActionRow.DesignSlot LAYER = KeyboardActionRow.DesignSlot.LAYER;
    private static final KeyboardActionRow.DesignSlot GLOBE = KeyboardActionRow.DesignSlot.GLOBE;
    private static final KeyboardActionRow.DesignSlot EMOJI = KeyboardActionRow.DesignSlot.EMOJI;
    private static final KeyboardActionRow.DesignSlot COMMA = KeyboardActionRow.DesignSlot.COMMA;
    private static final KeyboardActionRow.DesignSlot SPACE = KeyboardActionRow.DesignSlot.SPACE;
    private static final KeyboardActionRow.DesignSlot PERIOD = KeyboardActionRow.DesignSlot.PERIOD;
    private static final KeyboardActionRow.DesignSlot LANGUAGE = KeyboardActionRow.DesignSlot.LANGUAGE;
    private static final KeyboardActionRow.DesignSlot RETURN = KeyboardActionRow.DesignSlot.RETURN;

    public static void main(String[] args) {
        parsing();
        faces();
        twentySixKeyRows();
        fourteenKeyRows();
        nineKeyRows();
        tallReturn();
        symbolFirst();
        numberPadSpace();
        languageLeft();
        numberRow();
        groups();
        migration();
        backupRestoration();
        System.out.println("touch key layout styles: passed");
    }

    private static void parsing() {
        check(TouchKeyStyle.Row.STANDARD.wire.equals("standard") && TouchKeyStyle.Row.COMMA_PERIOD.wire.equals("comma_period")
            && TouchKeyStyle.Row.LONG_SPACE.wire.equals("long_space") && TouchKeyStyle.Row.EMOJI.wire.equals("emoji"),
            "wire strings match client-core");
        check(TouchKeyStyle.Row.TALL_RETURN.wire.equals("tall_return"), "tall_return matches client-core");
        check(TouchKeyStyle.NINE_KEY_ROWS.equals(List.of(TouchKeyStyle.Row.STANDARD, TouchKeyStyle.Row.COMMA_PERIOD,
            TouchKeyStyle.Row.EMOJI, TouchKeyStyle.Row.TALL_RETURN)), "右侧大回车 is the fourth nine-key option, after 表情键");
        check(TouchKeyStyle.parse("tall_return", TouchKeyStyle.NINE_KEY_ROWS) == TouchKeyStyle.Row.TALL_RETURN,
            "nine keys read tall_return");
        check(TouchKeyStyle.parse("tall_return", TouchKeyStyle.TWENTY_SIX_KEY_ROWS) == TouchKeyStyle.Row.STANDARD
            && TouchKeyStyle.parse("tall_return", TouchKeyStyle.FOURTEEN_KEY_ROWS) == TouchKeyStyle.Row.STANDARD,
            "only the nine-key frame offers tall_return");
        check(new TouchKeyStyle.Styles(TouchKeyStyle.Row.TALL_RETURN, TouchKeyStyle.Row.TALL_RETURN, TouchKeyStyle.Row.TALL_RETURN)
            .equals(new TouchKeyStyle.Styles(TouchKeyStyle.Row.STANDARD, TouchKeyStyle.Row.STANDARD, TouchKeyStyle.Row.TALL_RETURN)),
            "tall_return on a 26 or 14 key face falls back to standard");
        check(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE.equals("touch_twenty_six_key_style")
            && TouchKeyStyle.FOURTEEN_KEY_PREFERENCE.equals("touch_fourteen_key_style")
            && TouchKeyStyle.NINE_KEY_PREFERENCE.equals("touch_nine_key_style")
            && TouchKeyStyle.NUMBER_ROW_PREFERENCE.equals("touch_number_row"), "preference keys match client-core");
        for (TouchKeyStyle.Row row : TouchKeyStyle.TWENTY_SIX_KEY_ROWS)
            check(TouchKeyStyle.parse(row.wire, TouchKeyStyle.TWENTY_SIX_KEY_ROWS) == row, "26 key round trip " + row);
        check(TouchKeyStyle.parse(null, TouchKeyStyle.TWENTY_SIX_KEY_ROWS) == TouchKeyStyle.Row.STANDARD, "missing key reads standard");
        check(TouchKeyStyle.parse("split_space", TouchKeyStyle.TWENTY_SIX_KEY_ROWS) == TouchKeyStyle.Row.STANDARD,
            "an unknown style from a newer version reads standard");
        check(TouchKeyStyle.parse("LONG_SPACE", TouchKeyStyle.TWENTY_SIX_KEY_ROWS) == TouchKeyStyle.Row.STANDARD,
            "the wire form is exact");
        check(TouchKeyStyle.parse("comma_period", TouchKeyStyle.FOURTEEN_KEY_ROWS) == TouchKeyStyle.Row.STANDARD,
            "14 and 18 keys have no comma_period");
        check(TouchKeyStyle.parse("long_space", TouchKeyStyle.NINE_KEY_ROWS) == TouchKeyStyle.Row.STANDARD,
            "nine keys have no long_space");
        TouchKeyStyle.Styles parsed = TouchKeyStyle.Styles.parse("emoji", "long_space", "comma_period");
        check(parsed.equals(new TouchKeyStyle.Styles(TouchKeyStyle.Row.EMOJI, TouchKeyStyle.Row.LONG_SPACE,
            TouchKeyStyle.Row.COMMA_PERIOD)), "three styles parse independently");
        check(TouchKeyStyle.Styles.parse(null, null, null).equals(TouchKeyStyle.Styles.DEFAULT), "defaults are all standard");
        check(new TouchKeyStyle.Styles(TouchKeyStyle.Row.LONG_SPACE, TouchKeyStyle.Row.COMMA_PERIOD, TouchKeyStyle.Row.LONG_SPACE)
            .equals(new TouchKeyStyle.Styles(TouchKeyStyle.Row.LONG_SPACE, TouchKeyStyle.Row.STANDARD, TouchKeyStyle.Row.STANDARD)),
            "a style a face cannot draw falls back to standard");
    }

    /** 哪些键面读哪一类样式：中文 26 键读 26 键样式，英文、韩文和其他语言的 26 键保留标准底行。 */
    private static void faces() {
        TouchKeyStyle.Styles styles = new TouchKeyStyle.Styles(TouchKeyStyle.Row.EMOJI, TouchKeyStyle.Row.LONG_SPACE,
            TouchKeyStyle.Row.COMMA_PERIOD);
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN, InputSchemeTraits.WUBI,
                InputSchemeTraits.CANTONESE, -1})
            check(styles.forLayout(standard, scheme, false) == TouchKeyStyle.Row.EMOJI, "Chinese 26 key reads its style " + scheme);
        check(styles.forLayout(standard, InputSchemeTraits.QUANPIN, true) == TouchKeyStyle.Row.STANDARD,
            "English 26 key keeps the standard row");
        for (int scheme : new int[] {InputSchemeTraits.JAPANESE, InputSchemeTraits.VIETNAMESE, InputSchemeTraits.TIBETAN})
            check(styles.forLayout(standard, scheme, false) == TouchKeyStyle.Row.STANDARD, "other languages keep the standard row");
        check(styles.forLayout(KeyboardLayout.KOREAN_LAYOUT, InputSchemeTraits.KOREAN, false) == TouchKeyStyle.Row.STANDARD,
            "Korean keeps the standard row");
        check(styles.forLayout(KeyboardLayout.FOURTEEN_KEY_LAYOUT, InputSchemeTraits.QUANPIN, false) == TouchKeyStyle.Row.LONG_SPACE
            && styles.forLayout(KeyboardLayout.EIGHTEEN_KEY_LAYOUT, InputSchemeTraits.SHUANGPIN, false) == TouchKeyStyle.Row.LONG_SPACE,
            "14 and 18 keys share one style");
        check(styles.forLayout(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, InputSchemeTraits.QUANPIN, false) == TouchKeyStyle.Row.COMMA_PERIOD
            && styles.forLayout(KeyboardLayout.STROKE_LAYOUT, InputSchemeTraits.STROKE, false) == TouchKeyStyle.Row.COMMA_PERIOD,
            "pinyin nine key and stroke share the nine-key frame");
        for (int layout : new int[] {KeyboardLayout.HANDWRITING_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT,
                KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT})
            check(styles.forLayout(layout, -1, false) == TouchKeyStyle.Row.STANDARD, "no styles on layout " + layout);
        toolbarRow();
    }

    /** 长空格样式留着的工具栏行只看用户的 26 键样式和这块键盘的中文键面，不带中/英参数：同一块键盘切到英文时宿主传的仍是中文键面，结果不变，键盘高度不跳。 */
    private static void toolbarRow() {
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        TouchKeyStyle.Styles longSpace = new TouchKeyStyle.Styles(TouchKeyStyle.Row.LONG_SPACE, TouchKeyStyle.Row.LONG_SPACE,
            TouchKeyStyle.Row.STANDARD);
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN, InputSchemeTraits.WUBI,
                InputSchemeTraits.CANTONESE, -1})
            check(longSpace.keepsToolbarRow(standard, scheme), "a Chinese 26 key keyboard keeps the toolbar row " + scheme);
        // 英文 26 键自己画标准底行，但同一块键盘的工具栏行不随中/英出没。
        check(longSpace.forLayout(standard, InputSchemeTraits.QUANPIN, true) == TouchKeyStyle.Row.STANDARD
            && longSpace.keepsToolbarRow(standard, InputSchemeTraits.QUANPIN), "English on the same keyboard keeps the row");
        for (int scheme : new int[] {InputSchemeTraits.JAPANESE, InputSchemeTraits.VIETNAMESE, InputSchemeTraits.TIBETAN})
            check(!longSpace.keepsToolbarRow(standard, scheme), "other languages draw no long space and keep no row " + scheme);
        check(!longSpace.keepsToolbarRow(KeyboardLayout.KOREAN_LAYOUT, InputSchemeTraits.KOREAN), "Korean keeps no row");
        // 14/18 键的长空格不挪 符，它们和它们的英文 26 键都不留这一行。
        for (int layout : new int[] {KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT,
                KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.STROKE_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT})
            check(!longSpace.keepsToolbarRow(layout, InputSchemeTraits.QUANPIN), "no row on layout " + layout);
        for (TouchKeyStyle.Row other : List.of(TouchKeyStyle.Row.STANDARD, TouchKeyStyle.Row.COMMA_PERIOD, TouchKeyStyle.Row.EMOJI))
            check(!new TouchKeyStyle.Styles(other, TouchKeyStyle.Row.LONG_SPACE, TouchKeyStyle.Row.STANDARD)
                .keepsToolbarRow(standard, InputSchemeTraits.QUANPIN), "only long_space keeps the row, not " + other);
    }

    private static void twentySixKeyRows() {
        int layout = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        expect(layout, TouchKeyStyle.Row.STANDARD, entries(LAYER, 1.25f, SYMBOL, 1.05f, SPACE, 4f, COMMA, 1f, LANGUAGE, 1.05f, RETURN, 1.9f));
        expect(layout, TouchKeyStyle.Row.COMMA_PERIOD,
            entries(LAYER, 1.25f, SYMBOL, 1.05f, COMMA, 1f, SPACE, 3f, PERIOD, 1f, LANGUAGE, 1.05f, RETURN, 1.9f));
        expect(layout, TouchKeyStyle.Row.LONG_SPACE, entries(LAYER, 1.25f, COMMA, 1f, SPACE, 5.05f, LANGUAGE, 1.05f, RETURN, 1.9f));
        expect(layout, TouchKeyStyle.Row.EMOJI, entries(LAYER, 1.25f, EMOJI, 1.05f, SPACE, 4f, COMMA, 1f, LANGUAGE, 1.05f, RETURN, 1.9f));
        check(row(layout, false, TouchKeyStyle.Row.STANDARD).equals(KeyboardActionRow.designEntries(layout, false)),
            "the standard style is today's row");
        check(row(layout, true, TouchKeyStyle.Row.STANDARD).equals(KeyboardActionRow.designEntries(layout, true)),
            "the standard style is today's row with the globe");
        check(row(layout, false, TouchKeyStyle.Row.LONG_SPACE)
                .equals(KeyboardActionRow.designEntries(layout, false, false, false, false, true)),
            "long_space is exactly the earlier Android long-space row");
        // 英文模式不受样式影响（长空格沿用旧长空格行的写法），宿主对英文一律传标准样式。
        for (TouchKeyStyle.Row style : List.of(TouchKeyStyle.Row.COMMA_PERIOD, TouchKeyStyle.Row.EMOJI))
            check(KeyboardActionRow.designEntries(layout, false, false, false, true, style)
                    .equals(KeyboardActionRow.designEntries(layout, false, false, false, true)),
                "English 26 key draws the English row whatever the style");
    }

    private static void fourteenKeyRows() {
        for (int layout : new int[] {KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT}) {
            expect(layout, TouchKeyStyle.Row.STANDARD, entries(LAYER, 1.25f, COMMA, 1f, SPACE, 4f, PERIOD, 1f, LANGUAGE, 1.05f, RETURN, 1.9f));
            expect(layout, TouchKeyStyle.Row.LONG_SPACE, entries(LAYER, 1.25f, COMMA, 1f, SPACE, 5f, LANGUAGE, 1.05f, RETURN, 1.9f));
            // 😀 在原逗号的位置、取原逗号的宽度；逗号挪到原句号的位置、取原句号的宽度。
            expect(layout, TouchKeyStyle.Row.EMOJI, entries(LAYER, 1.25f, EMOJI, 1f, SPACE, 4f, COMMA, 1f, LANGUAGE, 1.05f, RETURN, 1.9f));
            expect(layout, TouchKeyStyle.Row.COMMA_PERIOD, row(layout, false, TouchKeyStyle.Row.STANDARD));
            check(row(layout, false, TouchKeyStyle.Row.STANDARD).equals(KeyboardActionRow.designEntries(layout, false)),
                "the standard 14/18 style is today's row");
        }
        // 大千注音与 14 键共用这一行，但没有可选样式，宿主传标准样式。
        check(row(KeyboardLayout.ZHUYIN_LAYOUT, false, TouchKeyStyle.Row.STANDARD)
            .equals(KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_LAYOUT, false)), "Dachen keeps its row");
    }

    private static void nineKeyRows() {
        for (int layout : new int[] {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.STROKE_LAYOUT}) {
            expect(layout, TouchKeyStyle.Row.STANDARD, entries(SYMBOL, 1.05f, LAYER, 1.25f, SPACE, 4f, LANGUAGE, 1.05f, RETURN, 1.9f));
            expect(layout, TouchKeyStyle.Row.COMMA_PERIOD,
                entries(SYMBOL, 1.05f, LAYER, 1.25f, COMMA, 1f, SPACE, 2f, PERIOD, 1f, LANGUAGE, 1.05f, RETURN, 1.9f));
            expect(layout, TouchKeyStyle.Row.EMOJI,
                entries(SYMBOL, 1.05f, LAYER, 1.25f, EMOJI, 1.05f, SPACE, 2.95f, LANGUAGE, 1.05f, RETURN, 1.9f));
            expect(layout, TouchKeyStyle.Row.LONG_SPACE, row(layout, false, TouchKeyStyle.Row.STANDARD));
            check(row(layout, false, TouchKeyStyle.Row.STANDARD).equals(KeyboardActionRow.designEntries(layout, false)),
                "the standard nine-key style is today's row");
        }
        // 九键数字层不随样式变：0 接替空格。右侧大回车另见 tallReturn()。
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        for (TouchKeyStyle.Row style : TouchKeyStyle.NINE_KEY_ROWS) {
            if (style == TouchKeyStyle.Row.TALL_RETURN) continue;
            check(KeyboardActionRow.designEntries(nine, true, false, true, false, style)
                    .equals(KeyboardActionRow.designEntries(nine, true, false, true)),
                "the nine-key number layer ignores " + style);
        }
    }

    /**
     * 右侧大回车（tall_return）：拼音九键、笔画和九键数字层都没有单独的底行，外框连同底行一整块；九宫格正下方那一行恰好与九宫格同宽，123（数字层是切换键）、地球键、中/英保持标准九键底行里的绝对宽度，字母层的空格占剩下的；数字层 0 在空格的位置，右边是与中/英同宽（1.05）的空格，0 占剩下的，小数点在右列，与 iOS、鸿蒙相同。中/英与回车不在同一行，不让出回车的点击区。
     */
    private static void tallReturn() {
        TouchKeyStyle.Row tall = TouchKeyStyle.Row.TALL_RETURN;
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        for (int layout : new int[] {nine, KeyboardLayout.STROKE_LAYOUT}) {
            check(KeyboardActionRow.drawsTallReturn(layout, tall), "tall frame on " + layout);
            for (TouchKeyStyle.Row other : List.of(TouchKeyStyle.Row.STANDARD, TouchKeyStyle.Row.COMMA_PERIOD, TouchKeyStyle.Row.EMOJI))
                check(!KeyboardActionRow.drawsTallReturn(layout, other), "no tall frame for " + other + " on " + layout);
            for (boolean globe : new boolean[] {false, true}) {
                for (boolean swap : new boolean[] {false, true}) {
                    for (boolean digits : new boolean[] {false, true}) {
                        List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.designEntries(layout, globe, swap, digits, false, tall);
                        check(entries.isEmpty() && !KeyboardActionRow.languageBesideReturn(entries),
                            "the tall frame owns its bottom row on " + layout);
                    }
                }
            }
        }
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.FOURTEEN_KEY_LAYOUT,
                KeyboardLayout.EIGHTEEN_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT, KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT})
            check(!KeyboardActionRow.drawsTallReturn(layout, tall), "no tall frame on " + layout);
        // 拼音九键：九宫格占外框 14/20；标准九键底行总份额 9.25（带地球键 10.25）。
        float nineShare = NineKeyLayout.GRID_WEIGHT / (2 * NineKeyLayout.SIDE_COLUMN_WEIGHT + NineKeyLayout.GRID_WEIGHT);
        expectTall(nineShare, false, false, false, entries(LAYER, 1.25f, SPACE, 9.25f * 0.7f - 2.3f, LANGUAGE, 1.05f));
        expectTall(nineShare, true, false, false, entries(LAYER, 1.25f, GLOBE, 1f, SPACE, 10.25f * 0.7f - 3.3f, LANGUAGE, 1.05f));
        expectTall(nineShare, false, true, false, entries(LANGUAGE, 1.05f, SPACE, 9.25f * 0.7f - 2.3f, LAYER, 1.25f));
        expectTall(nineShare, false, false, true,
            entries(LAYER, 1.25f, KeyboardActionRow.DesignSlot.ZERO, 9.25f * 0.7f - 3.35f, SPACE, 1.05f, LANGUAGE, 1.05f));
        expectTall(nineShare, true, false, true, entries(LAYER, 1.25f, GLOBE, 1f, KeyboardActionRow.DesignSlot.ZERO,
            10.25f * 0.7f - 4.35f, SPACE, 1.05f, LANGUAGE, 1.05f));
        expectTall(nineShare, true, true, true, entries(LANGUAGE, 1.05f, GLOBE, 1f, KeyboardActionRow.DesignSlot.ZERO,
            10.25f * 0.7f - 4.35f, SPACE, 1.05f, LAYER, 1.25f));
        check(KeyboardActionRow.TALL_RETURN_DIGIT_SPACE_WEIGHT == 1.05f, "the number layer's space is 1.05, as wide as 中/英");
        // 笔画外框 0.9 / 3 / 0.9：同一行按笔画网格的比例变窄，变的只有空格（数字层是 0）。
        expectTall(3f / 4.8f, false, false, false, entries(LAYER, 1.25f, SPACE, 9.25f * 0.625f - 2.3f, LANGUAGE, 1.05f));
        expectTall(3f / 4.8f, false, false, true, entries(LAYER, 1.25f, KeyboardActionRow.DesignSlot.ZERO,
            9.25f * 0.625f - 3.35f, SPACE, 1.05f, LANGUAGE, 1.05f));
        frameColumns();
        for (float share : new float[] {nineShare, 3f / 4.8f}) {
            for (boolean globe : new boolean[] {false, true}) {
                for (boolean digits : new boolean[] {false, true}) {
                    List<KeyboardActionRow.DesignEntry> standard = KeyboardActionRow.designEntries(nine, globe);
                    List<KeyboardActionRow.DesignEntry> strip = KeyboardActionRow.tallReturnEntries(share, globe, false, digits);
                    check(Math.abs(total(strip) - total(standard) * share) < 1e-4f, "the strip spans exactly the grid width");
                    // 123（数字层的切换键）、地球键、中/英在整行里的占比与同一地球键状态的标准九键底行相同。
                    for (KeyboardActionRow.DesignSlot slot : List.of(LAYER, GLOBE, LANGUAGE)) {
                        int index = slots(strip).indexOf(slot);
                        check(index >= 0 == (slot != GLOBE || globe), slot + " is drawn when expected, digits " + digits);
                        if (index < 0) continue;
                        float tallFraction = strip.get(index).weight() * share / total(strip);
                        float standardFraction = standard.get(slots(standard).indexOf(slot)).weight() / total(standard);
                        check(Math.abs(tallFraction - standardFraction) < 1e-5f, slot + " keeps its absolute width, digits " + digits);
                    }
                    // 小数点在右列 ⌫ 下面，不在九宫格下方那一行。
                    check(!slots(strip).contains(PERIOD), "the decimal point is not in the strip, digits " + digits);
                }
            }
        }
    }

    /**
     * 外框三列的宽度按画着的键面取：拼音九键 3 / 14 / 3，笔画 0.9 / 3 / 0.9。笔画的数字层借的是九键数字九宫格（`rebuildNineKeyRows`），键面仍是笔画，两层取同一组份额：符（左列）、123 / 返回、中/英和回车（右列）在整行里的位置与宽度不随切层变。原来数字层按九键份额画，符 和回车从 18.75% 缩到 15%，123 往左挪了 3.75%。
     */
    private static void frameColumns() {
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        int stroke = KeyboardLayout.STROKE_LAYOUT;
        check(KeyboardActionRow.tallReturnSideWeight(nine) == NineKeyLayout.SIDE_COLUMN_WEIGHT
            && KeyboardActionRow.tallReturnGridWeight(nine) == NineKeyLayout.GRID_WEIGHT, "pinyin nine-key keeps the 3 / 14 / 3 frame");
        check(KeyboardActionRow.tallReturnSideWeight(stroke) == StrokeKeyboardLayout.SIDE_COLUMN_WEIGHT
            && KeyboardActionRow.tallReturnGridWeight(stroke) == StrokeKeyboardLayout.GRID_WEIGHT, "stroke keeps the 0.9 / 3 / 0.9 frame");
        check(StrokeKeyboardLayout.SIDE_COLUMN_WEIGHT == 0.9f && StrokeKeyboardLayout.GRID_WEIGHT == 3f, "stroke column weights");
        check(Math.abs(KeyboardActionRow.tallReturnGridShare(nine) - 0.7f) < 1e-6f, "pinyin nine-key grid share 14/20");
        check(Math.abs(KeyboardActionRow.tallReturnGridShare(stroke) - 0.625f) < 1e-6f, "stroke grid share 3/4.8");
        for (int layout : new int[] {nine, stroke}) {
            float side = KeyboardActionRow.tallReturnSideWeight(layout);
            float row = 2 * side + KeyboardActionRow.tallReturnGridWeight(layout);
            float share = KeyboardActionRow.tallReturnGridShare(layout);
            for (boolean globe : new boolean[] {false, true}) {
                for (boolean swap : new boolean[] {false, true}) {
                    // 字母层和数字层的中列那一行：最左一键（123 或交换后的中/英）从左列右边开始，宽度一样；最右一键贴着右列。
                    List<KeyboardActionRow.DesignEntry> letters = KeyboardActionRow.tallReturnEntries(share, globe, swap, false);
                    List<KeyboardActionRow.DesignEntry> digits = KeyboardActionRow.tallReturnEntries(share, globe, swap, true);
                    float lettersFirst = letters.get(0).weight() / total(letters) * share;
                    float digitsFirst = digits.get(0).weight() / total(digits) * share;
                    float lettersLast = letters.get(letters.size() - 1).weight() / total(letters) * share;
                    float digitsLast = digits.get(digits.size() - 1).weight() / total(digits) * share;
                    check(letters.get(0).slot() == digits.get(0).slot() && Math.abs(lettersFirst - digitsFirst) < 1e-6f
                        && Math.abs(lettersLast - digitsLast) < 1e-6f, "the strip's outer keys do not move between layers on " + layout);
                    check(Math.abs(total(letters) - total(digits)) < 1e-4f, "both layers' strips span the same grid on " + layout);
                }
            }
            check(Math.abs(side / row - (layout == stroke ? 0.1875f : 0.15f)) < 1e-6f, "符 and the return column share on " + layout);
        }
    }

    private static void expectTall(float share, boolean globe, boolean swap, boolean digits,
            List<KeyboardActionRow.DesignEntry> expected) {
        List<KeyboardActionRow.DesignEntry> actual = KeyboardActionRow.tallReturnEntries(share, globe, swap, digits);
        check(actual.size() == expected.size(), "tall strip " + globe + swap + digits + ": " + actual);
        for (int index = 0; index < expected.size(); index++) {
            check(actual.get(index).slot() == expected.get(index).slot()
                    && Math.abs(actual.get(index).weight() - expected.get(index).weight()) < 1e-4f,
                "tall strip " + globe + swap + digits + " key " + index + ": " + actual);
        }
    }

    /** 各版本列出哪几组样式（与鸿蒙 `touchKeyLayoutGroups` 同一条规则）：一组都没有时键盘页不列「按键布局」入口，26 键与 14/18 键都没有时子页不列「顶部数字行」。 */
    private static void groups() {
        check(TouchKeyStyle.Groups.offeredBy(AppEdition.FULL).equals(new TouchKeyStyle.Groups(true, true, true)),
            "full offers all three groups");
        check(TouchKeyStyle.Groups.offeredBy(AppEdition.of("pinyin", "quanpin,shuangpin", "quanpin", true))
            .equals(new TouchKeyStyle.Groups(true, true, true)), "pinyin edition offers all three groups");
        TouchKeyStyle.Groups wubi = TouchKeyStyle.Groups.offeredBy(AppEdition.of("wubi", "wubi", "wubi", false));
        check(wubi.equals(new TouchKeyStyle.Groups(true, true, false)) && wubi.any() && wubi.numberRow(),
            "wubi edition has 26 and 14 key faces but no nine-key group");
        TouchKeyStyle.Groups stroke = TouchKeyStyle.Groups.offeredBy(AppEdition.of("stroke", "stroke", "stroke", false));
        check(stroke.equals(new TouchKeyStyle.Groups(false, false, true)) && stroke.any() && !stroke.numberRow(),
            "a stroke-only edition lists the entry but not the number row switch");
        check(TouchKeyStyle.Groups.offeredBy(AppEdition.of("cantonese", "cantonese", "cantonese", false))
            .equals(new TouchKeyStyle.Groups(true, false, false)), "cantonese offers only the 26 key group");
        for (String language : new String[] {"japanese", "vietnamese", "tibetan", "korean", "zhuyin"}) {
            TouchKeyStyle.Groups none = TouchKeyStyle.Groups.offeredBy(AppEdition.of(language, language, language, false));
            check(none.equals(new TouchKeyStyle.Groups(false, false, false)) && !none.any() && !none.numberRow(),
                language + " edition hides the 按键布局 entry");
        }
    }

    private static void numberRow() {
        int portrait = 760;
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.KOREAN_LAYOUT,
                KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT}) {
            check(KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, layout, portrait), "number row on " + layout);
            check(!KeyboardLayout.drawsNumberRow(false, KeyboardLayout.Layer.LETTERS, layout, portrait), "off by default on " + layout);
            check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.SYMBOLS, layout, portrait), "letters layer only on " + layout);
            check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, layout,
                KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP - 1), "hidden in a phone-landscape window on " + layout);
            check(KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, layout,
                KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP), "drawn from the height gate on " + layout);
            // 123 层保留同一排高度，切层时键盘不跳。
            check(KeyboardLayout.drawsSymbolExtraRow(true, KeyboardLayout.Layer.SYMBOLS, layout, portrait),
                "the symbol layer keeps the fifth row on " + layout);
        }
        for (int layout : new int[] {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT,
                KeyboardLayout.STROKE_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT,
                KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT})
            check(!KeyboardLayout.drawsNumberRow(true, KeyboardLayout.Layer.LETTERS, layout, portrait), "no number row on " + layout);
        for (String digit : KeyboardLayout.NUMBER_ROW)
            check(KeyPressIds.forCharacter(digit.charAt(0)).equals("Digit" + digit) && KeyPressIds.isKnown("Digit" + digit),
                "number row keys count as Digit" + digit);
        // 密码框和终端的自动数字行保持原来的范围（26 键一族与韩文），14/18 键只有用户打开共享偏好时才画。
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.KOREAN_LAYOUT}) {
            check(KeyboardLayout.drawsNumberRow(false, true, KeyboardLayout.Layer.LETTERS, layout, portrait),
                "automatic number row on " + layout);
            check(KeyboardLayout.drawsSymbolExtraRow(false, true, KeyboardLayout.Layer.SYMBOLS, layout, portrait),
                "automatic number row keeps the symbol layer height on " + layout);
            check(!KeyboardLayout.drawsNumberRow(false, true, KeyboardLayout.Layer.LETTERS, layout,
                KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP - 1), "automatic number row obeys the height gate on " + layout);
        }
        for (int layout : new int[] {KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT}) {
            check(!KeyboardLayout.drawsNumberRow(false, true, KeyboardLayout.Layer.LETTERS, layout, portrait),
                "no automatic number row on " + layout);
            check(!KeyboardLayout.drawsSymbolExtraRow(false, true, KeyboardLayout.Layer.SYMBOLS, layout, portrait),
                "no automatic symbol extra row on " + layout);
            check(KeyboardLayout.drawsNumberRow(true, false, KeyboardLayout.Layer.LETTERS, layout, portrait)
                && KeyboardLayout.drawsNumberRow(true, true, KeyboardLayout.Layer.LETTERS, layout, portrait),
                "the preference draws the number row on " + layout);
        }
        for (int layout : new int[] {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT,
                KeyboardLayout.STROKE_LAYOUT})
            check(!KeyboardLayout.drawsNumberRow(true, true, KeyboardLayout.Layer.LETTERS, layout, portrait),
                "neither source draws on " + layout);
        check(!KeyboardLayout.drawsNumberRow(false, false, KeyboardLayout.Layer.LETTERS, KeyboardLayout.STANDARD_TOUCH_LAYOUT,
            portrait), "both off draws nothing");
    }

    private static void migration() {
        Map<String, Object> legacy = new LinkedHashMap<>();
        check(!KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)), "nothing to migrate");
        legacy.put(AndroidLocalSettings.NUMBER_ROW, false);
        check(KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)),
            "an explicit false still has to be deleted");
        legacy.clear();
        legacy.put(AndroidLocalSettings.LONG_SPACE_LAYOUT, true);
        check(KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)), "long space pending");
        // #10229：九键滑动输入数字的旧本机键同样要迁，关闭也算（迁完要删）；距离仍是本机设置，不让迁移一直挂着。
        legacy.clear();
        legacy.put(AndroidLocalSettings.NINE_KEY_SWIPE, NineKeySwipePolicy.OFF);
        check(KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)),
            "an explicit nine-key off still has to be deleted");
        legacy.clear();
        legacy.put(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE, 30);
        check(!KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)),
            "the nine-key swipe distance stays local and is not migrated");
        check(!AndroidLocalSettings.spec(AndroidLocalSettings.LONG_SPACE_LAYOUT).synced
            && !AndroidLocalSettings.spec(AndroidLocalSettings.NUMBER_ROW).synced
            && !AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_SWIPE).synced,
            "the legacy keys never travel through android_local");
        nineKeySidebarMigration();
    }

    /**
     * 两张九键左侧符号表的旧键（#10223）：解析得出又不等于出厂表时才放进迁移请求（client-core 只在共享偏好缺键或是空数组时写入，已有的非空表不覆盖，由 preferences/tests.rs 钉住）；出厂旧值、不合规的旧值都不迁。旧键不管迁没迁都算待删，删掉以后再走迁移入口就没有可迁的，用户「恢复默认」后的空表不会被旧表写回。
     */
    private static void nineKeySidebarMigration() {
        String letters = AndroidLocalSettings.NINE_KEY_SYMBOLS;
        String digits = AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS;
        List<String> letterDefaults = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        List<String> digitDefaults = NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS;
        AndroidLocalSettings.Snapshot factory = new AndroidLocalSettings.Snapshot(AndroidLocalSettings.accepted(Map.of(
            letters, NineKeySidebarPolicy.format(letterDefaults), digits, NineKeySidebarPolicy.format(digitDefaults))));
        check(KeyLayoutStyleMigration.pending(factory), "factory-valued legacy tables still have to be deleted");
        check(KeyLayoutStyleMigration.legacyTable(factory, letters, letterDefaults).isEmpty()
            && KeyLayoutStyleMigration.legacyTable(factory, digits, digitDefaults).isEmpty(),
            "a legacy table equal to the factory table is not uploaded as a choice");
        AndroidLocalSettings.Snapshot custom = new AndroidLocalSettings.Snapshot(AndroidLocalSettings.accepted(Map.of(
            letters, "、 ……  ：", digits, "+ - ×")));
        check(KeyLayoutStyleMigration.legacyTable(custom, letters, letterDefaults).equals(List.of("、", "……", "：")),
            "a custom letter table is migrated in order");
        check(KeyLayoutStyleMigration.legacyTable(custom, digits, digitDefaults).equals(List.of("+", "-", "×")),
            "a custom digit table is migrated");
        // 只写了字母表：数字表不在请求里（空表），也照样算待删。
        AndroidLocalSettings.Snapshot lettersOnly = new AndroidLocalSettings.Snapshot(AndroidLocalSettings.accepted(Map.of(
            letters, "+ -")));
        check(KeyLayoutStyleMigration.pending(lettersOnly)
            && KeyLayoutStyleMigration.legacyTable(lettersOnly, digits, digitDefaults).isEmpty(), "a missing legacy key migrates nothing");
        // 不合规的旧文本读文件时就被丢掉（AndroidLocalSettings 的规范化），直接交给解析也不迁。
        check(KeyLayoutStyleMigration.legacyTable("toolong", letterDefaults).isEmpty()
            && KeyLayoutStyleMigration.legacyTable("", letterDefaults).isEmpty(), "an unparsable legacy table migrates nothing");
        check(!KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(AndroidLocalSettings.accepted(Map.of()))),
            "once the legacy keys are gone there is nothing to migrate again");
        check(!AndroidLocalSettings.spec(letters).synced && !AndroidLocalSettings.spec(digits).synced,
            "the legacy tables never travel through android_local");
    }

    /** 恢复旧备份使用备份里的显式值；新格式字段优先，缺失或无效旧键不改当前偏好。 */
    private static void backupRestoration() {
        String oldNumber = AndroidLocalSettings.NUMBER_ROW;
        String oldSpace = AndroidLocalSettings.LONG_SPACE_LAYOUT;
        String number = "platform.android.number_row";
        String style = "platform.android.twenty_six_key_style";
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNumber, false, oldSpace, true), Map.of())
            .equals(Map.of(number, false, style, "long_space")), "旧备份的关闭与长空格覆盖当前选择");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNumber, true, oldSpace, false), Map.of())
            .equals(Map.of(number, true, style, "standard")), "旧备份的开启与标准底行都能恢复");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNumber, false, oldSpace, true),
            Map.of(number, true, style, "emoji")).isEmpty(), "备份同时含新旧格式时以新格式为准");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNumber, "false"), Map.of()).isEmpty(),
            "无效旧值与缺失键不制造默认值");
        check(KeyLayoutStyleMigration.legacyKey(oldNumber) && KeyLayoutStyleMigration.legacyKey(oldSpace)
            && KeyLayoutStyleMigration.legacyKey(AndroidLocalSettings.NINE_KEY_SYMBOLS)
            && KeyLayoutStyleMigration.legacyKey(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS)
            && KeyLayoutStyleMigration.legacyKey(AndroidLocalSettings.NINE_KEY_SWIPE)
            && !KeyLayoutStyleMigration.legacyKey(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE)
            && !KeyLayoutStyleMigration.legacyKey("platform.android.synthetic"), "恢复时只移除五个已转换旧键，九键滑动距离留在本机");
        // 两张符号表：旧备份的文本转成账号文档的写法；等于出厂表或解析不出时写空串（出厂表）；备份里已有新字段时以新字段为准。账号字段名与旧键同名。
        String letters = "platform.android.nine_key_symbols";
        String digits = "platform.android.nine_key_digit_symbols";
        check(letters.equals(AndroidLocalSettings.NINE_KEY_SYMBOLS) && digits.equals(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS),
            "账号字段与本机旧键同名");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(letters, "、  …… ：", digits, "+ -"), Map.of())
            .equals(Map.of(letters, "、 …… ：", digits, "+ -")), "旧备份的自定义符号表恢复成共享写法");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(letters, "， 。 ？", digits, "toolong abcde"), Map.of())
            .equals(Map.of(letters, "", digits, "")), "出厂表和解析不出的旧表恢复成出厂表");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(letters, "、"), Map.of(letters, "+")).isEmpty(),
            "备份已带新字段时不用旧表");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(letters, 3), Map.of()).isEmpty(), "不是文本的旧值不恢复");

        // #10229：旧备份里的九键滑动方向转成同名的同步键，关闭也写；同一备份带着同步键时以它为准，不认识或类型不对的旧值不制造默认值。
        String oldNine = AndroidLocalSettings.NINE_KEY_SWIPE;
        String nine = "platform.android.nine_key_swipe";
        for (String direction : new String[] {"off", "up", "down"})
            check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNine, direction), Map.of())
                .equals(Map.of(nine, direction)), "旧备份的九键滑动 " + direction + " 能恢复");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNine, "down"), Map.of(nine, "up")).isEmpty(),
            "备份同时含九键滑动的新旧格式时以新格式为准");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNine, "sideways"), Map.of()).isEmpty()
            && KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNine, true), Map.of()).isEmpty(),
            "不认识的九键滑动旧值不制造默认值");
        check(KeyLayoutStyleMigration.restoredSharedEdits(Map.of(oldNumber, true, oldNine, "up"), Map.of())
            .equals(Map.of(number, true, nine, "up")), "数字行和九键滑动一起恢复");
    }

    /**
     * 「26 键「符」与「123」互换」（#10056，`touch_twenty_six_symbol_first`）：26 键底行里有 符 时（标准、逗号句号在空格两侧、英文、韩文）符 挪到 123 左边，宽度跟着键走，地球键仍紧跟 123；长空格和表情键没有 符，不变；九键外框、14/18 键和手写本来不以 123 | 符 打头，不变。从互换后的 26 键借来的数字九宫格按九键次序排（符 | 返回），返回键仍在字母层 123 那一格；没有 符 的样式照旧 返回 | 符。
     */
    private static void symbolFirst() {
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        TouchKeyStyle.NumberPadSpace platform = TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT;
        check(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE.equals("touch_twenty_six_symbol_first"), "symbol-first key matches client-core");
        check(slots(KeyboardActionRow.designEntries(standard, false, false, false, false, TouchKeyStyle.Row.STANDARD, true, platform))
            .equals(List.of(SYMBOL, LAYER, SPACE, COMMA, LANGUAGE, RETURN)), "standard 26 key puts 符 first");
        check(slots(KeyboardActionRow.designEntries(standard, true, false, false, false, TouchKeyStyle.Row.STANDARD, true, platform))
            .equals(List.of(SYMBOL, LAYER, GLOBE, SPACE, COMMA, LANGUAGE, RETURN)), "the globe stays right after 123");
        check(slots(KeyboardActionRow.designEntries(standard, false, false, false, false, TouchKeyStyle.Row.COMMA_PERIOD, true, platform))
            .equals(List.of(SYMBOL, LAYER, COMMA, SPACE, PERIOD, LANGUAGE, RETURN)), "comma_period swaps too");
        check(slots(KeyboardActionRow.designEntries(standard, false, false, false, true, TouchKeyStyle.Row.STANDARD, true, platform))
            .equals(List.of(SYMBOL, LAYER, PERIOD, SPACE, COMMA, LANGUAGE, RETURN)), "the English 26 key swaps too");
        check(slots(KeyboardActionRow.designEntries(KeyboardLayout.KOREAN_LAYOUT, false, false, false, false, TouchKeyStyle.Row.STANDARD, true, platform))
            .get(0) == SYMBOL, "Korean swaps too");
        for (TouchKeyStyle.Row style : List.of(TouchKeyStyle.Row.LONG_SPACE, TouchKeyStyle.Row.EMOJI))
            check(KeyboardActionRow.designEntries(standard, false, false, false, false, style, true, platform).equals(row(standard, false, style)),
                style + " has no 符 and is unchanged");
        for (TouchKeyStyle.Row style : TouchKeyStyle.TWENTY_SIX_KEY_ROWS) {
            List<KeyboardActionRow.DesignEntry> swapped = KeyboardActionRow.designEntries(standard, false, false, false, false, style, true, platform);
            check(Math.abs(total(swapped) - total(row(standard, false, style))) < 1e-5f, style + " keeps the row width");
            check(swapped.containsAll(row(standard, false, style)), style + " only reorders the keys");
        }
        for (int layout : new int[] {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
                KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT,
                KeyboardLayout.ZHUYIN_LAYOUT})
            check(KeyboardActionRow.designEntries(layout, true, false, false, false, TouchKeyStyle.Row.STANDARD, true, platform)
                .equals(KeyboardActionRow.designEntries(layout, true)), layout + " is unchanged");
        // 交换中/英与 123 叠在互换之后：符 仍在最左。
        check(slots(KeyboardActionRow.designEntries(standard, false, true, false, false, TouchKeyStyle.Row.STANDARD, true, platform))
            .equals(List.of(SYMBOL, LANGUAGE, SPACE, COMMA, LAYER, RETURN)), "the language/number swap still applies");
        // 借来的数字九宫格：返回键留在字母层 123 那一格。
        check(!KeyboardActionRow.numberPadLayerFirst(standard, TouchKeyStyle.Row.STANDARD, true)
            && KeyboardActionRow.numberPadLayerFirst(standard, TouchKeyStyle.Row.STANDARD, false)
            && KeyboardActionRow.numberPadLayerFirst(standard, TouchKeyStyle.Row.EMOJI, true)
            && KeyboardActionRow.numberPadLayerFirst(KeyboardLayout.FOURTEEN_KEY_LAYOUT, TouchKeyStyle.Row.STANDARD, true),
            "the borrowed pad follows the swapped source row");
        for (boolean globe : new boolean[] {false, true}) {
            List<KeyboardActionRow.DesignEntry> pad = KeyboardActionRow.numberPadEntries(standard, globe, false,
                TouchKeyStyle.Row.STANDARD, true, platform);
            List<KeyboardActionRow.DesignEntry> letters = KeyboardActionRow.designEntries(standard, globe, false, false, false,
                TouchKeyStyle.Row.STANDARD, true, platform);
            check(pad.get(0).slot() == SYMBOL && pad.get(1).slot() == LAYER
                && Math.abs(pad.get(0).weight() - letters.get(0).weight()) < 1e-5f
                && Math.abs(pad.get(1).weight() - letters.get(1).weight()) < 1e-5f,
                "符 and 返回 sit where 符 and 123 were, globe " + globe);
            check(pad.equals(KeyboardActionRow.numberPadEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, globe, false,
                TouchKeyStyle.Row.STANDARD)), "the pad uses the nine-key order, globe " + globe);
        }
        check(slots(KeyboardActionRow.numberPadEntries(standard, false, false, TouchKeyStyle.Row.EMOJI, true, platform)).get(0) == LAYER,
            "an emoji-style source keeps 返回 first");
        // 123 / #+= 层（英文、韩文等 26 键的数字入口，中文 26 键的 #+=）：字母层确实互换时 😀/符号 排在返回键前面，返回键在字母层 123 那一格、与 123 同宽；长空格、表情键样式和字母层没有 123 | 符 的键面不换。
        check(KeyboardActionRow.symbolFirstMoves(standard, TouchKeyStyle.Row.STANDARD)
            && KeyboardActionRow.symbolFirstMoves(standard, TouchKeyStyle.Row.COMMA_PERIOD)
            && KeyboardActionRow.symbolFirstMoves(KeyboardLayout.KOREAN_LAYOUT, TouchKeyStyle.Row.STANDARD)
            && !KeyboardActionRow.symbolFirstMoves(standard, TouchKeyStyle.Row.LONG_SPACE)
            && !KeyboardActionRow.symbolFirstMoves(standard, TouchKeyStyle.Row.EMOJI)
            && !KeyboardActionRow.symbolFirstMoves(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, TouchKeyStyle.Row.STANDARD)
            && !KeyboardActionRow.symbolFirstMoves(KeyboardLayout.STROKE_LAYOUT, TouchKeyStyle.Row.STANDARD)
            && !KeyboardActionRow.symbolFirstMoves(KeyboardLayout.FOURTEEN_KEY_LAYOUT, TouchKeyStyle.Row.STANDARD)
            && !KeyboardActionRow.symbolFirstMoves(KeyboardLayout.HANDWRITING_LAYOUT, TouchKeyStyle.Row.STANDARD),
            "the 123 layer follows only a swapped letter row");
        List<KeyboardActionRow.DesignEntry> english = KeyboardActionRow.designEntries(standard, false, false, false, true,
            TouchKeyStyle.Row.STANDARD, true, platform);
        for (List<KeyboardLayout.LayerKey> bottom : List.of(
                KeyboardLayout.numberLayer(false, false, false, false, false, true).get(3),
                KeyboardLayout.numberLayer(true, true, false, true, false, true).get(3),
                KeyboardLayout.moreSymbolLayer(false, false, false, true).get(3))) {
            check(bottom.get(0).kind() != KeyboardLayout.LayerKeyKind.LETTERS
                && bottom.get(1).kind() == KeyboardLayout.LayerKeyKind.LETTERS
                && Math.abs(bottom.get(0).weight() - english.get(0).weight()) < 1e-5f
                && Math.abs(bottom.get(1).weight() - english.get(1).weight()) < 1e-5f,
                "返回 sits where 123 was, behind " + bottom.get(0).text());
        }
        check(KeyboardLayout.numberLayer(false, false, false, false, false, false)
                .equals(KeyboardLayout.numberLayer(false, false, false, false, false))
            && KeyboardLayout.moreSymbolLayer(false, false, false, false).equals(KeyboardLayout.moreSymbolLayer(false, false, false))
            && KeyboardLayout.numberLayer(false, false, false, false, false, false).get(3).get(0).kind()
                == KeyboardLayout.LayerKeyKind.LETTERS, "without the swap the layer keeps 返回 first");
    }

    /**
     * 数字键盘空格位置（#10056，`touch_number_pad_space`）：缺省（`platform_default`）是 Android 原来的右列，0 接替整个空格份额；在 0 右边或左边时两键都在底行，左边那个占 4 − 1.05、右边那个 1.05（与中/英同宽），整行总份额不变。借来的数字九宫格、交换中/英与 123 都照样作用；右侧大回车不受影响。
     */
    private static void numberPadSpace() {
        KeyboardActionRow.DesignSlot zero = KeyboardActionRow.DesignSlot.ZERO;
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        check(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE.equals("touch_number_pad_space"), "number pad key matches client-core");
        check(TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT.wire.equals("platform_default")
            && TouchKeyStyle.NumberPadSpace.RIGHT_COLUMN.wire.equals("right_column")
            && TouchKeyStyle.NumberPadSpace.AFTER_ZERO.wire.equals("after_zero")
            && TouchKeyStyle.NumberPadSpace.BEFORE_ZERO.wire.equals("before_zero"), "number pad wire strings match client-core");
        check(TouchKeyStyle.NumberPadSpace.parse(null) == TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT
            && TouchKeyStyle.NumberPadSpace.parse("space_in_grid") == TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT
            && TouchKeyStyle.NumberPadSpace.parse("before_zero") == TouchKeyStyle.NumberPadSpace.BEFORE_ZERO,
            "missing and unknown values read as the platform default");
        check(TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT.resolved() == TouchKeyStyle.NumberPadSpace.RIGHT_COLUMN
            && TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT.inRightColumn()
            && !TouchKeyStyle.NumberPadSpace.AFTER_ZERO.inRightColumn(), "Android's default keeps the space in the right column");
        check(TouchKeyStyle.NumberPadSpace.CHOICES.stream().map(choice -> choice.title).toList()
            .equals(List.of("空格在右列", "空格在 0 右边", "空格在 0 左边")), "choice titles match iOS and HarmonyOS");
        check(TouchKeyStyle.NUMBER_PAD_SPACE_GROUP.equals("数字键盘")
            && TouchKeyStyle.NUMBER_PAD_SPACE_NOTE.equals("九宫格数字键盘里空格的位置；右侧大回车不受影响。")
            && TouchKeyStyle.SYMBOL_FIRST_TITLE.equals("26 键「符」与「123」互换")
            && TouchKeyStyle.SYMBOL_FIRST_NOTE.equals("符 排在 123 左边；长空格和表情键样式没有 符，不受影响。"),
            "settings texts match iOS and HarmonyOS word for word");

        List<KeyboardActionRow.DesignEntry> today = KeyboardActionRow.designEntries(nine, false, false, true);
        for (TouchKeyStyle.NumberPadSpace space : List.of(TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, TouchKeyStyle.NumberPadSpace.RIGHT_COLUMN))
            check(KeyboardActionRow.designEntries(nine, false, false, true, false, TouchKeyStyle.Row.STANDARD, false, space).equals(today)
                && today.equals(entries(SYMBOL, 1.05f, LAYER, 1.25f, zero, 4f, LANGUAGE, 1.05f, RETURN, 1.9f)),
                space + " is today's number layer");
        List<KeyboardActionRow.DesignEntry> after = KeyboardActionRow.designEntries(nine, false, false, true, false,
            TouchKeyStyle.Row.STANDARD, false, TouchKeyStyle.NumberPadSpace.AFTER_ZERO);
        check(after.equals(entries(SYMBOL, 1.05f, LAYER, 1.25f, zero, 2.95f, SPACE, 1.05f, LANGUAGE, 1.05f, RETURN, 1.9f)),
            "after_zero: 0 then a 1.05 space " + after);
        List<KeyboardActionRow.DesignEntry> before = KeyboardActionRow.designEntries(nine, true, false, true, false,
            TouchKeyStyle.Row.STANDARD, false, TouchKeyStyle.NumberPadSpace.BEFORE_ZERO);
        check(before.equals(entries(SYMBOL, 1.05f, LAYER, 1.25f, GLOBE, 1f, SPACE, 2.95f, zero, 1.05f, LANGUAGE, 1.05f, RETURN, 1.9f)),
            "before_zero: a wide space then a 1.05 zero " + before);
        for (TouchKeyStyle.NumberPadSpace space : TouchKeyStyle.NumberPadSpace.values()) {
            List<KeyboardActionRow.DesignEntry> pad = KeyboardActionRow.designEntries(nine, false, false, true, false,
                TouchKeyStyle.Row.STANDARD, false, space);
            check(Math.abs(total(pad) - total(KeyboardActionRow.designEntries(nine, false))) < 1e-5f, space + " keeps the row width");
            check(slots(pad).stream().filter(slot -> slot == zero).count() == 1, space + " has one 0");
            check(slots(pad).contains(SPACE) == !space.inRightColumn(), space + " draws the space in the row unless it is in the column");
            // 借来的数字九宫格（26 键）同样按它排，返回键仍在第一格。
            List<KeyboardActionRow.DesignEntry> borrowed = KeyboardActionRow.numberPadEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT,
                false, false, TouchKeyStyle.Row.STANDARD, false, space);
            check(borrowed.get(0).slot() == LAYER && borrowed.containsAll(pad), space + " lays the borrowed pad the same way");
            // 交换中/英与 123 照样作用。
            List<KeyboardActionRow.DesignEntry> swapped = KeyboardActionRow.numberPadEntries(nine, false, true,
                TouchKeyStyle.Row.STANDARD, false, space);
            check(swapped.get(1).slot() == LANGUAGE && slots(swapped).indexOf(LAYER) == slots(pad).indexOf(LANGUAGE),
                space + " still swaps 中/英 and 123");
            // 右侧大回车没有单独的底行，不受影响。
            check(KeyboardActionRow.numberPadEntries(nine, false, false, TouchKeyStyle.Row.TALL_RETURN, false, space).isEmpty(),
                space + " leaves tall_return alone");
            // 字母层不受它影响。
            check(KeyboardActionRow.designEntries(nine, false, false, false, false, TouchKeyStyle.Row.STANDARD, false, space)
                .equals(KeyboardActionRow.designEntries(nine, false)), space + " leaves the letter layer alone");
        }
    }

    /**
     * 「中/英键放在空格左边」（#10228，`touch_language_key_side`）：排好的底行里中/英左边紧挨着的一串空格、0、逗号、句号整串右移一格，中/英挪到这一串最左边，宽度跟着键走、整行总份额不变。空格左边没有标点时中/英紧挨空格，有标点时标点仍贴着空格。逐行钉住规格第 2.4 节：中文 26 键四种样式与互换、英文 26 键、14/18 键三种样式、九键外框三种样式、九键数字层三种空格位置、借来的数字九宫格、右侧大回车两层、手写；地球键照旧紧跟 123；本机「交换语言键与数字键」最后一步按键对调；中/英不再紧挨回车，回车不让；默认右边逐键不变；只有字符串 `left` 读成左。
     */
    private static void languageLeft() {
        KeyboardActionRow.DesignSlot zero = KeyboardActionRow.DesignSlot.ZERO;
        KeyboardActionRow.DesignSlot rewrite = KeyboardActionRow.DesignSlot.REWRITE;
        KeyboardActionRow.DesignSlot delete = KeyboardActionRow.DesignSlot.DELETE;
        TouchKeyStyle.NumberPadSpace platform = TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT;
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        int fourteen = KeyboardLayout.FOURTEEN_KEY_LAYOUT;
        int nine = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        // 读法与写法。
        check(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE.equals("touch_language_key_side"), "language side key matches client-core");
        check(TouchKeyStyle.languageLeft("left") && !TouchKeyStyle.languageLeft(null) && !TouchKeyStyle.languageLeft("right")
            && !TouchKeyStyle.languageLeft("LEFT") && !TouchKeyStyle.languageLeft("middle") && !TouchKeyStyle.languageLeft(Boolean.TRUE)
            && !TouchKeyStyle.languageLeft(1), "only the string left reads as left");
        check(TouchKeyStyle.languageKeySideWire(true).equals("left") && TouchKeyStyle.languageKeySideWire(false).equals("right"),
            "the toggle writes left and right");
        check(TouchKeyStyle.LANGUAGE_LEFT_TITLE.equals("中/英键放在空格左边")
            && TouchKeyStyle.LANGUAGE_LEFT_NOTE.equals("中/英 排到空格左边，逗号、句号仍紧挨空格。"), "settings texts match iOS and HarmonyOS word for word");

        // 中文 26 键。
        expectLeft(standard, false, TouchKeyStyle.Row.STANDARD, false,
            entries(LAYER, 1.25f, SYMBOL, 1.05f, LANGUAGE, 1.05f, SPACE, 4f, COMMA, 1f, RETURN, 1.9f));
        expectLeft(standard, false, TouchKeyStyle.Row.COMMA_PERIOD, false,
            entries(LAYER, 1.25f, SYMBOL, 1.05f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 3f, PERIOD, 1f, RETURN, 1.9f));
        expectLeft(standard, false, TouchKeyStyle.Row.LONG_SPACE, false,
            entries(LAYER, 1.25f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 5.05f, RETURN, 1.9f));
        expectLeft(standard, false, TouchKeyStyle.Row.EMOJI, false,
            entries(LAYER, 1.25f, EMOJI, 1.05f, LANGUAGE, 1.05f, SPACE, 4f, COMMA, 1f, RETURN, 1.9f));
        expectLeft(standard, false, TouchKeyStyle.Row.STANDARD, true,
            entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, SPACE, 4f, COMMA, 1f, RETURN, 1.9f));
        expectLeft(standard, false, TouchKeyStyle.Row.COMMA_PERIOD, true,
            entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 3f, PERIOD, 1f, RETURN, 1.9f));
        // 英文 26 键：中/英与中文 26 键停在同一格（第三格），切中/英时不跳。
        List<KeyboardActionRow.DesignEntry> english = KeyboardActionRow.designEntries(standard, false, false, false, true,
            TouchKeyStyle.Row.STANDARD, false, platform, true);
        check(english.equals(entries(LAYER, 1.25f, SYMBOL, 1.05f, LANGUAGE, 1.05f, PERIOD, 1f, SPACE, 3f, COMMA, 1f, RETURN, 1.9f)),
            "English 26 key keeps . against the space " + english);
        check(slots(english).indexOf(LANGUAGE) == slots(row(standard, false, TouchKeyStyle.Row.STANDARD, false, true)).indexOf(LANGUAGE),
            "中/英 sits in the same slot on the Chinese and English 26 key");
        check(slots(KeyboardActionRow.designEntries(KeyboardLayout.KOREAN_LAYOUT, false, false, false, false,
            TouchKeyStyle.Row.STANDARD, false, platform, true)).equals(List.of(LAYER, SYMBOL, LANGUAGE, SPACE, COMMA, RETURN)), "Korean too");
        // 14 / 18 键和大千注音。
        for (int layout : new int[] {fourteen, KeyboardLayout.EIGHTEEN_KEY_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT})
            check(row(layout, false, TouchKeyStyle.Row.STANDARD, false, true)
                .equals(entries(LAYER, 1.25f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 4f, PERIOD, 1f, RETURN, 1.9f)), layout + " standard");
        expectLeft(fourteen, false, TouchKeyStyle.Row.LONG_SPACE, false,
            entries(LAYER, 1.25f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 5f, RETURN, 1.9f));
        expectLeft(fourteen, false, TouchKeyStyle.Row.EMOJI, false,
            entries(LAYER, 1.25f, EMOJI, 1f, LANGUAGE, 1.05f, SPACE, 4f, COMMA, 1f, RETURN, 1.9f));
        // 九键外框（拼音九键、九键双拼、笔画）。
        for (int layout : new int[] {nine, KeyboardLayout.STROKE_LAYOUT}) {
            expectLeft(layout, false, TouchKeyStyle.Row.STANDARD, false,
                entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, SPACE, 4f, RETURN, 1.9f));
            expectLeft(layout, false, TouchKeyStyle.Row.COMMA_PERIOD, false,
                entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 2f, PERIOD, 1f, RETURN, 1.9f));
            expectLeft(layout, false, TouchKeyStyle.Row.EMOJI, false,
                entries(SYMBOL, 1.05f, LAYER, 1.25f, EMOJI, 1.05f, LANGUAGE, 1.05f, SPACE, 2.95f, RETURN, 1.9f));
        }
        // 地球键照旧紧跟 123（互换后紧跟 123）。
        check(slots(row(standard, true, TouchKeyStyle.Row.STANDARD, false, true))
            .equals(List.of(LAYER, GLOBE, SYMBOL, LANGUAGE, SPACE, COMMA, RETURN)), "the globe stays right after 123");
        check(slots(row(standard, true, TouchKeyStyle.Row.STANDARD, true, true))
            .equals(List.of(SYMBOL, LAYER, GLOBE, LANGUAGE, SPACE, COMMA, RETURN)), "the globe stays right after the swapped 123");
        check(slots(row(nine, true, TouchKeyStyle.Row.STANDARD, false, true))
            .equals(List.of(SYMBOL, LAYER, GLOBE, LANGUAGE, SPACE, RETURN)), "nine-key globe");

        // 九键数字层三种空格位置。
        expectNumbers(nine, platform, entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, zero, 4f, RETURN, 1.9f));
        expectNumbers(nine, TouchKeyStyle.NumberPadSpace.AFTER_ZERO,
            entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, zero, 2.95f, SPACE, 1.05f, RETURN, 1.9f));
        expectNumbers(nine, TouchKeyStyle.NumberPadSpace.BEFORE_ZERO,
            entries(SYMBOL, 1.05f, LAYER, 1.25f, LANGUAGE, 1.05f, SPACE, 2.95f, zero, 1.05f, RETURN, 1.9f));
        // 借来的数字九宫格：返回 | 符 之后是中/英，与 26 键字母层的中/英同一格；14/18 键差一格（数字九宫格多一个 符）。
        for (int layout : new int[] {standard, KeyboardLayout.HANDWRITING_LAYOUT, fourteen})
            expectNumbers(layout, TouchKeyStyle.NumberPadSpace.AFTER_ZERO,
                entries(LAYER, 1.25f, SYMBOL, 1.05f, LANGUAGE, 1.05f, zero, 2.95f, SPACE, 1.05f, RETURN, 1.9f));
        check(slots(KeyboardActionRow.numberPadEntries(standard, false, false, TouchKeyStyle.Row.STANDARD, false, platform, true))
            .indexOf(LANGUAGE) == slots(row(standard, false, TouchKeyStyle.Row.STANDARD, false, true)).indexOf(LANGUAGE),
            "the borrowed pad keeps 中/英 in the letter row's slot");
        check(slots(KeyboardActionRow.numberPadEntries(standard, false, false, TouchKeyStyle.Row.STANDARD, true, platform, true))
            .equals(List.of(SYMBOL, LAYER, LANGUAGE, zero, RETURN)), "the swapped 26 key lends 符 | 返回 | 中 | 0");
        check(slots(KeyboardActionRow.numberPadEntries(fourteen, false, false, TouchKeyStyle.Row.STANDARD, false, platform, true))
            .indexOf(LANGUAGE) == 2 && slots(row(fourteen, false, TouchKeyStyle.Row.STANDARD, false, true)).indexOf(LANGUAGE) == 1,
            "14/18 keys: 中/英 is one slot further right on the borrowed pad, accepted by the spec");
        for (TouchKeyStyle.NumberPadSpace space : TouchKeyStyle.NumberPadSpace.values())
            check(KeyboardActionRow.numberPadEntries(nine, false, false, TouchKeyStyle.Row.TALL_RETURN, false, space, true).isEmpty(),
                space + ": tall_return still owns its bottom row");

        // 手写：中/英挪到 ， 左边，重写、⌫、回车不动。
        check(row(KeyboardLayout.HANDWRITING_LAYOUT, false, TouchKeyStyle.Row.STANDARD, false, true).equals(entries(
                LAYER, 1.25f, LANGUAGE, 1.05f, COMMA, 1f, SPACE, 2.4f, PERIOD, 1f, rewrite, 1.25f, delete, 1.25f, RETURN, 1.6f)),
            "handwriting puts 中/英 before ，");

        // 右侧大回车：123 |（地球）| 中/英 | 空格，数字层 切换 |（地球）| 中/英 | 0 | 空格；宽度与右边时相同。
        float nineShare = KeyboardActionRow.tallReturnGridShare(nine);
        for (boolean globe : new boolean[] {false, true}) {
            for (boolean digits : new boolean[] {false, true}) {
                List<KeyboardActionRow.DesignEntry> right = KeyboardActionRow.tallReturnEntries(nineShare, globe, false, digits);
                List<KeyboardActionRow.DesignEntry> left = KeyboardActionRow.tallReturnEntries(nineShare, globe, false, digits, true);
                check(right.equals(KeyboardActionRow.tallReturnEntries(nineShare, globe, false, digits, false)), "tall right is today's strip");
                List<KeyboardActionRow.DesignSlot> expected = new ArrayList<>(List.of(LAYER));
                if (globe) expected.add(GLOBE);
                expected.add(LANGUAGE);
                if (digits) expected.add(zero);
                expected.add(SPACE);
                check(slots(left).equals(expected), "tall strip globe " + globe + " digits " + digits + ": " + left);
                check(left.containsAll(right) && Math.abs(total(left) - total(right)) < 1e-5f, "tall strip keeps every width");
                // 交换中/英与 123 按键对调：中/英挪到左边后不在最后一格，不能再对调首尾。
                List<KeyboardActionRow.DesignEntry> swapped = KeyboardActionRow.tallReturnEntries(nineShare, globe, true, digits, true);
                check(swapped.get(0).equals(new KeyboardActionRow.DesignEntry(LANGUAGE, 1.05f))
                    && swapped.get(slots(left).indexOf(LANGUAGE)).equals(new KeyboardActionRow.DesignEntry(LAYER, 1.25f))
                    && swapped.get(swapped.size() - 1).slot() == SPACE, "tall strip swaps 中/英 and 123 by key: " + swapped);
            }
        }

        // 本机「交换语言键与数字键」最后一步，按键对调：叠出百度「经典」中英 | 符 | 123 | 空格。
        check(KeyboardActionRow.designEntries(standard, false, true, false, false, TouchKeyStyle.Row.STANDARD, false, platform, true)
            .equals(entries(LANGUAGE, 1.05f, SYMBOL, 1.05f, LAYER, 1.25f, SPACE, 4f, COMMA, 1f, RETURN, 1.9f)), "26 key left + swap");
        check(slots(KeyboardActionRow.designEntries(nine, false, true, false, false, TouchKeyStyle.Row.STANDARD, false, platform, true))
            .equals(List.of(SYMBOL, LANGUAGE, LAYER, SPACE, RETURN)), "nine key left + swap");
        check(slots(KeyboardActionRow.numberPadEntries(standard, false, true, TouchKeyStyle.Row.STANDARD, false, platform, true))
            .equals(List.of(LANGUAGE, SYMBOL, LAYER, zero, RETURN)), "borrowed pad left + swap");

        // 每一行：只换次序、宽度和总份额不变，回车不再让给中/英；关上时逐键与原来相同；规则重复套用不再变。
        for (int layout : new int[] {standard, KeyboardLayout.KOREAN_LAYOUT, fourteen, KeyboardLayout.EIGHTEEN_KEY_LAYOUT,
                KeyboardLayout.ZHUYIN_LAYOUT, nine, KeyboardLayout.STROKE_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT}) {
            for (TouchKeyStyle.Row style : TouchKeyStyle.Row.values()) {
                for (boolean globe : new boolean[] {false, true}) {
                    for (boolean symbolFirst : new boolean[] {false, true}) {
                        for (boolean digits : new boolean[] {false, true}) {
                            List<KeyboardActionRow.DesignEntry> right = KeyboardActionRow.designEntries(layout, globe, false, digits, false,
                                style, symbolFirst, platform);
                            List<KeyboardActionRow.DesignEntry> left = KeyboardActionRow.designEntries(layout, globe, false, digits, false,
                                style, symbolFirst, platform, true);
                            String label = layout + " " + style + " globe " + globe + " symbolFirst " + symbolFirst + " digits " + digits;
                            check(KeyboardActionRow.designEntries(layout, globe, false, digits, false, style, symbolFirst, platform, false)
                                .equals(right), label + ": right is today's row");
                            check(left.size() == right.size() && left.containsAll(right) && Math.abs(total(left) - total(right)) < 1e-5f,
                                label + ": only reorders");
                            if (right.isEmpty()) continue;
                            check(KeyboardActionRow.languageBesideReturn(right) == (layout != KeyboardLayout.HANDWRITING_LAYOUT),
                                label + ": on the right, the return yields unless handwriting's keys sit between");
                            check(!KeyboardActionRow.languageBesideReturn(left), label + ": on the left, the return no longer yields");
                            check(KeyboardActionRow.languageBeforeSpace(left).equals(left), label + ": applying the rule twice changes nothing");
                            int language = slots(left).indexOf(LANGUAGE);
                            check(language > 0 && !List.of(SPACE, zero, COMMA, PERIOD).contains(left.get(language - 1).slot())
                                && List.of(SPACE, zero, COMMA, PERIOD).contains(left.get(language + 1).slot()),
                                label + ": 中/英 sits after the function keys and before the space run");
                        }
                    }
                }
            }
        }
        // 行里没有中/英、或中/英左边紧挨的不是空格一类时原样返回。
        List<KeyboardActionRow.DesignEntry> noLanguage = entries(LAYER, 1.25f, SPACE, 4f, RETURN, 1.9f);
        check(KeyboardActionRow.languageBeforeSpace(noLanguage) == noLanguage, "a row without 中/英 is unchanged");
        List<KeyboardActionRow.DesignEntry> leading = entries(LANGUAGE, 1.05f, SPACE, 4f, RETURN, 1.9f);
        check(KeyboardActionRow.languageBeforeSpace(leading) == leading, "中/英 already left of the space is unchanged");
        check(KeyboardActionRow.designEntries(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, true, false, false, false,
                TouchKeyStyle.Row.STANDARD, false, platform, true).isEmpty()
            && KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true, false, false, false,
                TouchKeyStyle.Row.STANDARD, false, platform, true).isEmpty(), "kana and zhuyin nine-key grids are out of scope");
    }

    /** 中/英在左边时这一行逐个比槽位和宽度，再核对整行总份额等于同一样式在右边时的行。 */
    private static void expectLeft(int layout, boolean globe, TouchKeyStyle.Row style, boolean symbolFirst,
            List<KeyboardActionRow.DesignEntry> expected) {
        List<KeyboardActionRow.DesignEntry> actual = row(layout, globe, style, symbolFirst, true);
        check(sameWeights(actual, expected), layout + " " + style + " symbolFirst " + symbolFirst + ": " + actual);
        check(Math.abs(total(actual) - total(row(layout, globe, style, symbolFirst, false))) < 1e-4f,
            layout + " " + style + " keeps the row width");
    }

    /** 中/英在左边时从 `sourceLayout` 进的数字九宫格逐个比槽位和宽度。 */
    private static void expectNumbers(int sourceLayout, TouchKeyStyle.NumberPadSpace space, List<KeyboardActionRow.DesignEntry> expected) {
        List<KeyboardActionRow.DesignEntry> actual = KeyboardActionRow.numberPadEntries(sourceLayout, false, false,
            TouchKeyStyle.Row.STANDARD, false, space, true);
        check(sameWeights(actual, expected), sourceLayout + " " + space + " number pad: " + actual);
    }

    private static boolean sameWeights(List<KeyboardActionRow.DesignEntry> actual, List<KeyboardActionRow.DesignEntry> expected) {
        if (actual.size() != expected.size()) return false;
        for (int index = 0; index < expected.size(); index++) {
            if (actual.get(index).slot() != expected.get(index).slot()
                    || Math.abs(actual.get(index).weight() - expected.get(index).weight()) >= 1e-5f) return false;
        }
        return true;
    }

    private static List<KeyboardActionRow.DesignEntry> row(int layout, boolean globe, TouchKeyStyle.Row style,
            boolean symbolFirst, boolean languageLeft) {
        return KeyboardActionRow.designEntries(layout, globe, false, false, false, style, symbolFirst,
            TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, languageLeft);
    }

    private static List<KeyboardActionRow.DesignEntry> row(int layout, boolean globe, TouchKeyStyle.Row style) {
        return KeyboardActionRow.designEntries(layout, globe, false, false, false, style);
    }

    /** 逐个比槽位和宽度，再核对地球键插在 123 之后、交换中/英与 123 后两键互换，以及整行总份额等于标准底行。 */
    private static void expect(int layout, TouchKeyStyle.Row style, List<KeyboardActionRow.DesignEntry> expected) {
        List<KeyboardActionRow.DesignEntry> actual = row(layout, false, style);
        check(actual.size() == expected.size(), layout + " " + style + ": " + actual);
        for (int index = 0; index < expected.size(); index++) {
            check(actual.get(index).slot() == expected.get(index).slot()
                    && Math.abs(actual.get(index).weight() - expected.get(index).weight()) < 1e-5f,
                layout + " " + style + " key " + index + ": " + actual);
        }
        check(Math.abs(total(actual) - total(KeyboardActionRow.designEntries(layout, false))) < 1e-4f,
            layout + " " + style + " keeps the row width");
        List<KeyboardActionRow.DesignEntry> globe = row(layout, true, style);
        int layer = slots(globe).indexOf(LAYER);
        check(globe.size() == actual.size() + 1 && globe.get(layer + 1).slot() == GLOBE, layout + " " + style + " globe after 123");
        List<KeyboardActionRow.DesignEntry> swapped = KeyboardActionRow.designEntries(layout, false, true, false, false, style);
        int language = slots(actual).indexOf(LANGUAGE);
        check(swapped.get(slots(actual).indexOf(LAYER)).slot() == LANGUAGE && swapped.get(language).slot() == LAYER,
            layout + " " + style + " swaps 中/英 and 123");
    }

    private static List<KeyboardActionRow.DesignEntry> entries(Object... pairs) {
        List<KeyboardActionRow.DesignEntry> list = new ArrayList<>(pairs.length / 2);
        for (int index = 0; index < pairs.length; index += 2)
            list.add(new KeyboardActionRow.DesignEntry((KeyboardActionRow.DesignSlot) pairs[index], (Float) pairs[index + 1]));
        return list;
    }

    private static List<KeyboardActionRow.DesignSlot> slots(List<KeyboardActionRow.DesignEntry> entries) {
        return entries.stream().map(KeyboardActionRow.DesignEntry::slot).toList();
    }

    private static float total(List<KeyboardActionRow.DesignEntry> entries) {
        float sum = 0;
        for (KeyboardActionRow.DesignEntry entry : entries) sum += entry.weight();
        return sum;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
