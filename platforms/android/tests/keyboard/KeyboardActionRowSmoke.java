import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardActionRow.Slot;
import app.msime.android.KeyboardLayout;
import app.msime.android.ZhuyinKeyboardLayout;
import java.util.List;

public final class KeyboardActionRowSmoke {
    public static void main(String[] args) {
        // 14 键（以及手写、大千注音）的底行：123 1.25 / ， 1 / 空格 4 / 。 1 / 中 1.05 / ↵ 1.9，地球键条件插在 123 之后。
        List<KeyboardActionRow.DesignEntry> design =
            KeyboardActionRow.designEntries(KeyboardLayout.FOURTEEN_KEY_LAYOUT, false);
        check(design.equals(List.of(
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.LAYER, 1.25f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.COMMA, 1f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.SPACE, 4f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.PERIOD, 1f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.LANGUAGE, 1.05f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.RETURN, 1.9f))),
            "design row order and weights");
        List<KeyboardActionRow.DesignEntry> withGlobe =
            KeyboardActionRow.designEntries(KeyboardLayout.FOURTEEN_KEY_LAYOUT, true);
        check(withGlobe.size() == 7 && withGlobe.get(1).slot() == KeyboardActionRow.DesignSlot.GLOBE
                && withGlobe.get(5).slot() == KeyboardActionRow.DesignSlot.LANGUAGE,
            "the globe follows 123 only when switching is offered, 中/英 stays next to return");
        List<KeyboardActionRow.DesignEntry> nineKey =
            KeyboardActionRow.designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true);
        check(nineKey.get(nineKey.size() - 2).slot() == KeyboardActionRow.DesignSlot.LANGUAGE
                && nineKey.get(nineKey.size() - 1).slot() == KeyboardActionRow.DesignSlot.RETURN,
            "中/英 sits right before return on the nine-key row too");
        // 拼音九键和笔画外框保留原五键底行，不增加独立标点。
        for (int layout : new int[] {KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.STROKE_LAYOUT}) {
            check(designSlots(layout, false, false).equals(List.of(KeyboardActionRow.DesignSlot.SYMBOL_PANEL,
                    KeyboardActionRow.DesignSlot.LAYER, KeyboardActionRow.DesignSlot.SPACE,
                    KeyboardActionRow.DesignSlot.LANGUAGE, KeyboardActionRow.DesignSlot.RETURN)),
                "nine-key frame row: 符 | 123 | space | 中 | return, layout " + layout);
            check(designSlots(layout, true, false).equals(List.of(KeyboardActionRow.DesignSlot.SYMBOL_PANEL,
                    KeyboardActionRow.DesignSlot.LAYER, KeyboardActionRow.DesignSlot.GLOBE,
                    KeyboardActionRow.DesignSlot.SPACE, KeyboardActionRow.DesignSlot.LANGUAGE,
                    KeyboardActionRow.DesignSlot.RETURN)),
                "the globe follows 123, after 符, layout " + layout);
            // 交换语言键与数字键：符 仍在最左，中/英 换到 123 的位置。
            check(designSlots(layout, true, true).equals(List.of(KeyboardActionRow.DesignSlot.SYMBOL_PANEL,
                    KeyboardActionRow.DesignSlot.LANGUAGE, KeyboardActionRow.DesignSlot.GLOBE,
                    KeyboardActionRow.DesignSlot.SPACE, KeyboardActionRow.DesignSlot.LAYER,
                    KeyboardActionRow.DesignSlot.RETURN)),
                "swapping 中/英 and 123 keeps 符 first, layout " + layout);
            check(designSlots(layout, false, true).equals(List.of(KeyboardActionRow.DesignSlot.SYMBOL_PANEL,
                    KeyboardActionRow.DesignSlot.LANGUAGE, KeyboardActionRow.DesignSlot.SPACE,
                    KeyboardActionRow.DesignSlot.LAYER, KeyboardActionRow.DesignSlot.RETURN)),
                "swapping without the globe keeps 符 first, layout " + layout);
            check(KeyboardActionRow.designEntries(layout, true).get(0).equals(new KeyboardActionRow.DesignEntry(
                    KeyboardActionRow.DesignSlot.SYMBOL_PANEL, KeyboardActionRow.DESIGN_SYMBOL_PANEL_WEIGHT))
                    && KeyboardActionRow.DESIGN_SYMBOL_PANEL_WEIGHT == KeyboardActionRow.DESIGN_LANGUAGE_WEIGHT,
                "符 takes the same share as 中/英");
        }
        // 26 键保留逗号、移到空格右侧，符号入口占旧逗号位置；地球键仍紧跟数字入口。
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.KOREAN_LAYOUT}) {
            for (boolean globe : new boolean[] {false, true}) {
                List<KeyboardActionRow.DesignSlot> expected = new java.util.ArrayList<>(List.of(
                    KeyboardActionRow.DesignSlot.LAYER, KeyboardActionRow.DesignSlot.SYMBOL_PANEL,
                    KeyboardActionRow.DesignSlot.SPACE, KeyboardActionRow.DesignSlot.COMMA,
                    KeyboardActionRow.DesignSlot.LANGUAGE, KeyboardActionRow.DesignSlot.RETURN));
                if (globe) expected.add(1, KeyboardActionRow.DesignSlot.GLOBE);
                check(designSlots(layout, globe, false).equals(expected), "26 键底行保留独立逗号");
                check(KeyboardActionRow.usesCommaSecondaryPeriod(layout), "26 键长按逗号输入句号");
            }
        }
        check(!KeyboardActionRow.usesCommaSecondaryPeriod(KeyboardLayout.FOURTEEN_KEY_LAYOUT)
            && !KeyboardActionRow.usesCommaSecondaryPeriod(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT),
            "其他布局不改长按动作");
        // 14 键第三行左端已经有 符；它和手写、大千注音的底行不带 符，仍有逗号句号。
        for (int layout : new int[] {KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT,
                KeyboardLayout.HANDWRITING_LAYOUT}) {
            for (boolean globe : new boolean[] {false, true}) {
                check(!designSlots(layout, globe, false).contains(KeyboardActionRow.DesignSlot.SYMBOL_PANEL)
                        && !designSlots(layout, globe, true).contains(KeyboardActionRow.DesignSlot.SYMBOL_PANEL),
                    "no 符 on the 14-key, handwriting or Dachen row, layout " + layout);
                check(KeyboardActionRow.designEntries(layout, globe).equals(
                        KeyboardActionRow.designEntries(KeyboardLayout.FOURTEEN_KEY_LAYOUT, globe)),
                    "the 14-key, handwriting and Dachen rows keep comma and period, layout " + layout);
            }
        }
        check("打开符号面板".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.SYMBOL_PANEL, true)),
            "符 describes the panel it opens");
        // 数字层只保留一个 0，原空格挪进右列；地球键和语言/数字交换仍可组合。
        for (boolean globe : new boolean[] {false, true}) {
            for (boolean swap : new boolean[] {false, true}) {
                List<KeyboardActionRow.DesignEntry> numeric = KeyboardActionRow.designEntries(
                    KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, globe, swap, true);
                List<KeyboardActionRow.DesignSlot> slots = numeric.stream()
                    .map(KeyboardActionRow.DesignEntry::slot).toList();
                check(slots.get(0) == KeyboardActionRow.DesignSlot.SYMBOL_PANEL
                        && slots.get(1) == (swap ? KeyboardActionRow.DesignSlot.LANGUAGE
                            : KeyboardActionRow.DesignSlot.LAYER),
                    "数字底行以符号和九键入口起头，交换时语言键占九键位置");
                check(slots.contains(KeyboardActionRow.DesignSlot.GLOBE) == globe
                        && slots.stream().filter(slot -> slot == KeyboardActionRow.DesignSlot.ZERO).count() == 1
                        && !slots.contains(KeyboardActionRow.DesignSlot.SPACE),
                    "数字底行唯一的 0 替下空格，地球键遵守宿主能力");
                check(slots.get(slots.size() - 3) == KeyboardActionRow.DesignSlot.ZERO
                        && slots.get(slots.size() - 2) == (swap ? KeyboardActionRow.DesignSlot.LAYER
                            : KeyboardActionRow.DesignSlot.LANGUAGE)
                        && slots.get(slots.size() - 1) == KeyboardActionRow.DesignSlot.RETURN,
                    "0 在底行中间，右侧保留语言与回车");
                check(KeyboardActionRow.languageBesideReturn(numeric) == !swap,
                    "回车只向相邻的语言键让位");
            }
        }
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
                KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT})
            check(KeyboardActionRow.designEntries(layout, true, false, true)
                    .equals(KeyboardActionRow.designEntries(layout, true)),
                "九键数字底行不改变其他布局");
        check("数字 0".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.ZERO, true)),
            "0 有独立无障碍描述");
        // 各普通底行交换动作与宽度，地球和其它按键顺序保持，回车不向数字入口让位。
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT,
                KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.KOREAN_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT,
                KeyboardLayout.STROKE_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT}) {
            for (boolean globe : new boolean[] {false, true}) {
                List<KeyboardActionRow.DesignEntry> original = KeyboardActionRow.designEntries(layout, globe);
                List<KeyboardActionRow.DesignEntry> swapped = KeyboardActionRow.designEntries(layout, globe, true);
                check(swapped.size() == original.size(), "swap preserves key count");
                int layer = designSlots(layout, globe, false).indexOf(KeyboardActionRow.DesignSlot.LAYER);
                check(swapped.get(layer).equals(new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.LANGUAGE,
                    KeyboardActionRow.DESIGN_LANGUAGE_WEIGHT)), "language takes the number entry's position with its width");
                check(swapped.get(swapped.size() - 2).equals(new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.LAYER,
                    KeyboardActionRow.DESIGN_LAYER_WEIGHT)), "number entry takes language position with its width");
                for (int index = 0; index < original.size(); index++) {
                    if (index != layer && original.get(index).slot() != KeyboardActionRow.DesignSlot.LANGUAGE)
                        check(swapped.get(index).equals(original.get(index)), "other keys keep position and width");
                }
                check(!KeyboardActionRow.languageBesideReturn(swapped), "return does not yield to numeric entry");
                check(KeyboardActionRow.designEntries(layout, globe, false).equals(original), "off restores default");
            }
        }

        // 中/英紧挨回车的两种底行，回车都把键帽左边让给中/英；没有底行时不让。
        check(KeyboardActionRow.languageBesideReturn(design)
                && KeyboardActionRow.languageBesideReturn(withGlobe)
                && KeyboardActionRow.languageBesideReturn(nineKey)
                && KeyboardActionRow.languageBesideReturn(
                    KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "return yields to the language key wherever it sits right before return");
        check(!KeyboardActionRow.languageBesideReturn(List.of())
                && !KeyboardActionRow.languageBesideReturn(List.of(
                    new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.SPACE, 4f),
                    new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.RETURN, 1.9f))),
            "a return key with something else on its left keeps its whole cap");
        check(KeyboardActionRow.RETURN_YIELD_DP > 0 && KeyboardActionRow.RETURN_YIELD_DP < 10,
            "the return key yields a few dp, not a large part of itself");
        check(KeyboardActionRow.designEntries(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, true).isEmpty(),
            "the kana grid takes no design row either");
        check(KeyboardActionRow.designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false).stream()
                .noneMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.COMMA
                    || entry.slot() == KeyboardActionRow.DesignSlot.PERIOD),
            "nine-key keeps punctuation in its sidebar");
        check(KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false).isEmpty()
                && KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true).isEmpty(),
            "the Zhuyin nine-key block carries 123, language, space and return itself, so there is no design row");
        check("，".equals(KeyboardActionRow.punctuationFace(KeyboardActionRow.DesignSlot.COMMA, true))
            && ".".equals(KeyboardActionRow.punctuationFace(KeyboardActionRow.DesignSlot.PERIOD, false)),
            "comma and period faces follow Chinese punctuation");
        check("切换到数字和符号".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.LAYER, true))
            && "切换中英文".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.LANGUAGE, true))
            && "切换输入法".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.GLOBE, true))
            && "空格".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.SPACE, true)),
            "design row descriptions follow §2.8");

        check(KeyboardActionRow.entries(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, true).isEmpty(),
            "the kana grid carries its own side columns and takes no action row");
        check(slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true).equals(List.of(
                Slot.SYMBOL_PANEL, Slot.LAYER, Slot.GLOBE, Slot.PUNCTUATION, Slot.SPACE,
                Slot.LANGUAGE, Slot.RETURN)),
            "standard row order");
        check(!slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false).contains(Slot.GLOBE),
            "no globe when the host cannot switch input methods");
        check(!slots(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true).contains(Slot.PUNCTUATION),
            "the nine-key sidebar already carries punctuation");
        check(!slots(KeyboardLayout.STROKE_LAYOUT, true).contains(Slot.PUNCTUATION),
            "the stroke keypad's sidebar carries punctuation as nine-key's does");
        check(slots(KeyboardLayout.HANDWRITING_LAYOUT, true).contains(Slot.PUNCTUATION),
            "handwriting keeps the quick punctuation key");
        check(slots(KeyboardLayout.KOREAN_LAYOUT, true).equals(
                slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the Korean keycaps sit over the standard rows and keep the standard action row");
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT,
                KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT,
                KeyboardLayout.KOREAN_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
                KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT}) {
            List<Slot> slots = slots(layout, true);
            check(slots.contains(Slot.SPACE) && slots.contains(Slot.RETURN)
                && slots.contains(Slot.LANGUAGE), "every action row commits, spaces and switches");
            check(slots.size() == slots.stream().distinct().count(), "no slot appears twice");
            float total = 0;
            for (KeyboardActionRow.Entry entry : KeyboardActionRow.entries(layout, true))
                total += entry.weight();
            check(total > 0, "weights are positive");
        }
        check(weight(KeyboardLayout.STANDARD_TOUCH_LAYOUT, Slot.SPACE)
            > weight(KeyboardLayout.STANDARD_TOUCH_LAYOUT, Slot.RETURN),
            "space is the widest key in the row");

        // Delete and 大小写 live in the last letter row whenever those rows are the surface, so the
        // action row must not repeat them.
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false),
            "letter rows end with delete");
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true),
            "symbol rows end with delete");
        check(!KeyboardActionRow.rowsCarryDelete(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false),
            "the nine-key grid owns its delete key");
        check(!KeyboardActionRow.rowsCarryDelete(KeyboardLayout.HANDWRITING_LAYOUT, false),
            "the handwriting tool column owns its delete key");
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.HANDWRITING_LAYOUT, true),
            "handwriting hands its symbol layer to the letter rows");
        check(KeyboardActionRow.rowsCarryCase(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false),
            "letter rows start with the case key");
        check(!KeyboardActionRow.rowsCarryCase(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true),
            "the digit page has no case to shift");
        check(!KeyboardActionRow.rowsCarryCase(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false),
            "九键切数字仍然是九键，没有大小写可切");
        check(KeyboardActionRow.rowsCarryCase(KeyboardLayout.KOREAN_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.KOREAN_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.KOREAN_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.KOREAN_LAYOUT, true),
            "the Korean rows carry Shift for the double consonants and delete, like the standard rows");
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_LAYOUT, false)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_LAYOUT, true),
            "the Dachen rows carry delete but no case key: bopomofo has no case");
        check(!KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STROKE_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STROKE_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.STROKE_LAYOUT, false)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.STROKE_LAYOUT, true),
            "the stroke grid owns its delete key and hands its symbol layer to the letter rows, with no case key");
        check(slots(KeyboardLayout.STROKE_LAYOUT, true).equals(
                slots(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true)),
            "the stroke keypad keeps the nine-key action row");
        check(slots(KeyboardLayout.ZHUYIN_LAYOUT, true).equals(
                slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the Dachen rows keep the standard action row");
        // 注音 9 键的网格自带 @#、逗号句号和 ⌫，底排不再重复它们。
        check(slots(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true).equals(List.of(
                Slot.LAYER, Slot.GLOBE, Slot.SPACE, Slot.LANGUAGE, Slot.RETURN))
                && slots(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false).equals(List.of(
                Slot.LAYER, Slot.SPACE, Slot.LANGUAGE, Slot.RETURN)),
            "the Zhuyin nine-key grid carries the symbol panel key and punctuation itself");
        check(!KeyboardActionRow.usesLetterRows(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false)
                && KeyboardActionRow.usesLetterRows(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true),
            "the Zhuyin nine-key grid owns its delete key and hands its symbol layer to the letter rows, with no case key");
        int standard = KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        int zhuyin = KeyboardLayout.ZHUYIN_LAYOUT;
        check(KeyboardActionRow.letterRowEdgeWeight(standard, false)
            > KeyboardActionRow.letterRowEdgeWeight(standard, true),
            "the ten-key symbol row leaves its edges less room than the seven-key letter row");
        // ㄈ…ㄥ plus ⌫ spans the same eleven shares as the digit row, so every key on both rows is one width.
        check(ZhuyinKeyboardLayout.rows().get(3).size() + KeyboardActionRow.letterRowEdgeWeight(zhuyin, false)
                == ZhuyinKeyboardLayout.rows().get(0).size(),
            "the Dachen ⌫ takes one key's share");
        check(KeyboardActionRow.letterRowEdgeWeight(zhuyin, true)
                == KeyboardActionRow.letterRowEdgeWeight(standard, true),
            "the Dachen symbol page keeps the standard edges");

        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false)),
            "letters offer the digit page");
        check("ABC".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the digit page returns to 26 keys");
        check("九键".equals(KeyboardActionRow.layerTitle(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true)),
            "the digit page returns to the grid the user picked");
        check("拼音".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true, true))
                && "拼音".equals(KeyboardActionRow.layerTitle(KeyboardLayout.HANDWRITING_LAYOUT, true, true)),
            "a borrowed Chinese number pad returns by the Chinese letters title");
        check("九键".equals(KeyboardActionRow.layerTitle(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true, true))
                && "14键".equals(KeyboardActionRow.layerTitle(KeyboardLayout.FOURTEEN_KEY_LAYOUT, true, true))
                && "ABC".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true, false))
                && "123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false, false)),
            "named faces keep their own return title");
        check("14键".equals(KeyboardActionRow.layerTitle(KeyboardLayout.FOURTEEN_KEY_LAYOUT, true)),
            "the fourteen-key keyboard returns from its digit page by name");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.KOREAN_LAYOUT, false))
                && "한".equals(KeyboardActionRow.layerTitle(KeyboardLayout.KOREAN_LAYOUT, true)),
            "the Korean digit page returns to the Hangul keycaps");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_LAYOUT, false))
                && "注".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_LAYOUT, true)),
            "the Zhuyin digit page returns to the Dachen keycaps");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false))
                && "注".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true)),
            "the Zhuyin nine-key digit page returns to the bopomofo grid");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STROKE_LAYOUT, false))
                && "笔".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STROKE_LAYOUT, true)),
            "the stroke digit page returns to the stroke keypad");
        check("切换到数字和符号".equals(KeyboardActionRow.layerDescription(false))
            && "切换到字母键盘".equals(KeyboardActionRow.layerDescription(true)),
            "layer key descriptions");

        List<KeyboardLayout.LayerKey> chineseNumberBottom =
            KeyboardLayout.numberLayer(true, true).get(3);
        List<KeyboardLayout.LayerKey> englishNumberBottom =
            KeyboardLayout.numberLayer(true, false).get(3);
        check(chineseNumberBottom.size() == 5 && englishNumberBottom.size() == 5
                && "中标".equals(chineseNumberBottom.get(2).text())
                && "英标".equals(englishNumberBottom.get(2).text())
                && chineseNumberBottom.get(2).kind() == KeyboardLayout.LayerKeyKind.PUNCTUATION_TOGGLE
                && "切换到英文标点".equals(chineseNumberBottom.get(2).description())
                && "切换到中文标点".equals(englishNumberBottom.get(2).description()),
            "number layers expose a Chinese/English punctuation toggle beside emoji");
        check(KeyboardLayout.moreSymbolLayer(true, true).get(3).get(2).kind()
                == KeyboardLayout.LayerKeyKind.PUNCTUATION_TOGGLE,
            "the extended symbol layer keeps the punctuation toggle");
        System.out.println("Android action row: composition, weights and layer faces passed");
    }

    private static List<KeyboardActionRow.DesignSlot> designSlots(int layout, boolean globe, boolean swap) {
        return KeyboardActionRow.designEntries(layout, globe, swap).stream()
            .map(KeyboardActionRow.DesignEntry::slot).toList();
    }

    private static List<Slot> slots(int layout, boolean globe) {
        return KeyboardActionRow.entries(layout, globe).stream()
            .map(KeyboardActionRow.Entry::slot).toList();
    }

    private static float weight(int layout, Slot slot) {
        for (KeyboardActionRow.Entry entry : KeyboardActionRow.entries(layout, true))
            if (entry.slot() == slot) return entry.weight();
        throw new AssertionError("Missing slot: " + slot);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
