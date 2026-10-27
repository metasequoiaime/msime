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
        numberRow();
        groups();
        migration();
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
    }

    private static void migration() {
        Map<String, Object> both = KeyLayoutStyleMigration.sharedEdits(true, true, null, false);
        check(both.equals(Map.of(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE, "long_space", TouchKeyStyle.NUMBER_ROW_PREFERENCE, true)),
            "both legacy switches move into the shared preferences");
        check(KeyLayoutStyleMigration.sharedEdits(true, false, "standard", false)
            .equals(Map.of(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE, "long_space")), "an explicit standard style is overwritten");
        check(KeyLayoutStyleMigration.sharedEdits(true, false, "emoji", false).isEmpty(),
            "a style already chosen on another device is kept");
        check(KeyLayoutStyleMigration.sharedEdits(true, false, "split_space", false)
            .equals(Map.of(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE, "long_space")),
            "an unknown style reads standard, the same way the keyboard draws it");
        check(KeyLayoutStyleMigration.sharedEdits(false, true, null, true).isEmpty(), "a number row already on stays on");
        check(KeyLayoutStyleMigration.sharedEdits(false, false, null, false).isEmpty(), "legacy off writes nothing");
        Map<String, Object> legacy = new LinkedHashMap<>();
        check(!KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)), "nothing to migrate");
        legacy.put(AndroidLocalSettings.NUMBER_ROW, false);
        check(KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)),
            "an explicit false still has to be deleted");
        legacy.clear();
        legacy.put(AndroidLocalSettings.LONG_SPACE_LAYOUT, true);
        check(KeyLayoutStyleMigration.pending(new AndroidLocalSettings.Snapshot(legacy)), "long space pending");
        check(!AndroidLocalSettings.spec(AndroidLocalSettings.LONG_SPACE_LAYOUT).synced
            && !AndroidLocalSettings.spec(AndroidLocalSettings.NUMBER_ROW).synced,
            "the legacy keys never travel through android_local");
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
