package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 大屏分离式键盘的纯逻辑：什么时候分离（设置 × 设备形态 × 方向）、哪些布局分离、每一行的断点和空隙宽度、底行空格的拆法、空隙里的按下不归任何键，以及与单手模式的互斥。
 */
public final class SplitKeyboardPolicySmoke {
    private static final float EPSILON = 1e-4f;

    public static void main(String[] args) {
        whenSplitApplies();
        portraitOverride();
        configurableBounds();
        whichLayoutsSplit();
        gapWidth();
        responsiveGapWidth();
        letterRowCuts();
        duplicatedInnerKeys();
        designLayerCuts();
        spaceSplit();
        deadGap();
        oneHandedExclusion();
        surfaceWidth();
        phoneLandscape();
        System.out.println("SplitKeyboardPolicySmoke ok");
    }

    private static void whenSplitApplies() {
        // 设置 × 设备形态（smallestScreenWidthDp）× 方向，只有三者都满足才分离。
        int[] widths = {360, 411, 599, 600, 720, 840};
        for (boolean enabled : new boolean[] {false, true}) {
            for (int width : widths) {
                for (boolean landscape : new boolean[] {false, true}) {
                    boolean expected = enabled && landscape && width >= 600;
                    check(SplitKeyboardPolicy.active(enabled, width, landscape) == expected,
                        "active(" + enabled + ", " + width + ", " + landscape + ")");
                }
            }
        }
        check(!SplitKeyboardPolicy.active(true, 411, true), "a phone in landscape never splits");
        check(!SplitKeyboardPolicy.active(true, 800, false), "a tablet in portrait does not split");
        check(!SplitKeyboardPolicy.active(false, 800, true), "the setting is off by default and then nothing splits");
    }

    private static void configurableBounds() {
        check(SplitKeyboardPolicy.active(true, 480, true, false, 480), "minimum threshold is usable");
        check(!SplitKeyboardPolicy.active(true, 479, true, false, 480), "below threshold stays whole");
        check(SplitKeyboardPolicy.active(true, 1000, true, false, 1200), "threshold clamps to maximum");
        check(SplitKeyboardPolicy.gapFraction(0, 10) == 0.10f, "minimum configured gap");
        check(SplitKeyboardPolicy.gapFraction(0, 50) == 0.40f, "maximum configured gap");
        close(SplitKeyboardPolicy.gapWeight(10f, 0f, 0, 10), 10f / 9f, "configured ten percent gap");
        check(SplitKeyboardPolicy.gapFraction(1200, 10) >= 0.399f, "wide screen keeps key area usable");
    }

    private static void whichLayoutsSplit() {
        check(SplitKeyboardPolicy.splitsLayout(KeyboardLayout.STANDARD_TOUCH_LAYOUT), "the 26-key family splits");
        check(SplitKeyboardPolicy.splitsLayout(KeyboardLayout.KOREAN_LAYOUT), "Korean splits");
        int[] grids = {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT,
            KeyboardLayout.HANDWRITING_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
            KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT};
        for (int layout : grids) {
            check(!SplitKeyboardPolicy.splitsLayout(layout), "layout " + layout + " does not split");
            check(!SplitKeyboardPolicy.drawn(true, 800, true, layout), "layout " + layout + " is never drawn split");
        }
        check(SplitKeyboardPolicy.drawn(true, 800, true, KeyboardLayout.STANDARD_TOUCH_LAYOUT), "drawn when everything holds");
        check(!SplitKeyboardPolicy.drawn(true, 800, false, KeyboardLayout.KOREAN_LAYOUT), "not drawn in portrait");
        check(!SplitKeyboardPolicy.drawn(false, 800, true, KeyboardLayout.STANDARD_TOUCH_LAYOUT), "not drawn when off");
        check(!SplitKeyboardPolicy.drawn(true, 599, true, KeyboardLayout.STANDARD_TOUCH_LAYOUT), "not drawn on a phone");
    }

    private static void portraitOverride() {
        for (boolean enabled : new boolean[] {false, true}) {
            for (boolean portrait : new boolean[] {false, true}) {
                for (boolean landscape : new boolean[] {false, true}) {
                    for (int width : new int[] {360, 599, 600, 840}) {
                        boolean active = enabled && width >= 600 && (landscape || portrait);
                        check(SplitKeyboardPolicy.active(enabled, width, landscape, portrait) == active,
                            "竖屏覆盖仍须满足主开关和大屏条件");
                        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT,
                                KeyboardLayout.KOREAN_LAYOUT, KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT,
                                KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT}) {
                            check(SplitKeyboardPolicy.drawn(enabled, width, landscape, layout, portrait)
                                == (active && SplitKeyboardPolicy.splitsLayout(layout)),
                                "竖屏覆盖不改变支持的布局");
                        }
                    }
                }
            }
        }
        check(SplitKeyboardPolicy.drawn(true, 673, false, KeyboardLayout.STANDARD_TOUCH_LAYOUT, true),
            "展开内屏竖放时分离");
        check(!SplitKeyboardPolicy.drawn(true, 344, false, KeyboardLayout.STANDARD_TOUCH_LAYOUT, true),
            "折叠到窄外屏后恢复完整键盘");
    }

    private static void gapWidth() {
        for (float total : new float[] {7.8f, 9f, 9.8f, 10f, 10.2f, 11.2f, 12.2f}) {
            float gap = SplitKeyboardPolicy.gapWeight(total);
            close(gap / (total + gap), SplitKeyboardPolicy.GAP_FRACTION, "the gap is a quarter of the row at " + total);
        }
        check(SplitKeyboardPolicy.gapWeight(-1f) == 0f, "a negative row has no gap");
        // 第一行十个键：两半各占 37.5%，空隙 25%。
        float[] row = ones(10);
        int cut = SplitKeyboardPolicy.cutIndex(row);
        float gap = SplitKeyboardPolicy.gapWeight(10f);
        close(sum(row, 0, cut) / (10f + gap), .375f, "left half of the top row");
        close(sum(row, cut, row.length) / (10f + gap), .375f, "right half of the top row");
    }

    /** 按窗口变化验证可达范围、临界连续性与重复键借宽，防止大平板把五列键拉成长条。 */
    private static void responsiveGapWidth() {
        for (int width : new int[] {0, 600, 720, 959, 960, 961, 1200, 1600, 2400}) {
            float gap = SplitKeyboardPolicy.gapWeight(10f, 0f, width);
            float contentFraction = 10f / (10f + gap);
            float expected = width <= 0 ? .75f : Math.min(.75f, 720f / width);
            close(contentFraction, expected, "usable fraction at " + width);
            if (width > 0) check(width * contentFraction <= 720.001f, "bounded thumb reach at " + width);
            for (float duplicate : new float[] {0f, 1f}) {
                float remaining = SplitKeyboardPolicy.gapWeight(10f, duplicate, width);
                close(10f + remaining + duplicate, 10f + gap, "duplicate keeps key widths at " + width);
                check(remaining > 0, "gap remains after duplicate at " + width);
            }
        }
        close(SplitKeyboardPolicy.gapFraction(-1), .25f, "unknown width keeps the original gap");
        close(SplitKeyboardPolicy.gapWeight(-1f, 0f, 1600), 0f, "empty row keeps no gap");
    }

    private static void letterRowCuts() {
        check(SplitKeyboardPolicy.cutIndex(ones(10)) == 5, "qwert | yuiop");
        // 第二行：两侧各 0.5 的缩进、a–l 九个键、隐藏的微软双拼 ; 键记 0。
        float[] secondRow = new float[12];
        secondRow[0] = .5f;
        for (int index = 1; index <= 9; index++) secondRow[index] = 1f;
        secondRow[10] = 0f;
        secondRow[11] = .5f;
        check(SplitKeyboardPolicy.cutIndex(secondRow) == 6, "indent asdfg | hjkl indent: the odd key goes left");
        // 微软双拼：; 键出现、两侧缩进收起。断点不变，整行份额也还是 10，所以 ; 键随渲染出现或消失时不必重建这一行。
        float[] microsoft = new float[12];
        for (int index = 1; index <= 10; index++) microsoft[index] = 1f;
        check(SplitKeyboardPolicy.cutIndex(microsoft) == 6, "asdfg | hjkl;");
        close(sum(microsoft, 0, 12), sum(secondRow, 0, 12), "the second row keeps one total either way");
        // 韩文第二行没有缩进，; 键隐藏。
        float[] korean = new float[10];
        for (int index = 0; index < 9; index++) korean[index] = 1f;
        check(SplitKeyboardPolicy.cutIndex(korean) == 5, "Korean ㅁㄴㅇㄹㅎ | ㅗㅓㅏㅣ");
        // 第三行：⇧ zxcvbnm ⌫，⇧ 和 ⌫ 留在外侧，多出的 v 归左边。
        float edge = KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
        float[] third = {edge, 1, 1, 1, 1, 1, 1, 1, edge};
        int cut = SplitKeyboardPolicy.cutIndex(third);
        check(cut == 5, "shift zxcv | bnm delete, got " + cut);
        check(cut > 0 && cut < third.length, "shift and delete end up on opposite halves");
        check(SplitKeyboardPolicy.cutIndex(new float[0]) == 0, "an empty row cuts at zero");
    }

    /** #6022：字母层第二、三行在右半边内侧重复 G、V，重复的键从空隙里占一个键宽，两半等宽、键宽不变。 */
    private static void duplicatedInnerKeys() {
        float[] standardSecond = new float[12];
        standardSecond[0] = .5f;
        for (int index = 1; index <= 9; index++) standardSecond[index] = 1f;
        standardSecond[11] = .5f;
        float letterEdge = KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
        float[] standardThird = {letterEdge, 1, 1, 1, 1, 1, 1, 1, letterEdge};
        check(!SplitKeyboardPolicy.duplicatesInnerKey(0, 3, ones(10)), "qwert | yuiop is already even");
        check(SplitKeyboardPolicy.duplicatesInnerKey(1, 3, standardSecond), "the a-l row repeats g");
        check(SplitKeyboardPolicy.duplicatesInnerKey(2, 3, standardThird), "the z-m row repeats v");
        check(!SplitKeyboardPolicy.duplicatesInnerKey(1, 4, standardSecond)
                && !SplitKeyboardPolicy.duplicatesInnerKey(3, 3, standardThird),
            "other row sets repeat nothing");
        // 微软双拼显示 ; 时第二行两侧缩进隐藏（记 0）：0 + asdfg | hjkl; + 0 本来五对五，重复 G 会变成五对六、空隙偏开半个键，所以不重复。
        float[] microsoftSecond = new float[12];
        for (int index = 1; index <= 10; index++) microsoftSecond[index] = 1f;
        check(SplitKeyboardPolicy.cutIndex(microsoftSecond) == 6, "asdfg | hjkl; with the hidden indent on the left");
        check(!SplitKeyboardPolicy.duplicatesInnerKey(1, 3, microsoftSecond), "the Microsoft ten-key row repeats nothing");
        // 韩文两套式第二行九个键、不带缩进：ㅁㄴㅇㄹㅎ | ㅗㅓㅏㅣ，重复后五对五。
        check(SplitKeyboardPolicy.duplicatesInnerKey(1, 3, ones(9)), "a nine-key row without indents repeats its inner key");
        check(!SplitKeyboardPolicy.duplicatesInnerKey(1, 3, new float[0]), "an empty row repeats nothing");
        close(SplitKeyboardPolicy.gapWeight(10f, 0f), SplitKeyboardPolicy.gapWeight(10f), "no duplicate, same gap");
        close(SplitKeyboardPolicy.gapWeight(10f, 1f), SplitKeyboardPolicy.gapWeight(10f) - 1f, "the duplicate takes one key from the gap");
        check(SplitKeyboardPolicy.gapWeight(1f, 5f) == 0f, "the gap never goes negative");

        // 第二行：0.5 + asdfg | g + hjkl + 0.5，两半都是 5.5；整行总份额和不重复时一样，所以键和第一行一样宽。
        float[] secondRow = new float[12];
        secondRow[0] = .5f;
        for (int index = 1; index <= 9; index++) secondRow[index] = 1f;
        secondRow[11] = .5f;
        int cut = SplitKeyboardPolicy.cutIndex(secondRow);
        float total = sum(secondRow, 0, secondRow.length);
        float gap = SplitKeyboardPolicy.gapWeight(total, 1f);
        close(sum(secondRow, 0, cut), sum(secondRow, cut, secondRow.length) + 1f, "asdfg | ghjkl are even");
        close(total + gap + 1f, 10f + SplitKeyboardPolicy.gapWeight(10f), "the second row keeps the top row's total");
        check(secondRow[cut - 1] == 1f, "the repeated key is a letter, not the indent");

        // 第三行：⇧ zxcv | v bnm ⌫，两半都是 ⇧ + 4。
        float edge = KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
        float[] third = {edge, 1, 1, 1, 1, 1, 1, 1, edge};
        cut = SplitKeyboardPolicy.cutIndex(third);
        close(sum(third, 0, cut), sum(third, cut, third.length) + 1f, "shift zxcv | vbnm delete are even");
        check(cut - 1 > 0 && third[cut - 1] == 1f, "the repeated key is v, not shift");
    }

    private static void designLayerCuts() {
        for (boolean chinese : new boolean[] {true, false}) {
            for (List<List<KeyboardLayout.LayerKey>> layer : List.of(
                    KeyboardLayout.numberLayer(chinese), KeyboardLayout.moreSymbolLayer(chinese))) {
                check(SplitKeyboardPolicy.cutIndex(weights(layer.get(0))) == 5, "first design row splits 5 | 5");
                check(SplitKeyboardPolicy.cutIndex(weights(layer.get(1))) == 5, "second design row splits 5 | 5");
                List<KeyboardLayout.LayerKey> third = layer.get(2);
                int cut = SplitKeyboardPolicy.cutIndex(weights(third));
                check(cut == 4, "toggle + three marks | two marks + delete, got " + cut);
                check(third.get(0).kind() == KeyboardLayout.LayerKeyKind.LAYER_TOGGLE
                    && third.get(third.size() - 1).kind() == KeyboardLayout.LayerKeyKind.DELETE,
                    "the layer toggle and delete stay on the outer edges");
            }
        }
    }

    private static void spaceSplit() {
        // 中/英挪到空格左边（`touch_language_key_side` 为 `left`）后中缝仍在空格里、照样居中。
        for (boolean languageLeft : new boolean[] {false, true})
        for (boolean globe : new boolean[] {false, true}) {
            for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.KOREAN_LAYOUT}) {
                List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.designEntries(layout, globe, false, false, false,
                    TouchKeyStyle.Row.STANDARD, false, TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, languageLeft);
                float[] weights = new float[entries.size()];
                int space = -1;
                for (int index = 0; index < weights.length; index++) {
                    weights[index] = entries.get(index).weight();
                    if (entries.get(index).slot() == KeyboardActionRow.DesignSlot.SPACE) space = index;
                }
                checkCenteredSpace(weights, space, "action row globe=" + globe + " layout=" + layout + " languageLeft=" + languageLeft);
                // 常用标点键隐藏时（记 0）空隙仍然居中。
                float[] noComma = weights.clone();
                for (int index = 0; index < noComma.length; index++)
                    if (entries.get(index).slot() == KeyboardActionRow.DesignSlot.COMMA) noComma[index] = 0f;
                checkCenteredSpace(noComma, space, "action row without the comma");
            }
        }
        List<KeyboardLayout.LayerKey> bottom = KeyboardLayout.numberLayer(true).get(3);
        int space = -1;
        for (int index = 0; index < bottom.size(); index++)
            if (bottom.get(index).kind() == KeyboardLayout.LayerKeyKind.SPACE) space = index;
        checkCenteredSpace(weights(bottom), space, "design layer bottom row");
        // 空格偏在一侧、居中会让某一半太窄时，每一半至少保留原宽度的四分之一。
        float[] lopsided = {6f, 4f, 1f};
        float left = SplitKeyboardPolicy.leftSpaceWeight(lopsided, 1);
        close(left, 4f * SplitKeyboardPolicy.MIN_SPACE_SHARE, "the left space keeps its minimum share");
        boolean threw = false;
        try { SplitKeyboardPolicy.leftSpaceWeight(lopsided, 3); } catch (IllegalArgumentException expected) { threw = true; }
        check(threw, "a missing space key is a programming error");
    }

    private static void checkCenteredSpace(float[] weights, int space, String label) {
        check(space >= 0, label + ": has a space key");
        float total = sum(weights, 0, weights.length);
        float left = SplitKeyboardPolicy.leftSpaceWeight(weights, space);
        float right = weights[space] - left;
        check(left > 0 && right > 0, label + ": both halves keep a space key");
        close(sum(weights, 0, space) + left, total / 2f, label + ": the gap is centred");
    }

    /**
     * 按 LinearLayout 的方式排一行（键带左右外边距，空隙和缩进没有），再用 {@link KeyboardKeyArea} 判断空隙归属时的同一条规则（{@link KeyboardGapPolicy#gapDistance}）检查：落在中间空隙里的按下不在任何键的外边距内，所以不会被交给旁边的键；紧贴内侧键的外边距仍然交还给那个键。空隙本身是不可点击、不带外边距的普通 View，KeyboardKeyArea 的查找会跳过它。
     */
    private static void deadGap() {
        float edge = KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
        float[][] rows = {ones(10), {.5f, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, .5f}, {edge, 1, 1, 1, 1, 1, 1, 1, edge}};
        // 两种键距：默认的半个键距约 3 px，最大键距再放大一倍。
        for (int margin : new int[] {3, 12}) {
            for (float[] base : rows) {
                int cut = SplitKeyboardPolicy.cutIndex(base);
                float gap = SplitKeyboardPolicy.gapWeight(sum(base, 0, base.length));
                List<float[]> keys = layout(base, cut, gap, 1600, 8, margin);
                float[] gapSpan = keys.remove(keys.size() - 1);
                float gapWidth = gapSpan[1] - gapSpan[0];
                check(gapWidth > 1600 * .2f, "the gap is about a quarter of the row: " + gapWidth);
                int height = 100;
                for (float x = gapSpan[0]; x < gapSpan[1]; x += 1f) {
                    for (float[] key : keys) {
                        float distance = KeyboardGapPolicy.gapDistance(x - key[0], height / 2f,
                            (int) (key[1] - key[0]), height, margin, margin, margin, margin);
                        check(distance < 0, "a press at " + x + " in the gap is not routed to the key at " + key[0]);
                    }
                }
                // 内侧键右边外边距里的按下仍然交给它：键距的空隙不变成死区。
                float[] inner = keys.get(innerKey(base, cut));
                float distance = KeyboardGapPolicy.gapDistance(inner[1] - inner[0] + margin - 1, height / 2f,
                    (int) (inner[1] - inner[0]), height, margin, margin, margin, margin);
                check(distance >= 0, "the inner key still owns its own margin");
            }
        }
    }

    /** 左半边最内侧那个有宽度的键在键列表里的下标。 */
    private static int innerKey(float[] weights, int cut) {
        int keyIndex = -1;
        int last = -1;
        for (int index = 0; index < cut; index++) {
            if (weights[index] == 1f || weights[index] == KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT) {
                keyIndex++;
                last = keyIndex;
            }
        }
        return last;
    }

    /**
     * 模拟水平 LinearLayout：内边距 `padding`，宽度为 0 的子视图按份额分配扣掉内边距和外边距后的宽度。份额为 1 或边键份额的是键（带外边距），其余是缩进；最后追加空隙的 `[left, right)`。
     */
    private static List<float[]> layout(float[] weights, int cut, float gapWeight, int width, int padding, int margin) {
        float total = gapWeight;
        int keys = 0;
        for (float weight : weights) {
            total += weight;
            if (isKey(weight)) keys++;
        }
        float available = width - 2f * padding - keys * 2f * margin;
        List<float[]> spans = new ArrayList<>();
        float[] gap = null;
        float x = padding;
        for (int index = 0; index <= weights.length; index++) {
            if (index == cut) {
                float w = available * gapWeight / total;
                gap = new float[] {x, x + w};
                x += w;
            }
            if (index == weights.length) break;
            float weight = weights[index];
            if (!isKey(weight)) {
                x += available * weight / total;
                continue;
            }
            x += margin;
            float w = available * weight / total;
            spans.add(new float[] {x, x + w});
            x += w + margin;
        }
        spans.add(gap);
        return spans;
    }

    private static boolean isKey(float weight) {
        return weight == 1f || weight == KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
    }

    private static void oneHandedExclusion() {
        for (String stored : new String[] {"off", "left", "right"}) {
            check("off".equals(SplitKeyboardPolicy.effectiveOneHanded(stored, true)), stored + " is not applied while split");
            check(stored.equals(SplitKeyboardPolicy.effectiveOneHanded(stored, false)), stored + " comes back when not split");
        }
        check("off".equals(SplitKeyboardPolicy.effectiveOneHanded(null, false)), "a missing value is off");
    }

    private static void surfaceWidth() {
        check(KeyboardFormFactorPolicy.surfaceWidthDp(800, 1280, true) == 0, "a split tablet keyboard fills the window");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(800, 1280, false) == 720, "an unsplit tablet keyboard keeps the 720 dp cap");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(800, 1280) == 720, "the two-argument form is the unsplit one");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(411, 915, true) == 0, "a phone fills either way");
    }

    private static float[] weights(List<KeyboardLayout.LayerKey> row) {
        float[] weights = new float[row.size()];
        for (int index = 0; index < weights.length; index++) weights[index] = row.get(index).weight();
        return weights;
    }

    private static float[] ones(int count) {
        float[] weights = new float[count];
        java.util.Arrays.fill(weights, 1f);
        return weights;
    }

    private static float sum(float[] weights, int from, int to) {
        float total = 0f;
        for (int index = from; index < to; index++) total += weights[index];
        return total;
    }

    /** #10440：手机横屏分支只看本机开关、横屏、smallestWidth 低于启用宽度和分离后的字母键宽度，宽度取键盘实际可用宽度。 */
    private static void phoneLandscape() {
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        int spacing = KeyboardSpacingPolicy.DEFAULT_KEY_SPACING_TENTHS;
        int gap = SplitKeyboardPolicy.DEFAULT_GAP_PERCENT;
        int threshold = SplitKeyboardPolicy.DEFAULT_THRESHOLD_DP;
        // 第一排字母键 = 可用宽度 × 75% / 10 − 6 dp 键距；默认设置下 560 dp 正好是 36 dp 边界。
        close(SplitKeyboardPolicy.phoneLetterKeyWidthDp(800, spacing, gap), 54f, "800 dp keyboard gives 54 dp keys");
        close(SplitKeyboardPolicy.phoneLetterKeyWidthDp(560, spacing, gap), 36f, "560 dp keyboard sits on the 36 dp boundary");
        check(SplitKeyboardPolicy.phoneLetterKeyWidthDp(0, spacing, gap) == 0f, "an unknown width never splits");
        // 常见手机横屏：扣掉侧边导航栏后仍有 780 dp 左右，分。
        check(SplitKeyboardPolicy.phoneActive(true, 411, true, threshold, 780, spacing, gap), "an ordinary phone in landscape splits");
        check(SplitKeyboardPolicy.phoneActive(true, 411, true, threshold, 560, spacing, gap), "exactly 36 dp keys split");
        check(!SplitKeyboardPolicy.phoneActive(true, 411, true, threshold, 559, spacing, gap), "keys narrower than 36 dp stay whole");
        // 屏幕很宽，但键盘可用宽度被分屏或系统留白压窄：按实际宽度判断，不分。
        check(!SplitKeyboardPolicy.phoneActive(true, 411, true, threshold, 500, spacing, gap), "a narrowed keyboard surface stays whole");
        // 更窄的键距、更小的空隙会让键变宽，门槛随之放宽。
        check(SplitKeyboardPolicy.phoneActive(true, 411, true, threshold, 540, KeyboardSpacingPolicy.MIN_KEY_SPACING_TENTHS,
            SplitKeyboardPolicy.MIN_GAP_PERCENT), "narrow spacing and a small gap widen the keys");
        close(SplitKeyboardPolicy.phoneLetterKeyWidthDp(780, spacing, SplitKeyboardPolicy.MAX_GAP_PERCENT + 10),
            SplitKeyboardPolicy.phoneLetterKeyWidthDp(780, spacing, SplitKeyboardPolicy.MAX_GAP_PERCENT),
            "out-of-range gap clamps like the large-screen branch");
        // 默认关；竖屏不分；大屏走自己的分支，手机开关不管。
        check(!SplitKeyboardPolicy.phoneActive(false, 411, true, threshold, 780, spacing, gap), "off by default");
        check(!SplitKeyboardPolicy.phoneActive(true, 411, false, threshold, 780, spacing, gap), "portrait never splits on a phone");
        check(!SplitKeyboardPolicy.phoneActive(true, 600, true, threshold, 1200, spacing, gap), "a large screen is not the phone branch");
        check(SplitKeyboardPolicy.phoneActive(true, 599, true, threshold, 780, spacing, gap), "just below the threshold is a phone");
        check(SplitKeyboardPolicy.phoneActive(true, 640, true, 700, 900, spacing, gap), "below a raised threshold is still a phone");
        check(!SplitKeyboardPolicy.active(true, 411, true), "the large-screen switch alone never splits a phone");
        // 布局：只有 26 键一族和韩文。
        check(SplitKeyboardPolicy.phoneDrawn(true, 411, true, standard, threshold, 780, spacing, gap), "26 keys drawn split");
        check(SplitKeyboardPolicy.phoneDrawn(true, 411, true, KeyboardLayout.KOREAN_LAYOUT, threshold, 780, spacing, gap), "Korean drawn split");
        check(!SplitKeyboardPolicy.phoneDrawn(true, 411, true, KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, threshold, 780, spacing, gap),
            "nine-key stays whole");
        // 手机分离铺满窗口，不套大屏的 720 dp 居中外框。
        check(KeyboardFormFactorPolicy.surfaceWidthDp(411, 891, true) == 0, "a split phone keyboard fills the window");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(640, 960, true) == 0, "a split keyboard below a raised threshold fills too");
        check(SplitKeyboardPolicy.effectiveOneHanded("left", true).equals("off"), "one-handed yields to a phone split");
    }

    private static void close(float actual, float expected, String message) {
        if (Math.abs(actual - expected) > EPSILON) throw new AssertionError(message + ": " + actual + " vs " + expected);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
